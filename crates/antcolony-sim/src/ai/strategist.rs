//! Rule-based Strategist KS. Watches colony state + threats and emits
//! high-level Goals. Replaces the existing scripted red-team AI behavior
//! when the blackboard system is wired into `Simulation::tick`.
//!
//! Phase 9.1 implementation is intentionally simple: 3-4 hand-coded
//! rules that map state → Goal. Phase 9.3 swaps this out for an LLM
//! backend that reads the same blackboard snapshot and writes the
//! same Goal facts via JSON output.

use crate::ai::blackboard::{BlackboardSnapshot, Directive, Fact, KsId, ObservationKind};
use crate::ai::knowledge_source::{Cadence, Contribution, KnowledgeSource, KsName};
use crate::colony::{BehaviorWeights, CasteRatio};
use crate::simulation::Simulation;

const KS_ID: KsId = 1;

pub struct StrategistKs;

impl KnowledgeSource for StrategistKs {
    fn name(&self) -> KsName {
        KsName {
            id: KS_ID,
            display: "Strategist",
        }
    }

    fn cadence(&self) -> Cadence {
        Cadence::EveryNSubsteps(60)
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
        let total_adults = colony.adult_total();
        let target_pop = sim.config.colony.target_population.max(1);

        // Rule 1: low food → push foragers. Goal priority high if we're
        // also approaching starvation.
        if colony.food_stored < (sim.config.colony.egg_cost * 4.0) {
            let urgency = if colony.food_stored < 0.5 { 0.95 } else { 0.6 };
            out.push(Contribution {
                fact: Fact::Goal {
                    directive: Directive::AdjustBehaviorWeights(BehaviorWeights {
                        forage: 0.85,
                        dig: 0.05,
                        nurse: 0.10,
                    }),
                    priority: urgency,
                    source: KS_ID,
                },
            });
        }

        // Rule 2: population over saturation cap → throttle laying via
        // a Goal that the queen-economy code can read. (For now the
        // saturation cap is enforced directly in colony_economy_tick;
        // this Goal is a hook for future LLM strategist to override.)
        if total_adults as f32 > (target_pop as f32) * 1.0 {
            out.push(Contribution {
                fact: Fact::Goal {
                    directive: Directive::AdjustBehaviorWeights(BehaviorWeights {
                        forage: 0.5,
                        dig: 0.3,
                        nurse: 0.2,
                    }),
                    priority: 0.5,
                    source: KS_ID,
                },
            });
        }

        // Rule 3: any unresolved Threat from Combat KS → boost soldier
        // ratio.
        let threat_count = bb
            .facts
            .iter()
            .filter(|f| matches!(f, Fact::Threat { .. }))
            .count();
        if threat_count > 0 {
            out.push(Contribution {
                fact: Fact::Goal {
                    directive: Directive::AdjustCasteRatio(CasteRatio {
                        worker: 0.55,
                        soldier: 0.40,
                        breeder: 0.05,
                    }),
                    priority: 0.7 + (threat_count as f32 * 0.05).min(0.25),
                    source: KS_ID,
                },
            });
        }

        // Rule 4: brood pipeline near-empty → emergency caste reset
        // toward worker production.
        if (colony.eggs + colony.larvae + colony.pupae) < 3 && colony.queen_health > 0.0 {
            out.push(Contribution {
                fact: Fact::Goal {
                    directive: Directive::AdjustCasteRatio(CasteRatio {
                        worker: 0.95,
                        soldier: 0.0,
                        breeder: 0.05,
                    }),
                    priority: 0.85,
                    source: KS_ID,
                },
            });
            out.push(Contribution {
                fact: Fact::Observation {
                    what: ObservationKind::BroodPipelineEmpty,
                    tick: bb.current_tick,
                    source: KS_ID,
                    confidence: 0.95,
                },
            });
        }

        out
    }
}
