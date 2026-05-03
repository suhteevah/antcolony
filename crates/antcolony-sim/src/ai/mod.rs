//! Phase 9.1 — Blackboard reasoning + rule-based Knowledge Sources
//!
//! Implements the architecture described in `docs/ai-architecture.md`:
//! a per-colony `Blackboard` data structure that multiple specialized
//! `KnowledgeSource` implementors read from and contribute to. The
//! blackboard arbiter promotes high-priority contributions to
//! `Directive` commitments which the sim then enacts.
//!
//! This module provides:
//! - `Blackboard`, `Fact`, `Directive` types
//! - `KnowledgeSource` trait + `Cadence` enum
//! - Rule-based `Strategist`, `Forager`, and `Combat` KS implementations
//!   (no LLM — those land in Phase 9.3)
//! - `Arbiter` that promotes Goals to Directives and dedupes contradictions
//!
//! NOT YET wired into `Simulation::tick` — that happens in a follow-up
//! commit once the v2 validation sweep clears the binary lock.

pub mod blackboard;
pub mod brain;
pub mod knowledge_source;
pub mod strategist;
pub mod forager;
pub mod combat;
pub mod arbiter;

pub use arbiter::Arbiter;
pub use blackboard::{Blackboard, BlackboardSnapshot, Directive, Fact, FactRef};
pub use brain::{
    AetherLmBrain, AggressorBrain, AiBrain, AiDecision, BreederBrain, ColonyAiState,
    ConservativeBuilderBrain, DefenderBrain, EconomistBrain, ForagerBrain, HeuristicBrain,
    MatchStatus, MlpBrain, RandomBrain, TunedBrain,
};
pub use combat::CombatKs;
pub use forager::ForagerKs;
pub use knowledge_source::{Cadence, Contribution, KnowledgeSource, KsName};
pub use strategist::StrategistKs;
