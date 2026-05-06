//! Wire-protocol message types.
//!
//! All messages are length-prefixed JSON: `[u32 BE length][JSON bytes]`.
//! See `transport.rs` for read/write helpers.

use antcolony_sim::AiDecision;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Top-level message type sent between peers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NetMessage {
    /// First message from each peer after the TCP handshake. Both peers
    /// send Hello; the connection only enters the gameplay phase after
    /// each side has accepted the other's Hello.
    Hello(HelloPayload),

    /// Acknowledgment of the partner's Hello. `accepted=false` means
    /// the responder rejects the offered config (version mismatch,
    /// seed disagreement, etc.) and the connection should close.
    HelloAck { protocol_version: u32, accepted: bool, reason: Option<String> },

    /// One peer's input for a given decision tick. Sent in lockstep --
    /// neither peer advances past `tick` until both have received the
    /// counterpart's `TickInput` for that tick.
    TickInput(TickInput),

    /// Cooperative goodbye. Sender drops connection after writing.
    Disconnect { reason: String },
}

/// Payload of the opening Hello.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HelloPayload {
    /// Must equal [`crate::PROTOCOL_VERSION`] on the receiver.
    pub protocol_version: u32,
    /// Which side this peer wants to play (Black = colony 0, Red = colony 1).
    pub peer_role: PeerRole,
    /// Match seed. Both peers MUST agree before play begins -- typically
    /// the host's seed, echoed by the joiner. Mismatch => HelloAck reject.
    pub seed: u64,
    /// FNV-1a hash of the SimConfig used to construct the sim. Different
    /// configs produce different sims even from the same seed, so we
    /// verify match before play.
    pub config_hash: u64,
    /// Friendly display name for the UI.
    pub display_name: String,
}

/// Per-tick input message. `decision` is the only "command" data each
/// peer contributes per decision tick; the sim is otherwise fully
/// determined by the seed and the prior shared inputs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TickInput {
    /// Decision tick this input applies to. Must arrive in monotonic
    /// order; a peer that receives an out-of-order TickInput must
    /// disconnect (the sim integrity is gone).
    pub tick: u64,
    /// AiDecision the sender wants applied to its colony at this tick.
    pub decision: AiDecision,
    /// Sender's local sim state hash AT this tick (after applying ANY
    /// previous-tick inputs, before this tick's). Used to detect desync.
    pub state_hash: u64,
}

/// Which colony slot the peer plays.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PeerRole {
    /// Colony 0 -- the "black" / "home" side. Host typically takes this.
    Black,
    /// Colony 1 -- the "red" / "away" side. Joiner typically takes this.
    Red,
    /// Future: spider rogue agent. Not used in V1 transport.
    Spider,
}

impl PeerRole {
    /// Numeric colony id this peer drives.
    pub fn colony_id(self) -> u8 {
        match self {
            PeerRole::Black => 0,
            PeerRole::Red => 1,
            PeerRole::Spider => 255,
        }
    }
}

#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("malformed json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("frame too large: {len} bytes (max {max})")]
    FrameTooLarge { len: u32, max: u32 },
    #[error("peer disconnected: {0}")]
    Disconnected(String),
    #[error("desync at tick {tick}: local hash {local:#x} != remote {remote:#x}")]
    Desync { tick: u64, local: u64, remote: u64 },
    #[error("protocol version mismatch: peer={peer} ours={ours}")]
    VersionMismatch { peer: u32, ours: u32 },
    #[error("seed mismatch: peer={peer} ours={ours}")]
    SeedMismatch { peer: u64, ours: u64 },
    #[error("config hash mismatch: peer={peer:#x} ours={ours:#x}")]
    ConfigMismatch { peer: u64, ours: u64 },
    #[error("both peers requested role {0:?}")]
    RoleConflict(PeerRole),
    #[error("unexpected message: {0}")]
    Unexpected(String),
}
