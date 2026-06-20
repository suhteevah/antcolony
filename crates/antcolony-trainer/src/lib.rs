//! antcolony-trainer — pure-Rust RL training for the colony AI brain.
//!
//! Architecture: actor-critic with Tanh-squashed Gaussian policy over the
//! 6-dim AiDecision space, trained via PPO with GAE. Runs the sim
//! IN-PROCESS (no subprocess overhead) — every match feeds tensors
//! directly into Candle's autograd graph.
//!
//! Per the May 2026 literature review (`docs/ai-literature-review-2026-05.md`),
//! we drop BC entirely in favor of outcome-driven RL with league self-play.
//! The trained policy ends up in the same MlpBrain JSON format the sim
//! already loads, so inference deployment requires zero Rust changes
//! beyond this crate.
//!
//! # Aether parity
//!
//! Every Candle feature this crate uses is tracked in
//! `J:\aether\ANTCOLONY_FR.md` as a feature Aether should add. The
//! trainer is built around a `Backend` trait so Aether can replace
//! Candle as a single trait-impl swap once it ships parity.

pub mod backend;
pub mod policy;
pub mod ppo;
pub mod env;
pub mod eval;
pub mod league;
pub mod hierarchical;
pub mod joint_ppo;
pub mod reward;
pub mod parallel_env;
pub mod phase3;
pub mod self_play;
pub use hierarchical::{HierarchicalActorCritic, CommanderPolicy, AntPolicy, Sizing};
pub use parallel_env::ParallelEnv;
pub use joint_ppo::{JointPpoConfig, JointPpoTrainer, JointRollout, JointLossStats};
pub use reward::{ColonyMetrics, RewardConfig, compute_step_reward};
pub use hierarchical::obs_to_tensors::{rich_to_tensors, ant_obs_to_tensors, rich_batch_to_tensors};

pub use eval::{EvalReport, evaluate_hac};
pub use phase3::{Phase3Config, run_phase3, Phase3Report};
pub use backend::{Backend, CandleBackend};
pub use policy::ActorCritic;
pub use ppo::{PpoConfig, PpoTrainer};
pub use env::{MatchEnv, Trajectory, StepRecord};
pub use league::League;
pub use self_play::{SnapshotPool, OpponentKind};

/// Match the MlpBrain layout in crates/antcolony-sim/src/ai/brain.rs.
/// `INPUT_DIM` and `OUTPUT_DIM` are locked by the sim's state/decision
/// schema. `HIDDEN_DIM` is the default — runtime override via
/// `PpoConfig.hidden_dim` lets us ship wider nets (128, 256) without
/// breaking deployment, since MlpBrain reads dimensions out of the
/// weight matrices at load time.
pub const INPUT_DIM: usize = 17;
pub const HIDDEN_DIM: usize = 64;
pub const OUTPUT_DIM: usize = 6;
