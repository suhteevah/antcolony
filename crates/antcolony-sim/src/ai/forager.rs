//! Rule-based Forager KS. Watches food state, emits Observations
//! about food inflow + storage and Goals to redistribute behavior
//! weights when forage shifts are needed.

use crate::ai::blackboard::{BlackboardSnapshot, Fact, KsId, ObservationKind};
use crate::ai::knowledge_source::{Cadence, Contribution, KnowledgeSource, KsName};
use crate::simulation::Simulation;

const KS_ID: KsId = 2;

pub struct ForagerKs;

impl KnowledgeSource for ForagerKs {
    fn name(&self) -> KsName {
        KsName {
            id: KS_ID,
            display: "Forager",
        }
    }

    fn cadence(&self) -> Cadence {
        Cadence::EveryNSubsteps(30)
    }

    fn observe(
        &self,
        sim: &Simulation,
        bb: &BlackboardSnapshot,
    ) -> Vec<Contribution> {
        let Some(colony) = sim.colonies.iter().find(|c| c.id == bb.colony_id) else {
            return Vec::new();
        };
        let mut out = Vec::new();

        // Observation: high food inflow — let other KS know we're flush.
        if colony.food_inflow_recent > sim.config.colony.adult_food_consumption * 100.0 {
            out.push(Contribution {
                fact: Fact::Observation {
                    what: ObservationKind::HighFoodInflow {
                        rate_per_tick: colony.food_inflow_recent,
                    },
                    tick: bb.current_tick,
                    source: KS_ID,
                    confidence: 0.9,
                },
            });
        }

        // Observation: low food storage.
        if colony.food_stored < (sim.config.colony.egg_cost * 2.0) {
            out.push(Contribution {
                fact: Fact::Observation {
                    what: ObservationKind::LowFood {
                        food_stored: colony.food_stored,
                    },
                    tick: bb.current_tick,
                    source: KS_ID,
                    confidence: 0.95,
                },
            });
        }

        out
    }
}
