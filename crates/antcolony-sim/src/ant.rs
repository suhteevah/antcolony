//! Ant agent: FSM state, kinematics, and direction selection.
//!
//! All decisions are LOCAL — an ant can read only its sensing cone and its
//! own state. Colony-level behavior emerges from pheromone feedback.

use glam::Vec2;
use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::config::AntConfig;
use crate::module::ModuleId;
use crate::pheromone::{PheromoneGrid, PheromoneLayer};
use crate::tube::TubeTransit;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AntState {
    Idle,
    Exploring,
    FollowingTrail,
    PickingUpFood,
    ReturningHome,
    StoringFood,
    Fighting,
    Fleeing,
    Nursing,
    Digging,
    /// Winter diapause (K3): the ant is immobile and non-depositing until
    /// ambient temperature rises above the warm threshold.
    Diapause,
    /// K5 nuptial flight: breeders have left the nest to mate. Not
    /// moving on the grid; flight progress tracked by `state_timer`.
    /// Predation ticks while flying; at the end, a daughter colony may
    /// be founded (counter on `ColonyState`).
    NuptialFlight,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AntCaste {
    Worker,
    Soldier,
    Queen,
    Breeder,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ant {
    pub id: u32,
    pub position: Vec2,
    pub heading: f32,
    pub state: AntState,
    pub caste: AntCaste,
    pub colony_id: u8,
    pub health: f32,
    pub food_carried: f32,
    pub age: u32,
    pub state_timer: u32,
    /// Which module the ant is currently on (K2). Ignored and implicitly 0
    /// for pre-K2 single-module sims.
    pub module_id: ModuleId,
    /// `Some` when the ant is inside a tube (K2), `None` on a grid.
    pub transit: Option<TubeTransit>,
    /// P4 Avenger mechanic: exactly one ant per AI colony carries this
    /// flag. The avenger's heading is overridden each tick to track the
    /// nearest player ant. On death the role transfers to a random
    /// surviving non-queen sibling.
    #[serde(default)]
    pub is_avenger: bool,
}

impl Ant {
    pub fn new_worker(id: u32, colony_id: u8, position: Vec2, heading: f32, health: f32) -> Self {
        Self::new_with_caste(id, colony_id, position, heading, health, AntCaste::Worker)
    }

    pub fn new_with_caste(
        id: u32,
        colony_id: u8,
        position: Vec2,
        heading: f32,
        health: f32,
        caste: AntCaste,
    ) -> Self {
        Self {
            id,
            position,
            heading,
            state: AntState::Exploring,
            caste,
            colony_id,
            health,
            food_carried: 0.0,
            age: 0,
            state_timer: 0,
            module_id: 0,
            transit: None,
            is_avenger: false,
        }
    }

    #[inline]
    pub fn is_in_transit(&self) -> bool {
        self.transit.is_some()
    }

    /// Per-ant body size in mm, derived from the species' base size and
    /// polymorphism, scaled by caste. Used by the tube-bore gate (K2.2).
    #[inline]
    pub fn body_size_mm(&self, cfg: &AntConfig) -> f32 {
        let base = cfg.worker_size_mm;
        match self.caste {
            AntCaste::Worker | AntCaste::Breeder => base,
            AntCaste::Queen => base * 1.3,
            AntCaste::Soldier => {
                if cfg.polymorphic {
                    base * 1.6
                } else {
                    base * 1.15
                }
            }
        }
    }

    #[inline]
    pub fn speed(&self, cfg: &AntConfig) -> f32 {
        match self.caste {
            AntCaste::Worker | AntCaste::Breeder => cfg.speed_worker,
            AntCaste::Soldier => cfg.speed_soldier,
            AntCaste::Queen => cfg.speed_queen,
        }
    }

    /// Layer the ant is currently following for gradient steering.
    #[inline]
    pub fn target_layer(&self) -> PheromoneLayer {
        match self.state {
            AntState::ReturningHome | AntState::StoringFood => PheromoneLayer::HomeTrail,
            _ => PheromoneLayer::FoodTrail,
        }
    }

    pub fn transition(&mut self, new_state: AntState) {
        if self.state != new_state {
            tracing::trace!(
                ant = self.id,
                from = ?self.state,
                to = ?new_state,
                "FSM transition"
            );
            self.state = new_state;
            self.state_timer = 0;
        }
    }
}

/// Stochastic direction selection via Ant Colony Optimization weighting.
///
/// `p(j) = (τ^α) * (η^β) / Σ k  (τ^α)*(η^β)` where τ is pheromone intensity and
/// η = `(1 + cos(angle - heading))` is a forward bias.
///
/// - With probability `exploration_rate`, returns a random heading.
/// - If no pheromone is sensed, picks a forward-biased random heading.
pub fn choose_direction(
    ant: &Ant,
    grid: &PheromoneGrid,
    cfg: &AntConfig,
    rng: &mut impl Rng,
) -> f32 {
    if rng.r#gen::<f32>() < cfg.exploration_rate {
        return rng.gen_range(0.0..std::f32::consts::TAU);
    }

    let samples = grid.sample_cone(
        ant.position,
        ant.heading,
        cfg.sense_angle.to_radians(),
        cfg.sense_radius as f32,
        ant.target_layer(),
    );

    let mut weighted: Vec<(f32, f32)> = Vec::with_capacity(samples.len());
    let mut total = 0.0f32;
    for (cell, intensity) in &samples {
        let delta = *cell - ant.position;
        if delta.length_squared() < 1e-6 {
            continue;
        }
        let angle = delta.y.atan2(delta.x);
        // forward-bias heuristic
        let bias = (1.0 + (angle - ant.heading).cos()).max(0.01);
        let tau = intensity.max(0.0);
        let w = (tau + 0.01).powf(cfg.alpha) * bias.powf(cfg.beta);
        weighted.push((angle, w));
        total += w;
    }

    if total <= 1e-6 || weighted.is_empty() {
        // No useful signal — wander forward with a small jitter.
        let jitter = rng.gen_range(-0.6..0.6);
        return ant.heading + jitter;
    }

    let mut pick = rng.r#gen::<f32>() * total;
    for (angle, w) in &weighted {
        pick -= *w;
        if pick <= 0.0 {
            return *angle;
        }
    }
    weighted.last().map(|(a, _)| *a).unwrap_or(ant.heading)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AntConfig;

    #[test]
    fn test_fsm_transitions() {
        let mut a = Ant::new_worker(1, 0, Vec2::new(5.0, 5.0), 0.0, 10.0);
        assert_eq!(a.state, AntState::Exploring);
        a.transition(AntState::FollowingTrail);
        assert_eq!(a.state, AntState::FollowingTrail);
        assert_eq!(a.state_timer, 0);
        a.transition(AntState::PickingUpFood);
        a.transition(AntState::ReturningHome);
        assert_eq!(a.target_layer(), PheromoneLayer::HomeTrail);
        a.transition(AntState::Exploring);
        assert_eq!(a.target_layer(), PheromoneLayer::FoodTrail);
    }

    #[test]
    fn direction_biased_toward_pheromone() {
        use rand::SeedableRng;
        use rand_chacha::ChaCha8Rng;
        let mut grid = PheromoneGrid::new(40, 40);
        // Deposit a strong trail east of the ant.
        for dx in 1..8 {
            grid.deposit(20 + dx, 20, PheromoneLayer::FoodTrail, 8.0, 10.0);
        }
        let ant = Ant::new_worker(1, 0, Vec2::new(20.5, 20.5), 0.0, 10.0);
        let mut cfg = AntConfig::default();
        cfg.exploration_rate = 0.0;
        let mut eastward = 0;
        let mut rng = ChaCha8Rng::seed_from_u64(7);
        for _ in 0..200 {
            let h = choose_direction(&ant, &grid, &cfg, &mut rng);
            // east == heading within ±pi/3
            if h.cos() > 0.5 {
                eastward += 1;
            }
        }
        assert!(eastward > 150, "expected eastward bias, got {}", eastward);
    }
}
