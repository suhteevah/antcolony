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
    /// Cross-species usurpation (B8): an attacker is channeling the enemy
    /// queen-kill. Exposed + interruptible — if the ant dies or is forced
    /// to Fleeing mid-channel, colony usurp progress resets. 05 Findings
    /// 8/9/10 (Johnson 2002 timing; Topoff & Zimmerli 1993 disguise).
    Usurping,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
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
    /// Designated raider: steered toward the enemy nest by `raid_seek_tick`
    /// and permitted to descend the enemy entrance. Default false; only set
    /// when `combat.raid_seeking_enabled`. Mirrors `is_avenger`.
    #[serde(default)]
    pub is_raider: bool,
    /// P7: the yellow-ant avatar. Exactly one ant may carry this flag
    /// at a time. Its heading is set directly by WASD input; the FSM
    /// does NOT override it while possessed.
    #[serde(default)]
    pub is_player: bool,
    /// P7: when `Some`, this ant is in a recruit bond — each tick its
    /// heading points at the leader's position (override of FSM).
    /// Queens and the player avatar ignore the bond.
    #[serde(default)]
    pub follow_leader: Option<u32>,
    /// Dig system: when an ant in `AntState::Digging` is at a Solid
    /// face, this counter accumulates per substep. Tile flips when it
    /// crosses `DIG_PROGRESS_THRESHOLD`. Zeroes when the ant moves off
    /// the face or transitions out of Digging. Replaces the previous
    /// instant-flip behavior with multi-substep excavation so the
    /// player can actually watch tunnels grow.
    #[serde(default)]
    pub dig_progress: u32,
    /// Dig system: target cell the ant is currently excavating, paired
    /// with `dig_progress`. `None` when not actively digging or when
    /// the target was just flipped to Empty.
    #[serde(default)]
    pub dig_target: Option<(u16, u16)>,
    /// Dig system: when true, the ant is carrying a soil pellet from
    /// an excavation site to the surface kickout zone. Mirrors
    /// `food_carried` semantically. Set when a tile flips Solid→Empty
    /// (digger picks up the pellet); cleared when the ant drops at
    /// the kickout zone. See docs/biology.md "Soil pellets, not
    /// grains" + "Kickout mound" entries.
    #[serde(default)]
    pub carrying_soil: bool,
    /// Per-ant ACO modulators set by the per-ant brain (Phase 2). With
    /// default values (1.0, 1.0, 0.0, 1.0, 0.0), `choose_direction`
    /// reduces to the pre-Phase-1 ACO formula bit-for-bit. See
    /// `ai::observation::AntModulators`.
    #[serde(default)]
    pub modulators: crate::ai::observation::AntModulators,
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
            is_raider: false,
            is_player: false,
            follow_leader: None,
            dig_progress: 0,
            dig_target: None,
            carrying_soil: false,
            modulators: crate::ai::observation::AntModulators::default(),
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
///
/// Modulator wiring: `alpha_eff = cfg.alpha * mods.alpha_mult`,
/// `beta_eff = cfg.beta * mods.beta_mult`,
/// `explore_eff = cfg.exploration_rate + mods.exploration_mod`.
///
/// **Read-side clamps** here ([0.1, 10.0] for alpha/beta, [0.0, 1.0] for explore)
/// are defense-in-depth — they catch out-of-range values that bypass the
/// stricter write-side clamps in `Simulation::apply_ant_modulators` ([0.1, 5.0]
/// for alpha/beta, [-0.1, 0.1] for exploration_mod). The two clamps are
/// intentionally different: write-side is the trainer-facing contract, read-side
/// is the "even if something gets through, don't blow up" backstop.
///
/// Default modulators (1.0, 1.0, 0.0, ...) reduce to the pre-plumbing formula
/// EXACTLY for any species with `cfg.alpha` and `cfg.beta` already inside the
/// read-clamp range (which all current species satisfy: alpha=1.0, beta=2.0).
/// If a future species sets `cfg.alpha < 0.1` or `cfg.beta < 0.1`, the read
/// clamp will floor it and the "default = pre-plumbing" guarantee will quietly
/// break for that species. Verify in `species.rs` / TOML if you're adding one.
pub fn choose_direction(
    ant: &Ant,
    grid: &PheromoneGrid,
    cfg: &AntConfig,
    rng: &mut impl Rng,
) -> f32 {
    let mods = &ant.modulators;
    let alpha_eff = (cfg.alpha * mods.alpha_mult).clamp(0.1, 10.0);
    let beta_eff = (cfg.beta * mods.beta_mult).clamp(0.1, 10.0);
    let explore_eff = (cfg.exploration_rate + mods.exploration_mod).clamp(0.0, 1.0);

    if rng.r#gen::<f32>() < explore_eff {
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
        let bias = (1.0 + (angle - ant.heading).cos()).max(0.01);
        let tau = intensity.max(0.0);
        let w = (tau + 0.01).powf(alpha_eff) * bias.powf(beta_eff);
        weighted.push((angle, w));
        total += w;
    }

    if total <= 1e-6 || weighted.is_empty() {
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
    fn new_ant_has_default_modulators() {
        // Plumbing check: `new_worker` (and its delegate `new_with_caste`) must
        // wire the field. Identity-correctness of `AntModulators::default()`
        // itself is verified by `ai::observation::tests::modulators_default_is_identity`.
        let a = Ant::new_worker(1, 0, Vec2::new(5.0, 5.0), 0.0, 10.0);
        assert_eq!(
            a.modulators,
            crate::ai::observation::AntModulators::default()
        );
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

    #[test]
    fn default_modulators_reproduce_baseline_direction() {
        use rand::SeedableRng;
        use rand_chacha::ChaCha8Rng;

        let mut grid = PheromoneGrid::new(40, 40);
        for dx in 1..8 {
            grid.deposit(20 + dx, 20, PheromoneLayer::FoodTrail, 8.0, 10.0);
        }
        let ant = Ant::new_worker(1, 0, Vec2::new(20.5, 20.5), 0.0, 10.0);
        let mut cfg = AntConfig::default();
        cfg.exploration_rate = 0.0;

        let mut rng = ChaCha8Rng::seed_from_u64(0xa17_de_f);
        let mut eastward = 0;
        for _ in 0..100 {
            let h = choose_direction(&ant, &grid, &cfg, &mut rng);
            if h.cos() > 0.0 {
                eastward += 1;
            }
        }
        assert!(
            eastward >= 70,
            "default modulators should give baseline behavior (≥70/100 eastward), got {}",
            eastward
        );
    }

    #[test]
    fn high_alpha_mult_strengthens_pheromone_following() {
        use rand::SeedableRng;
        use rand_chacha::ChaCha8Rng;

        // Build a grid where east (dy=0) and NE (dy=2) trails compete.
        // Cells at (dx, 2) relative to the ant are at ~34–53° off heading=0,
        // so `h.cos() > 0.85` (≈ ±32°) will only pass exactly-east returns.
        //
        // East intensity 2.5, NE intensity 2.0 gives a 1.25:1 pheromone contrast.
        // With alpha=1: weight ratio ≈ (2.51)^1 / (2.01)^1 ≈ 1.25 (east mildly preferred).
        // With alpha=5: weight ratio ≈ (2.51)^5 / (2.01)^5 ≈ 98.8 / 33.1 ≈ 2.98x (east
        //   strongly preferred), so the ant picks exactly-east much more consistently.
        //
        // 200 trials per arm + ≥10-sample margin (5pp) reliably measures alpha
        // amplification rather than noise. Seeds are re-used so the only variable is alpha.
        let mut grid = PheromoneGrid::new(40, 40);
        // East trail (dy=0): heading=0 points directly at these.
        for dx in 1..6 {
            grid.deposit(20 + dx, 20, PheromoneLayer::FoodTrail, 2.5, 10.0);
        }
        // NE trail: cells at dy=2 are ~34–63° off east — far enough outside the
        // h.cos() > 0.85 filter that NE picks don't inflate the exactly-east count.
        for dx in 1..5 {
            grid.deposit(20 + dx, 20 + 2, PheromoneLayer::FoodTrail, 2.0, 10.0);
        }

        let mut ant = Ant::new_worker(1, 0, Vec2::new(20.5, 20.5), 0.0, 10.0);
        let mut cfg = AntConfig::default();
        cfg.exploration_rate = 0.0;

        ant.modulators.alpha_mult = 5.0;
        let mut rng = ChaCha8Rng::seed_from_u64(0x57e1_a17);
        let mut exact_east_high = 0;
        for _ in 0..200 {
            let h = choose_direction(&ant, &grid, &cfg, &mut rng);
            // h.cos() > 0.85 ≈ ±32°: only truly east headings pass.
            // NE cells at dy=2 are ~34–63° off east so they don't inflate the count.
            if h.cos() > 0.85 {
                exact_east_high += 1;
            }
        }

        ant.modulators.alpha_mult = 1.0;
        let mut rng = ChaCha8Rng::seed_from_u64(0x57e1_a17);
        let mut eastward_base = 0;
        for _ in 0..200 {
            let h = choose_direction(&ant, &grid, &cfg, &mut rng);
            if h.cos() > 0.85 {
                eastward_base += 1;
            }
        }

        // Strict-greater would pass at +1 sample, which could happen by RNG drift
        // alone. Require a meaningful margin (≥10 / 200 samples = 5pp) so this
        // genuinely measures alpha amplification, not noise.
        assert!(
            exact_east_high >= eastward_base + 10,
            "alpha_mult=5 should pull east meaningfully more than alpha_mult=1 (got {} vs {}, margin {})",
            exact_east_high,
            eastward_base,
            (exact_east_high as i32) - (eastward_base as i32),
        );
    }

    #[test]
    fn exploration_mod_zero_preserves_exploration() {
        use rand::SeedableRng;
        use rand_chacha::ChaCha8Rng;

        let grid = PheromoneGrid::new(10, 10);
        let ant = Ant::new_worker(1, 0, Vec2::new(5.0, 5.0), 0.0, 10.0);
        let mut cfg = AntConfig::default();
        cfg.exploration_rate = 1.0;
        let mut rng = ChaCha8Rng::seed_from_u64(0x6e0_77);

        let mut headings = Vec::new();
        for _ in 0..50 {
            headings.push(choose_direction(&ant, &grid, &cfg, &mut rng));
        }
        let mean = headings.iter().sum::<f32>() / headings.len() as f32;
        let variance: f32 =
            headings.iter().map(|h| (h - mean).powi(2)).sum::<f32>() / headings.len() as f32;
        assert!(
            variance > 1.0,
            "uniform-random heading should have wide variance, got {}",
            variance
        );
    }
}
