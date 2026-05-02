//! Per-colony Blackboard data structure. The colony's "thoughts" —
//! observations, threats, goals, hypotheses — live here. Each fact
//! is attributable to a `KnowledgeSource` so the player can see WHY
//! the colony made a decision in the side-panel UI.

use serde::{Deserialize, Serialize};

use crate::colony::{BehaviorWeights, CasteRatio};
use crate::module::ModuleId;

/// Identifier referencing a fact in the same blackboard's `facts` vec
/// by index. Used for `Hypothesis.support` etc.
pub type FactRef = u32;

/// One unit of structured "thought" on the blackboard. Facts are
/// produced by KS contributions and consumed by other KS or by the
/// arbiter when promoting Goals to commitments.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Fact {
    /// "I see X at Y." Most basic primitive — sensor-style.
    Observation {
        what: ObservationKind,
        tick: u64,
        source: KsId,
        confidence: f32,
    },
    /// "X is dangerous, expires by tick N." Used by Combat KS to alert
    /// other KS to threats.
    Threat {
        entity: ThreatRef,
        severity: f32,
        expires_tick: u64,
        source: KsId,
    },
    /// "I want the colony to do X with priority P." Goals are arbitrated
    /// into commitments (Directives applied to the sim).
    Goal {
        directive: Directive,
        priority: f32,
        source: KsId,
    },
    /// "I believe X because of (Y, Z)." Used by Strategist when
    /// integrating multiple Observations into a higher-level conclusion.
    Hypothesis {
        proposition: String,
        support: Vec<FactRef>,
        confidence: f32,
        source: KsId,
    },
}

/// Stable identifier for a knowledge source. Stored in facts so the
/// UI can attribute reasoning ("Forager KS observed low food trail").
pub type KsId = u8;

/// What an Observation is about. Extensible — add variants as KS grow.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ObservationKind {
    LowFood { food_stored: f32 },
    HighFoodInflow { rate_per_tick: f32 },
    PopulationApproachingTarget { fraction: f32 },
    ColonyEnteredDiapause,
    ColonyExitedDiapause,
    EnemySighted { module: ModuleId, count: u32 },
    QueenHealthLow { health: f32 },
    BroodPipelineEmpty,
    SoilExhaustedAtFace { module: ModuleId },
}

/// What a Threat references. For combat, an enemy ant or predator;
/// for environmental, a hazard like a lawnmower sweep.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ThreatRef {
    EnemyColony { colony_id: u8 },
    Predator { predator_id: u32 },
    Weather { kind: WeatherThreat },
    Starvation,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum WeatherThreat {
    LawnmowerSweep,
    Rain,
    Frost,
}

/// A commitment the arbiter promoted from a Goal. The sim's
/// `apply_directives` step (Phase 9.1 follow-up) consumes these and
/// mutates colony state accordingly.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Directive {
    /// "Shift caste production toward more soldiers / breeders / workers."
    AdjustCasteRatio(CasteRatio),
    /// "Shift behavior weights — more foraging, less digging, etc."
    AdjustBehaviorWeights(BehaviorWeights),
    /// "Excavate at this cell on this module." (Used by Architect KS.)
    Excavate { module: ModuleId, cell: (u32, u32) },
    /// "Recall foragers — winter is coming or there's a major threat."
    RecallForagers,
    /// "Push out for nuptial flight despite suboptimal conditions."
    ForceNuptialFlight,
}

/// Per-colony blackboard. Stored in `Simulation` as a parallel
/// `Vec<Blackboard>` indexed by colony id (or in a HashMap if colonies
/// are no longer densely-numbered).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Blackboard {
    pub colony_id: u8,
    pub facts: Vec<Fact>,
    pub commitments: Vec<Directive>,
    /// Tick when this blackboard was last arbitrated. Drives staleness
    /// pruning of old observations.
    pub last_arbitrated_tick: u64,
}

impl Blackboard {
    pub fn new(colony_id: u8) -> Self {
        Self {
            colony_id,
            facts: Vec::new(),
            commitments: Vec::new(),
            last_arbitrated_tick: 0,
        }
    }

    /// Append a new fact. KS contributions go through here.
    pub fn add_fact(&mut self, fact: Fact) {
        self.facts.push(fact);
    }

    /// Drop facts older than `max_age_ticks` from the current tick.
    /// Threats with explicit `expires_tick` are also pruned. Goals are
    /// preserved unless the arbiter committed them — committed goals
    /// move to `commitments` and out of `facts`.
    pub fn prune_stale(&mut self, current_tick: u64, max_age_ticks: u64) {
        self.facts.retain(|f| match f {
            Fact::Observation { tick, .. } => current_tick - *tick < max_age_ticks,
            Fact::Threat { expires_tick, .. } => *expires_tick > current_tick,
            // Goals + Hypotheses live until the arbiter handles them
            // (or until manually pruned).
            Fact::Goal { .. } | Fact::Hypothesis { .. } => true,
        });
    }

    /// Return all current Goals (unarbitrated suggestions).
    pub fn goals(&self) -> impl Iterator<Item = (&Directive, f32, KsId)> {
        self.facts.iter().filter_map(|f| match f {
            Fact::Goal { directive, priority, source } => Some((directive, *priority, *source)),
            _ => None,
        })
    }

    /// Return all current Threats.
    pub fn threats(&self) -> impl Iterator<Item = (&ThreatRef, f32)> {
        self.facts.iter().filter_map(|f| match f {
            Fact::Threat { entity, severity, .. } => Some((entity, *severity)),
            _ => None,
        })
    }
}

/// Read-only snapshot of a blackboard, passed to KS observers so they
/// can see other KS's contributions without mutating. The actual
/// blackboard is mutated by the arbiter alone.
#[derive(Debug)]
pub struct BlackboardSnapshot<'a> {
    pub colony_id: u8,
    pub facts: &'a [Fact],
    pub current_tick: u64,
}
