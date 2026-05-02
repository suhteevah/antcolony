//! Rule-based Combat KS. Watches enemy proximity + alarm pheromone +
//! losses_this_tick. Emits Threat facts that other KS (Strategist)
//! consume.

use crate::ai::blackboard::{
    BlackboardSnapshot, Fact, KsId, ThreatRef,
};
use crate::ai::knowledge_source::{Cadence, Contribution, KnowledgeSource, KsName};
use crate::simulation::Simulation;

const KS_ID: KsId = 3;

pub struct CombatKs;

impl KnowledgeSource for CombatKs {
    fn name(&self) -> KsName {
        KsName {
            id: KS_ID,
            display: "Combat",
        }
    }

    fn cadence(&self) -> Cadence {
        Cadence::EveryNSubsteps(15)
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

        // Recent losses → emit a Threat about each rival colony.
        if colony.combat_losses_this_tick > 0 {
            for other in &sim.colonies {
                if other.id == colony.id {
                    continue;
                }
                let severity = (colony.combat_losses_this_tick as f32 / 5.0).min(1.0);
                out.push(Contribution {
                    fact: Fact::Threat {
                        entity: ThreatRef::EnemyColony { colony_id: other.id },
                        severity,
                        expires_tick: bb.current_tick + 600,
                        source: KS_ID,
                    },
                });
            }
        }

        // Predator threat — look at simulation predators.
        for predator in &sim.predators {
            let severity = match predator.kind {
                crate::hazards::PredatorKind::Spider => 0.6,
                crate::hazards::PredatorKind::Antlion => 0.9,
            };
            out.push(Contribution {
                fact: Fact::Threat {
                    entity: ThreatRef::Predator {
                        predator_id: predator.id,
                    },
                    severity,
                    expires_tick: bb.current_tick + 300,
                    source: KS_ID,
                },
            });
        }

        out
    }
}
