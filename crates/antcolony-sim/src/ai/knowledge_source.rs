//! `KnowledgeSource` trait — the contract every KS implements to
//! contribute to a colony's blackboard. Rule-based KS are cheap and
//! run every substep / outer-tick. LLM-backed KS (Phase 9.3) run on
//! a longer cadence and produce richer reasoning per call.

use crate::ai::blackboard::{BlackboardSnapshot, Fact, KsId};
use crate::simulation::Simulation;

/// Stable name + numeric id for a KS. Lets the UI attribute facts
/// to a specific source.
#[derive(Debug, Clone, Copy)]
pub struct KsName {
    pub id: KsId,
    pub display: &'static str,
}

/// How often a KS wants to fire. Cheap rule-based KS run per substep;
/// LLM-backed KS run on a slower cadence to keep the inference budget
/// bounded.
#[derive(Debug, Clone, Copy)]
pub enum Cadence {
    EverySubstep,
    EveryNSubsteps(u32),
    OnEvent,
    Manual,
}

/// One KS output: append a fact to the blackboard. The arbiter promotes
/// Goals to commitments downstream.
pub struct Contribution {
    pub fact: Fact,
}

pub trait KnowledgeSource: Send + Sync {
    fn name(&self) -> KsName;
    fn cadence(&self) -> Cadence;

    /// Read the sim + the current blackboard snapshot, return any
    /// new facts to append. Pure: doesn't mutate sim or blackboard.
    fn observe(
        &self,
        sim: &Simulation,
        blackboard: &BlackboardSnapshot,
    ) -> Vec<Contribution>;
}
