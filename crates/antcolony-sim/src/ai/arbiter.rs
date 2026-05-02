//! The Arbiter promotes the highest-priority Goals to Directive
//! commitments. Phase 9.1 implementation: simple priority-queue with
//! conflict resolution by directive type. Phase 9.3 swaps this out
//! for an LLM-arbitrated version that synthesizes contradictory
//! Goals into a unified plan.

use crate::ai::blackboard::{Blackboard, Directive, Fact};

pub struct Arbiter;

impl Arbiter {
    /// Promote the highest-priority Goal of each conflicting type to
    /// the blackboard's commitments vec. Goals are NOT removed from
    /// `facts` — keeping them lets the UI side-panel show "Strategist
    /// proposed X with priority 0.7, you can see why".
    pub fn arbitrate(blackboard: &mut Blackboard, current_tick: u64) {
        // Collect best Goal per directive type.
        let mut best_caste: Option<(&Directive, f32)> = None;
        let mut best_behavior: Option<(&Directive, f32)> = None;
        let mut best_excavate: Option<(&Directive, f32)> = None;
        let mut force_recall = false;
        let mut force_nuptial = false;

        for fact in &blackboard.facts {
            if let Fact::Goal { directive, priority, .. } = fact {
                match directive {
                    Directive::AdjustCasteRatio(_) => {
                        if best_caste.map(|(_, p)| *priority > p).unwrap_or(true) {
                            best_caste = Some((directive, *priority));
                        }
                    }
                    Directive::AdjustBehaviorWeights(_) => {
                        if best_behavior.map(|(_, p)| *priority > p).unwrap_or(true) {
                            best_behavior = Some((directive, *priority));
                        }
                    }
                    Directive::Excavate { .. } => {
                        if best_excavate.map(|(_, p)| *priority > p).unwrap_or(true) {
                            best_excavate = Some((directive, *priority));
                        }
                    }
                    Directive::RecallForagers => force_recall = true,
                    Directive::ForceNuptialFlight => force_nuptial = true,
                }
            }
        }

        // Replace commitments with the freshly-arbitrated set.
        blackboard.commitments.clear();
        if let Some((d, _)) = best_caste {
            blackboard.commitments.push(d.clone());
        }
        if let Some((d, _)) = best_behavior {
            blackboard.commitments.push(d.clone());
        }
        if let Some((d, _)) = best_excavate {
            blackboard.commitments.push(d.clone());
        }
        if force_recall {
            blackboard.commitments.push(Directive::RecallForagers);
        }
        if force_nuptial {
            blackboard.commitments.push(Directive::ForceNuptialFlight);
        }
        blackboard.last_arbitrated_tick = current_tick;
    }
}
