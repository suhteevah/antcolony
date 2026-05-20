//! Hierarchical commander + per-ant policy nets for the ant-brain Phase 2.
//!
//! - [`sizing`] — A1/A2/A3 dim presets shared by both tiers
//! - [`transformer`] — minimal transformer block primitive (multi-head attn +
//!   LayerNorm + FFN), used by both tier backbones
//! - [`commander::CommanderPolicy`] — outer-tick commander brain
//! - [`ant::AntPolicy`] — per-ant brain (shared instance per colony)
//! - [`actor_critic::HierarchicalActorCritic`] — composes both tiers
//!
//! The existing flat [`crate::policy::ActorCritic`] MLP is unchanged — it
//! remains the 47% Nash regression baseline.

pub mod actor_critic;
pub mod ant;
pub mod commander;
pub mod sizing;
pub mod transformer;

// Re-exports are added progressively as each sub-module lands. Uncomment
// each line in its owning task:
pub use actor_critic::HierarchicalActorCritic;   // T8
pub use ant::AntPolicy;                          // T7
pub use commander::CommanderPolicy;              // T5
pub use sizing::{Sizing, A1, A2, A3};            // T3
