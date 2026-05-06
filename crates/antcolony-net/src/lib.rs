//! antcolony-net -- lockstep deterministic netcode for PvP.
//!
//! # Design (V1)
//!
//! - Direct-IP TCP. Host listens, client connects. No matchmaking.
//! - Lockstep: each tick (or every `decision_cadence` ticks), peers
//!   exchange a `TickInput` containing the local AiDecision plus a
//!   short hash of their local sim state. Both peers advance the sim
//!   only after both inputs for that tick are received.
//! - Hash mismatch = desync; peers dump full state for forensics.
//! - Length-prefixed JSON framing: `[u32 BE length][JSON payload]`.
//!   Trivial bandwidth (~12 msgs/sec) means JSON overhead is fine and
//!   the wire format stays human-debuggable.
//!
//! # Determinism gate
//!
//! Verified via `cargo run -p antcolony-sim --release --example det_check`
//! on 2026-05-06: same process / different processes / different
//! `RAYON_NUM_THREADS` all produce byte-identical state. The lockstep
//! protocol can therefore use simple state hashes for desync detection.
//!
//! # Linux / Proton-GE
//!
//! Pure `std::net` + `serde_json` -- no Windows-specific deps. Should
//! run unmodified on Linux x86_64 (the only target Proton-GE players
//! will use).

pub mod hash;
pub mod protocol;
pub mod transport;

pub use hash::sim_state_hash;
pub use protocol::{NetMessage, PeerRole, TickInput, HelloPayload, ProtocolError};
pub use transport::{LockstepPeer, PeerConfig, host, connect};

/// Wire-protocol version. Bump when the message layout changes in any
/// non-additive way. Peers refuse to play if versions disagree.
pub const PROTOCOL_VERSION: u32 = 1;

/// How many ticks elapse between AiDecision exchanges. Must match the
/// renderer / matchup_bench DECISION_CADENCE so brain decisions land at
/// the same tick boundaries on both peers.
pub const DECISION_CADENCE: u64 = 5;
