//! TCP transport for the lockstep protocol.
//!
//! Synchronous (no tokio) -- with a 30Hz sim and a 5-tick decision
//! cadence we exchange ~12 messages/sec total. A blocking thread per
//! peer is plenty.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream, ToSocketAddrs};
use std::time::Duration;

use crate::protocol::{HelloPayload, NetMessage, PeerRole, ProtocolError, TickInput};
use crate::PROTOCOL_VERSION;

/// 1 MiB cap. Messages are tiny (a couple hundred bytes); anything
/// over a megabyte is malformed or hostile.
pub const MAX_FRAME_BYTES: u32 = 1 << 20;

/// Configuration for a single peer.
#[derive(Debug, Clone)]
pub struct PeerConfig {
    pub role: PeerRole,
    pub seed: u64,
    pub config_hash: u64,
    pub display_name: String,
    /// Maximum wall-clock time to wait for the partner's TickInput
    /// before tearing down the connection. None = block forever.
    pub recv_timeout: Option<Duration>,
}

/// Bind to `addr` and accept the first incoming connection.
pub fn host(addr: impl ToSocketAddrs) -> Result<TcpStream, ProtocolError> {
    let listener = TcpListener::bind(addr)?;
    let local = listener.local_addr().ok();
    tracing::info!(addr = ?local, "host listening");
    let (stream, peer) = listener.accept()?;
    tracing::info!(?peer, "host accepted peer");
    Ok(stream)
}

/// Connect to `addr` (the host's listen address).
pub fn connect(addr: impl ToSocketAddrs) -> Result<TcpStream, ProtocolError> {
    let stream = TcpStream::connect(addr)?;
    tracing::info!(peer = ?stream.peer_addr().ok(), "connected to host");
    Ok(stream)
}

/// Stateful peer wrapper. Owns the TcpStream and tracks the protocol
/// state machine (handshake -> gameplay -> disconnect).
pub struct LockstepPeer {
    stream: TcpStream,
    pub config: PeerConfig,
    pub remote: Option<HelloPayload>,
}

impl LockstepPeer {
    pub fn new(stream: TcpStream, config: PeerConfig) -> Result<Self, ProtocolError> {
        if let Some(t) = config.recv_timeout {
            stream.set_read_timeout(Some(t))?;
        }
        Ok(Self { stream, config, remote: None })
    }

    /// Send our Hello, receive theirs, validate, exchange acks.
    /// On success, `self.remote` is populated and play can begin.
    pub fn handshake(&mut self) -> Result<(), ProtocolError> {
        let hello = HelloPayload {
            protocol_version: PROTOCOL_VERSION,
            peer_role: self.config.role,
            seed: self.config.seed,
            config_hash: self.config.config_hash,
            display_name: self.config.display_name.clone(),
        };
        write_message(&mut self.stream, &NetMessage::Hello(hello))?;

        let remote_hello = match read_message(&mut self.stream)? {
            NetMessage::Hello(h) => h,
            other => return Err(ProtocolError::Unexpected(format!("expected Hello, got {other:?}"))),
        };

        if remote_hello.protocol_version != PROTOCOL_VERSION {
            write_ack(&mut self.stream, false, Some(format!("version {} != {}", remote_hello.protocol_version, PROTOCOL_VERSION)))?;
            return Err(ProtocolError::VersionMismatch { peer: remote_hello.protocol_version, ours: PROTOCOL_VERSION });
        }
        if remote_hello.seed != self.config.seed {
            write_ack(&mut self.stream, false, Some("seed mismatch".into()))?;
            return Err(ProtocolError::SeedMismatch { peer: remote_hello.seed, ours: self.config.seed });
        }
        if remote_hello.config_hash != self.config.config_hash {
            write_ack(&mut self.stream, false, Some("config hash mismatch".into()))?;
            return Err(ProtocolError::ConfigMismatch { peer: remote_hello.config_hash, ours: self.config.config_hash });
        }
        if remote_hello.peer_role == self.config.role {
            write_ack(&mut self.stream, false, Some("both peers requested same role".into()))?;
            return Err(ProtocolError::RoleConflict(self.config.role));
        }

        write_ack(&mut self.stream, true, None)?;
        let ack = read_message(&mut self.stream)?;
        match ack {
            NetMessage::HelloAck { accepted: true, .. } => {}
            NetMessage::HelloAck { accepted: false, reason, .. } => {
                return Err(ProtocolError::Disconnected(reason.unwrap_or_else(|| "peer rejected".into())));
            }
            other => return Err(ProtocolError::Unexpected(format!("expected HelloAck, got {other:?}"))),
        }

        tracing::info!(
            local = %self.config.display_name,
            remote = %remote_hello.display_name,
            seed = self.config.seed,
            "handshake complete"
        );
        self.remote = Some(remote_hello);
        Ok(())
    }

    /// Send our TickInput for tick `t`, then block reading the partner's.
    /// Returns the partner's TickInput. Verifies the tick number matches
    /// (lockstep invariant). Hash mismatch produces a Desync error;
    /// caller decides whether to dump state and abort.
    pub fn exchange_tick(&mut self, ours: TickInput) -> Result<TickInput, ProtocolError> {
        let tick = ours.tick;
        let local_hash = ours.state_hash;
        write_message(&mut self.stream, &NetMessage::TickInput(ours))?;
        let remote = match read_message(&mut self.stream)? {
            NetMessage::TickInput(ti) => ti,
            NetMessage::Disconnect { reason } => return Err(ProtocolError::Disconnected(reason)),
            other => return Err(ProtocolError::Unexpected(format!("expected TickInput, got {other:?}"))),
        };
        if remote.tick != tick {
            return Err(ProtocolError::Unexpected(format!(
                "tick desync: sent {tick}, peer sent {}", remote.tick
            )));
        }
        if remote.state_hash != local_hash {
            return Err(ProtocolError::Desync {
                tick,
                local: local_hash,
                remote: remote.state_hash,
            });
        }
        Ok(remote)
    }

    /// Adjust the per-read timeout. Useful to set a generous timeout
    /// for the handshake then tighten it for gameplay.
    pub fn set_recv_timeout(&self, timeout: Option<Duration>) -> Result<(), ProtocolError> {
        self.stream.set_read_timeout(timeout)?;
        Ok(())
    }

    pub fn send_disconnect(&mut self, reason: impl Into<String>) -> Result<(), ProtocolError> {
        write_message(&mut self.stream, &NetMessage::Disconnect { reason: reason.into() })?;
        Ok(())
    }
}

fn write_ack(stream: &mut TcpStream, accepted: bool, reason: Option<String>) -> Result<(), ProtocolError> {
    write_message(stream, &NetMessage::HelloAck {
        protocol_version: PROTOCOL_VERSION,
        accepted,
        reason,
    })
}

/// Length-prefixed JSON write. `[u32 BE length][JSON bytes]`.
pub fn write_message(stream: &mut TcpStream, msg: &NetMessage) -> Result<(), ProtocolError> {
    let bytes = serde_json::to_vec(msg)?;
    let len = bytes.len();
    if len > MAX_FRAME_BYTES as usize {
        return Err(ProtocolError::FrameTooLarge { len: len as u32, max: MAX_FRAME_BYTES });
    }
    stream.write_all(&(len as u32).to_be_bytes())?;
    stream.write_all(&bytes)?;
    stream.flush()?;
    Ok(())
}

/// Length-prefixed JSON read.
pub fn read_message(stream: &mut TcpStream) -> Result<NetMessage, ProtocolError> {
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf)?;
    let len = u32::from_be_bytes(len_buf);
    if len > MAX_FRAME_BYTES {
        return Err(ProtocolError::FrameTooLarge { len, max: MAX_FRAME_BYTES });
    }
    let mut buf = vec![0u8; len as usize];
    stream.read_exact(&mut buf)?;
    let msg = serde_json::from_slice::<NetMessage>(&buf)?;
    Ok(msg)
}
