//! AI brain trait — pluggable per-colony decision policies.
//!
//! # Why a trait
//!
//! The pre-Phase-B AI was a single hardcoded heuristic baked into
//! `Simulation::red_ai_tick` — fine for "AI vs human" but not extensible.
//! For AI-vs-AI matchups, learned policies (Aether LM checkpoints), and
//! eventually external-model integration, we need a clean swap point.
//!
//! Three implementations ship here:
//! - [`HeuristicBrain`] — the original `red_ai_tick` logic refactored as
//!   a brain. Baseline; deterministic; no model.
//! - [`RandomBrain`] — uniformly random valid decisions. Useful as the
//!   noise-floor opponent in matchup benches and as a smoke test for
//!   the trait itself.
//! - [`AetherLmBrain`] — *stub*. Loads an Aether-LM checkpoint and runs
//!   inference per decision tick. Integration point exists; the actual
//!   model call is `todo!()` until we have a trained checkpoint.
//!
//! # Decision cadence
//!
//! Brains decide once per outer tick (not per substep). The
//! [`AiDecision`] is then applied to the colony's caste ratio + behavior
//! weights + tech-unlock pipeline.

use crate::colony::TechUnlock;

/// Compact game-state snapshot the brain consumes per decision tick.
/// Fixed-size feature vector — model-friendly, no allocation per tick.
///
/// Add fields with care: every `AiBrain` impl reads this struct, and
/// learned-model checkpoints are tied to its layout. Bumping the layout
/// invalidates trained checkpoints.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ColonyAiState {
    /// Stored food units in this colony's storage chamber.
    pub food_stored: f32,
    /// Rolling average of food returned to nest by foragers (sim-internal).
    pub food_inflow_recent: f32,
    /// Adult workers on roster.
    pub worker_count: u32,
    /// Adult soldiers on roster.
    pub soldier_count: u32,
    /// Adult breeders (alates).
    pub breeder_count: u32,
    /// Eggs in brood pool.
    pub brood_egg: u32,
    /// Larvae in brood pool.
    pub brood_larva: u32,
    /// Pupae in brood pool.
    pub brood_pupa: u32,
    /// Live queens in this colony.
    pub queens_alive: u32,
    /// Combat losses suffered by this colony in the last tick.
    pub combat_losses_recent: u32,
    /// Distance (in tiles) to the nearest known enemy ant. f32::INFINITY
    /// when no enemy has been sensed.
    pub enemy_distance_min: f32,
    /// Number of enemy workers currently on the same module.
    pub enemy_worker_count: u32,
    /// Number of enemy soldiers currently on the same module.
    pub enemy_soldier_count: u32,
    /// In-game day of year (0-364).
    pub day_of_year: u32,
    /// In-game ambient temperature (°C).
    pub ambient_temp_c: f32,
    /// True if the colony is currently in seasonal diapause.
    pub diapause_active: bool,
    /// True if it's daytime in-game (06:00-18:00 sim time).
    pub is_daytime: bool,
}

/// What the brain decides per tick. All fields are bounded; values
/// outside the documented range are clamped by the caller.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AiDecision {
    /// Worker share of newly-laid brood [0.0..1.0].
    pub caste_ratio_worker: f32,
    /// Soldier share [0.0..1.0]. worker+soldier+breeder must sum to 1.0
    /// (the caller renormalizes).
    pub caste_ratio_soldier: f32,
    /// Breeder share [0.0..1.0].
    pub caste_ratio_breeder: f32,
    /// Behavior weight: how much of the workforce prioritizes foraging
    /// [0.0..1.0]. The three weights are renormalized by the caller.
    pub forage_weight: f32,
    /// Behavior weight: digging.
    pub dig_weight: f32,
    /// Behavior weight: nursing brood.
    pub nurse_weight: f32,
    /// Optional tech the brain wants to research/unlock this tick.
    /// Most brains return None most ticks; PvP-mode brains may pace
    /// research as a strategic decision.
    pub research_choice: Option<TechUnlock>,
}

impl AiDecision {
    /// Trivial sanity check used by tests.
    pub fn is_valid(&self) -> bool {
        let castes = [
            self.caste_ratio_worker,
            self.caste_ratio_soldier,
            self.caste_ratio_breeder,
        ];
        let weights = [self.forage_weight, self.dig_weight, self.nurse_weight];
        castes.iter().chain(weights.iter()).all(|v| v.is_finite() && (0.0..=1.0).contains(v))
    }
}

/// Pluggable per-colony AI brain. One instance per AI-controlled colony;
/// `decide` is called once per outer tick.
///
/// Implementations MUST be deterministic given a fixed RNG seed when
/// they consume randomness, so AI-vs-AI bench runs are reproducible.
pub trait AiBrain: Send + Sync {
    /// Plain-English brain identifier. Surfaces in logs, bench reports,
    /// and self-play data files.
    fn name(&self) -> &str;

    /// Compute the decision for this tick from the colony's state.
    fn decide(&mut self, state: &ColonyAiState) -> AiDecision;
}

// ============================================================
// HeuristicBrain — the legacy `red_ai_tick` rules as a brain.
// ============================================================

/// Reactive rule-based brain. Mirrors the pre-trait `Simulation::red_ai_tick`
/// semantics: under combat losses → escalate soldier ratio; under low
/// food → all-hands-foraging.
///
/// Deterministic; no RNG.
#[derive(Debug, Clone)]
pub struct HeuristicBrain {
    /// Snapshot of the brain's CURRENT internal weights (mutated
    /// every `decide()` so subsequent decisions build on the prior
    /// state, just like `red_ai_tick` does).
    state: BrainInternalState,
    /// Soft food threshold below which the colony shifts toward
    /// forage-everything. Auto-derived from colony egg cost in the
    /// constructor; can be overridden.
    pub low_food_threshold: f32,
}

#[derive(Debug, Clone)]
struct BrainInternalState {
    caste_ratio_worker: f32,
    caste_ratio_soldier: f32,
    caste_ratio_breeder: f32,
    forage_weight: f32,
    dig_weight: f32,
    nurse_weight: f32,
}

impl Default for BrainInternalState {
    fn default() -> Self {
        Self {
            caste_ratio_worker: 0.65,
            caste_ratio_soldier: 0.30,
            caste_ratio_breeder: 0.05,
            forage_weight: 0.55,
            dig_weight: 0.20,
            nurse_weight: 0.25,
        }
    }
}

impl HeuristicBrain {
    /// Build with sensible defaults. `egg_cost` is the colony's
    /// `egg_cost_food` value; the brain treats `egg_cost * 4` as
    /// the "low food" threshold (matches legacy `red_ai_tick`).
    pub fn new(egg_cost: f32) -> Self {
        Self {
            state: BrainInternalState::default(),
            low_food_threshold: egg_cost * 4.0,
        }
    }
}

impl AiBrain for HeuristicBrain {
    fn name(&self) -> &str {
        "heuristic"
    }

    fn decide(&mut self, state: &ColonyAiState) -> AiDecision {
        // Rule 1: combat losses → push soldier ratio up.
        if state.combat_losses_recent > 0 {
            let shift = 0.01 * state.combat_losses_recent as f32;
            let target = (self.state.caste_ratio_soldier + shift).min(0.5);
            let delta = target - self.state.caste_ratio_soldier;
            self.state.caste_ratio_soldier = target;
            self.state.caste_ratio_worker =
                (self.state.caste_ratio_worker - delta).max(0.05);
        }

        // Rule 2: food low → forage-everything.
        if state.food_stored < self.low_food_threshold {
            let shift = 0.02;
            let target = (self.state.forage_weight + shift).min(0.9);
            let delta = target - self.state.forage_weight;
            self.state.forage_weight = target;
            self.state.nurse_weight = (self.state.nurse_weight - delta * 0.5).max(0.05);
            self.state.dig_weight = (self.state.dig_weight - delta * 0.5).max(0.02);
        }

        AiDecision {
            caste_ratio_worker: self.state.caste_ratio_worker,
            caste_ratio_soldier: self.state.caste_ratio_soldier,
            caste_ratio_breeder: self.state.caste_ratio_breeder,
            forage_weight: self.state.forage_weight,
            dig_weight: self.state.dig_weight,
            nurse_weight: self.state.nurse_weight,
            research_choice: None,
        }
    }
}

// ============================================================
// Archetype brains — fixed-personality heuristics for tournament
// diversity. Each one has a distinct strategic identity. Together
// they form the matchup pool that produces a diverse training
// corpus for behavior cloning, instead of bootstrapping from one
// heuristic against itself.
//
// Each archetype carries:
// - a default (caste_ratio, behavior_weights) prior expressing its
//   strategic identity
// - reaction rules that adjust those priors in response to combat
//   losses, food shortage, or enemy proximity
// - a stable name for tournament reporting
// ============================================================

/// Internal helper — common reaction-rule shape for archetype brains.
fn archetype_decide(
    base: &mut BrainInternalState,
    state: &ColonyAiState,
    losses_response: f32, // 0.0 = ignore losses, 1.0 = max escalation
    food_response: f32,   // 0.0 = ignore low food, 1.0 = max forage
    food_threshold: f32,
) -> AiDecision {
    if state.combat_losses_recent > 0 && losses_response > 0.0 {
        let shift = 0.01 * state.combat_losses_recent as f32 * losses_response;
        let target = (base.caste_ratio_soldier + shift).min(0.7);
        let delta = target - base.caste_ratio_soldier;
        base.caste_ratio_soldier = target;
        base.caste_ratio_worker = (base.caste_ratio_worker - delta).max(0.05);
    }
    if state.food_stored < food_threshold && food_response > 0.0 {
        let shift = 0.02 * food_response;
        let target = (base.forage_weight + shift).min(0.95);
        let delta = target - base.forage_weight;
        base.forage_weight = target;
        base.nurse_weight = (base.nurse_weight - delta * 0.5).max(0.05);
        base.dig_weight = (base.dig_weight - delta * 0.5).max(0.02);
    }
    AiDecision {
        caste_ratio_worker: base.caste_ratio_worker,
        caste_ratio_soldier: base.caste_ratio_soldier,
        caste_ratio_breeder: base.caste_ratio_breeder,
        forage_weight: base.forage_weight,
        dig_weight: base.dig_weight,
        nurse_weight: base.nurse_weight,
        research_choice: None,
    }
}

/// Defender — fortified turtle. High soldier baseline + nurse-heavy.
/// Mild reaction to losses (already prepared); slow to shift.
pub struct DefenderBrain { state: BrainInternalState }
impl DefenderBrain {
    pub fn new() -> Self {
        Self { state: BrainInternalState {
            caste_ratio_worker: 0.50, caste_ratio_soldier: 0.45, caste_ratio_breeder: 0.05,
            forage_weight: 0.20, dig_weight: 0.10, nurse_weight: 0.70,
        } }
    }
}
impl Default for DefenderBrain { fn default() -> Self { Self::new() } }
impl AiBrain for DefenderBrain {
    fn name(&self) -> &str { "defender" }
    fn decide(&mut self, s: &ColonyAiState) -> AiDecision {
        archetype_decide(&mut self.state, s, 0.3, 0.5, 20.0)
    }
}

/// Aggressor — pushes the fight. Soldier-heavy, forage-heavy, fast
/// escalation on combat losses, all-hands-foraging when food drops.
pub struct AggressorBrain { state: BrainInternalState }
impl AggressorBrain {
    pub fn new() -> Self {
        Self { state: BrainInternalState {
            caste_ratio_worker: 0.30, caste_ratio_soldier: 0.65, caste_ratio_breeder: 0.05,
            forage_weight: 0.70, dig_weight: 0.10, nurse_weight: 0.20,
        } }
    }
}
impl Default for AggressorBrain { fn default() -> Self { Self::new() } }
impl AiBrain for AggressorBrain {
    fn name(&self) -> &str { "aggressor" }
    fn decide(&mut self, s: &ColonyAiState) -> AiDecision {
        archetype_decide(&mut self.state, s, 1.5, 1.0, 30.0)
    }
}

/// Economist — worker monoculture. Maximize food + worker count first,
/// late-tier soldier ramp only when actively invaded.
pub struct EconomistBrain { state: BrainInternalState }
impl EconomistBrain {
    pub fn new() -> Self {
        Self { state: BrainInternalState {
            caste_ratio_worker: 0.85, caste_ratio_soldier: 0.05, caste_ratio_breeder: 0.10,
            forage_weight: 0.85, dig_weight: 0.05, nurse_weight: 0.10,
        } }
    }
}
impl Default for EconomistBrain { fn default() -> Self { Self::new() } }
impl AiBrain for EconomistBrain {
    fn name(&self) -> &str { "economist" }
    fn decide(&mut self, s: &ColonyAiState) -> AiDecision {
        // Ignore early losses; only react when enemies are visible adjacent.
        let losses_resp = if s.enemy_distance_min < 5.0 { 1.5 } else { 0.0 };
        archetype_decide(&mut self.state, s, losses_resp, 0.3, 10.0)
    }
}

/// Breeder — alate factory, founds far-flung daughters. High breeder share.
pub struct BreederBrain { state: BrainInternalState }
impl BreederBrain {
    pub fn new() -> Self {
        Self { state: BrainInternalState {
            caste_ratio_worker: 0.55, caste_ratio_soldier: 0.05, caste_ratio_breeder: 0.40,
            forage_weight: 0.50, dig_weight: 0.20, nurse_weight: 0.30,
        } }
    }
}
impl Default for BreederBrain { fn default() -> Self { Self::new() } }
impl AiBrain for BreederBrain {
    fn name(&self) -> &str { "breeder" }
    fn decide(&mut self, s: &ColonyAiState) -> AiDecision {
        archetype_decide(&mut self.state, s, 0.5, 0.7, 25.0)
    }
}

/// Forager — pure pacifist economy. No soldiers ever. Maximum food
/// throughput. Loses to anything that fights but produces a clean
/// "what foraging looks like" signal in the training corpus.
pub struct ForagerBrain { state: BrainInternalState }
impl ForagerBrain {
    pub fn new() -> Self {
        Self { state: BrainInternalState {
            caste_ratio_worker: 0.95, caste_ratio_soldier: 0.00, caste_ratio_breeder: 0.05,
            forage_weight: 0.90, dig_weight: 0.05, nurse_weight: 0.05,
        } }
    }
}
impl Default for ForagerBrain { fn default() -> Self { Self::new() } }
impl AiBrain for ForagerBrain {
    fn name(&self) -> &str { "forager" }
    fn decide(&mut self, s: &ColonyAiState) -> AiDecision {
        // Even under attack, forager refuses to make soldiers.
        archetype_decide(&mut self.state, s, 0.0, 1.0, 30.0)
    }
}

/// Conservative Builder — infrastructure first, slow reaction.
/// Heavy on dig + nurse, modest forage + soldier baseline.
pub struct ConservativeBuilderBrain { state: BrainInternalState }
impl ConservativeBuilderBrain {
    pub fn new() -> Self {
        Self { state: BrainInternalState {
            caste_ratio_worker: 0.70, caste_ratio_soldier: 0.20, caste_ratio_breeder: 0.10,
            forage_weight: 0.30, dig_weight: 0.30, nurse_weight: 0.40,
        } }
    }
}
impl Default for ConservativeBuilderBrain { fn default() -> Self { Self::new() } }
impl AiBrain for ConservativeBuilderBrain {
    fn name(&self) -> &str { "conservative" }
    fn decide(&mut self, s: &ColonyAiState) -> AiDecision {
        archetype_decide(&mut self.state, s, 0.3, 0.4, 15.0)
    }
}

/// TunedBrain — parameterized archetype. Same shape as the named
/// archetypes (9 floats: caste W/S/B + behavior F/D/N + reaction
/// LR/FR/FT) but constructed from explicit values instead of a
/// hardcoded identity. Used by the variant-tournament pipeline to
/// spin up perturbed strategy variants without recompiling.
pub struct TunedBrain {
    label: String,
    state: BrainInternalState,
    losses_response: f32,
    food_response: f32,
    food_threshold: f32,
}

impl TunedBrain {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        label: impl Into<String>,
        worker: f32, soldier: f32, breeder: f32,
        forage: f32, dig: f32, nurse: f32,
        losses_response: f32, food_response: f32, food_threshold: f32,
    ) -> Self {
        Self {
            label: label.into(),
            state: BrainInternalState {
                caste_ratio_worker: worker,
                caste_ratio_soldier: soldier,
                caste_ratio_breeder: breeder,
                forage_weight: forage,
                dig_weight: dig,
                nurse_weight: nurse,
            },
            losses_response,
            food_response,
            food_threshold,
        }
    }
}

impl AiBrain for TunedBrain {
    fn name(&self) -> &str { &self.label }
    fn decide(&mut self, s: &ColonyAiState) -> AiDecision {
        archetype_decide(&mut self.state, s, self.losses_response, self.food_response, self.food_threshold)
    }
}

/// Archetype identifier for SpeciesBrain blending. Matches the named
/// archetype brains 1:1 — the SpeciesBrain blends the species' biological
/// baseline with the archetype's strategic posture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrainArchetype {
    Heuristic,
    Defender,
    Aggressor,
    Economist,
    Breeder,
    Forager,
    Conservative,
}

impl BrainArchetype {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "heuristic" => Some(Self::Heuristic),
            "defender" => Some(Self::Defender),
            "aggressor" => Some(Self::Aggressor),
            "economist" => Some(Self::Economist),
            "breeder" => Some(Self::Breeder),
            "forager" => Some(Self::Forager),
            "conservative" => Some(Self::Conservative),
            _ => None,
        }
    }

    /// Returns the 9-tuple of strategic posture parameters
    /// (caste W/S/B + behavior F/D/N + losses_response + food_response + food_threshold).
    fn params(self) -> [f32; 9] {
        match self {
            Self::Heuristic    => [0.65, 0.30, 0.05, 0.55, 0.20, 0.25, 1.0, 1.0, 20.0],
            Self::Defender     => [0.50, 0.45, 0.05, 0.20, 0.10, 0.70, 0.3, 0.5, 20.0],
            Self::Aggressor    => [0.30, 0.65, 0.05, 0.70, 0.10, 0.20, 1.5, 1.0, 30.0],
            Self::Economist    => [0.85, 0.05, 0.10, 0.85, 0.05, 0.10, 0.0, 0.3, 10.0],
            Self::Breeder      => [0.55, 0.05, 0.40, 0.50, 0.20, 0.30, 0.5, 0.7, 25.0],
            Self::Forager      => [0.95, 0.00, 0.05, 0.90, 0.05, 0.05, 0.0, 1.0, 30.0],
            Self::Conservative => [0.70, 0.20, 0.10, 0.30, 0.30, 0.40, 0.3, 0.4, 15.0],
        }
    }
}

/// SpeciesBrain — biology-grounded archetype. Blends a species' baseline
/// (derived from cited fields in its TOML: aggression, recruitment style,
/// queen_eggs_per_day, default_caste_ratio) with a strategic archetype
/// overlay. The blend lets, say, a Camponotus play "economist" mode while
/// still preserving its biologically-mandated 10% major caste, and lets a
/// Lasius play "aggressor" mode while still respecting its low aggression
/// (0.2). Every brain in the resulting pool is biologically defensible.
///
/// Construction:
/// - `SpeciesBrain::from_species(species, archetype, blend)` for an
///   already-loaded `Species`.
/// - `SpeciesBrain::from_toml_path(path, archetype, blend)` for the
///   common case of loading from disk.
///
/// `blend` ∈ [0.0, 1.0]: 0.0 = pure species baseline, 1.0 = pure
/// archetype, 0.5 = balanced blend.
pub struct SpeciesBrain {
    inner: TunedBrain,
}

impl SpeciesBrain {
    pub fn from_species(species: &crate::species::Species, archetype: BrainArchetype, blend: f32) -> Self {
        let baseline = species_baseline_params(species);
        let overlay = archetype.params();
        let alpha = blend.clamp(0.0, 1.0);
        let mut p = [0.0_f32; 9];
        for i in 0..9 {
            p[i] = (1.0 - alpha) * baseline[i] + alpha * overlay[i];
        }
        // Renormalize caste triple (W/S/B) and behavior triple (F/D/N) so
        // each sums to 1.0 — sim invariant.
        let (cw, cs, cb) = renormalize_triple(p[0], p[1], p[2]);
        let (bf, bd, bn) = renormalize_triple(p[3], p[4], p[5]);
        p[0] = cw; p[1] = cs; p[2] = cb;
        p[3] = bf; p[4] = bd; p[5] = bn;
        let label = format!("{}__{:?}", species.id, archetype);
        Self {
            inner: TunedBrain::new(label, p[0], p[1], p[2], p[3], p[4], p[5], p[6], p[7], p[8]),
        }
    }

    pub fn from_toml_path<P: AsRef<std::path::Path>>(
        path: P, archetype: BrainArchetype, blend: f32,
    ) -> Result<Self, crate::error::SimError> {
        let species = crate::species::Species::load_from_file(path)?;
        Ok(Self::from_species(&species, archetype, blend))
    }
}

impl AiBrain for SpeciesBrain {
    fn name(&self) -> &str { self.inner.name() }
    fn decide(&mut self, s: &ColonyAiState) -> AiDecision { self.inner.decide(s) }
}

/// Derive a 9-tuple baseline from a species' cited biological fields.
/// This is the same mapping documented in `scripts/derive_species_brains.py`,
/// kept in lockstep with that script — every value here traces to a
/// citation in the species TOML's source comments.
///
/// Phase B hook #14 — `ecological_role.invasive_status` modulates the
/// strategic baseline. Invasive pests (Linepithema, Solenopsis invicta)
/// get a 30% expansion+aggression bias; introduced species (Lasius
/// niger in NA, Tetramorium immigrans) get 10%. This is what drives
/// real-world displacement — invasives outcompete natives by being
/// more expansion-aggressive, not by being individually stronger ants.
fn species_baseline_params(sp: &crate::species::Species) -> [f32; 9] {
    use crate::species_extended::{InvasiveStatus, RecruitmentStyle};
    let aggression = sp.combat.aggression;
    let eggs = sp.growth.queen_eggs_per_day;
    let recruitment = sp.behavior.recruitment;
    let cr = &sp.default_caste_ratio;
    let invasive_bias: f32 = match sp.ecological_role.invasive_status {
        InvasiveStatus::InvasivePest => 1.3,
        InvasiveStatus::Introduced => 1.1,
        InvasiveStatus::Native => 1.0,
    };

    // Recruitment style → forage commitment (modulated by invasive bias)
    let forage_base = match recruitment {
        RecruitmentStyle::Mass => 0.70,
        RecruitmentStyle::Group => 0.55,
        RecruitmentStyle::TandemRun => 0.45,
        RecruitmentStyle::Individual => 0.40,
    };
    let forage_raw = (forage_base * invasive_bias).min(0.95);
    // Substrate / mound construction → dig commitment. Indexed by species
    // id since this is per-species ecology, not a generic field.
    let dig_raw = match sp.id.as_str() {
        "lasius_niger" => 0.20,
        "camponotus_pennsylvanicus" => 0.30,
        "formica_rufa" => 0.30,
        "pogonomyrmex_occidentalis" => 0.25,
        "tetramorium_immigrans" => 0.15,
        "tapinoma_sessile" => 0.10,
        "aphaenogaster_rudis" => 0.20,
        _ => 0.20,
    };
    // Egg-lay rate → nurse commitment (high lay rate needs more nurses).
    let nurse_raw = if eggs >= 40.0 { 0.45 }
                    else if eggs >= 25.0 { 0.30 }
                    else if eggs >= 15.0 { 0.22 }
                    else { 0.18 };
    let total = forage_raw + dig_raw + nurse_raw;
    let (f, d, n) = (forage_raw / total, dig_raw / total, nurse_raw / total);

    [
        cr.worker, cr.soldier, cr.breeder,
        f, d, n,
        (aggression * 2.0 * invasive_bias).min(3.0),  // losses_response: 2× cited aggression × invasive bias
        (1.0 - aggression).max(0.1),  // food_response: low-aggression species relocate sooner
        20.0,                          // food_threshold: legacy egg_cost × 4
    ]
}

fn renormalize_triple(a: f32, b: f32, c: f32) -> (f32, f32, f32) {
    let s = a + b + c;
    if s > 0.0 { (a / s, b / s, c / s) } else { (1.0/3.0, 1.0/3.0, 1.0/3.0) }
}

// ============================================================
// RandomBrain — noise-floor opponent / smoke test.
// ============================================================

/// Uniformly-random decisions in the legal value bands. Seeded for
/// reproducibility. NOT a strong opponent — by design — it's the
/// matchup-bench's "noise floor" so we can measure how much any other
/// brain is actually beating chance.
#[derive(Debug)]
pub struct RandomBrain {
    rng: rand_chacha::ChaCha8Rng,
}

impl RandomBrain {
    pub fn new(seed: u64) -> Self {
        use rand::SeedableRng;
        Self {
            rng: rand_chacha::ChaCha8Rng::seed_from_u64(seed),
        }
    }
}

impl AiBrain for RandomBrain {
    fn name(&self) -> &str {
        "random"
    }

    fn decide(&mut self, _state: &ColonyAiState) -> AiDecision {
        use rand::Rng;
        // Caste ratios: pick three random draws and renormalize so they sum to 1.
        let a: f32 = self.rng.r#gen();
        let b: f32 = self.rng.r#gen();
        let c: f32 = self.rng.r#gen();
        let sum = (a + b + c).max(1e-6);
        // Behavior weights: same trick.
        let f: f32 = self.rng.r#gen();
        let d: f32 = self.rng.r#gen();
        let n: f32 = self.rng.r#gen();
        let wsum = (f + d + n).max(1e-6);
        AiDecision {
            caste_ratio_worker: a / sum,
            caste_ratio_soldier: b / sum,
            caste_ratio_breeder: c / sum,
            forage_weight: f / wsum,
            dig_weight: d / wsum,
            nurse_weight: n / wsum,
            research_choice: None,
        }
    }
}

// ============================================================
// MlpBrain — pure-Rust MLP forward pass, GPU-trained weights.
// ============================================================

/// Compact MLP brain. Trained externally (`scripts/train_mlp_brain.py`,
/// PyTorch on CUDA), checkpoint loaded as JSON, forward pass in pure
/// Rust at inference time. No subprocess overhead, no model framework
/// at runtime — just matrix-vector math.
///
/// Architecture: 17 (state features) -> hidden -> hidden -> 6 (decision)
/// with ReLU between layers and sigmoid on output. Outputs are
/// renormalized by `apply_ai_decision` so the brain doesn't have to
/// emit perfectly-summing values.
///
/// # File format
///
/// JSON object with shape:
/// ```text
/// {
///   "input_dim":  17, "hidden_dim": 64, "output_dim": 6,
///   "input_mean": [...17 floats...],   // z-score normalization
///   "input_std":  [...17 floats...],
///   "w1":  [[...]], "b1": [...],   // 17 -> hidden (row-major)
///   "w2":  [[...]], "b2": [...],   // hidden -> hidden
///   "w3":  [[...]], "b3": [...]    // hidden -> 6
/// }
/// ```
pub struct MlpBrain {
    pub label: String,
    input_mean: Vec<f32>,
    input_std: Vec<f32>,
    w1: Vec<Vec<f32>>, b1: Vec<f32>,
    w2: Vec<Vec<f32>>, b2: Vec<f32>,
    w3: Vec<Vec<f32>>, b3: Vec<f32>,
    /// Training-time exploration noise (sigma for additive Gaussian).
    /// Default 0.0 (deterministic). Set via set_explore_std() or the
    /// `noisy_mlp:<path>:<std>` matchup_bench spec.
    explore_std: f32,
    rng: rand_chacha::ChaCha8Rng,
}

#[derive(serde::Deserialize)]
struct MlpWeightsFile {
    #[allow(dead_code)] input_dim: usize,
    #[allow(dead_code)] hidden_dim: usize,
    #[allow(dead_code)] output_dim: usize,
    input_mean: Vec<f32>,
    input_std: Vec<f32>,
    w1: Vec<Vec<f32>>, b1: Vec<f32>,
    w2: Vec<Vec<f32>>, b2: Vec<f32>,
    w3: Vec<Vec<f32>>, b3: Vec<f32>,
}

impl MlpBrain {
    /// Load from a JSON weights file produced by `train_mlp_brain.py`.
    pub fn load(path: impl AsRef<std::path::Path>, label: impl Into<String>) -> std::io::Result<Self> {
        let path = path.as_ref();
        let raw = std::fs::read_to_string(path)?;
        let parsed: MlpWeightsFile = serde_json::from_str(&raw)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        use rand::SeedableRng;
        Ok(Self {
            label: label.into(),
            input_mean: parsed.input_mean,
            input_std: parsed.input_std,
            w1: parsed.w1, b1: parsed.b1,
            w2: parsed.w2, b2: parsed.b2,
            w3: parsed.w3, b3: parsed.b3,
            explore_std: 0.0,
            rng: rand_chacha::ChaCha8Rng::seed_from_u64(0xfeed),
        })
    }
}

/// matmul + bias + ReLU. `w` is row-major (each inner vec = one output row).
fn relu_layer(input: &[f32], w: &[Vec<f32>], b: &[f32]) -> Vec<f32> {
    let mut out = vec![0.0f32; w.len()];
    for (i, row) in w.iter().enumerate() {
        let mut acc = b[i];
        for (j, x) in input.iter().enumerate() {
            acc += row[j] * x;
        }
        out[i] = if acc > 0.0 { acc } else { 0.0 };
    }
    out
}

fn sigmoid_layer(input: &[f32], w: &[Vec<f32>], b: &[f32]) -> Vec<f32> {
    let mut out = vec![0.0f32; w.len()];
    for (i, row) in w.iter().enumerate() {
        let mut acc = b[i];
        for (j, x) in input.iter().enumerate() {
            acc += row[j] * x;
        }
        out[i] = 1.0 / (1.0 + (-acc).exp());
    }
    out
}

fn state_to_features(s: &ColonyAiState) -> Vec<f32> {
    let ed = if s.enemy_distance_min.is_finite() {
        s.enemy_distance_min
    } else {
        1e6
    };
    vec![
        s.food_stored, s.food_inflow_recent,
        s.worker_count as f32, s.soldier_count as f32, s.breeder_count as f32,
        s.brood_egg as f32, s.brood_larva as f32, s.brood_pupa as f32,
        s.queens_alive as f32, s.combat_losses_recent as f32,
        ed, s.enemy_worker_count as f32, s.enemy_soldier_count as f32,
        s.day_of_year as f32, s.ambient_temp_c,
        if s.diapause_active { 1.0 } else { 0.0 },
        if s.is_daytime { 1.0 } else { 0.0 },
    ]
}

impl MlpBrain {
    /// Set training-time exploration noise. Std is the per-dimension
    /// Gaussian noise added to the sigmoid output (clamped to [0,1]).
    /// 0.0 = deterministic (default, eval mode). >0.0 = stochastic
    /// (PPO/REINFORCE training mode).
    pub fn set_explore_std(&mut self, std: f32) {
        self.explore_std = std;
    }
}

impl AiBrain for MlpBrain {
    fn name(&self) -> &str {
        &self.label
    }

    fn decide(&mut self, state: &ColonyAiState) -> AiDecision {
        let raw = state_to_features(state);
        // z-score normalization
        let normalized: Vec<f32> = raw
            .iter()
            .zip(self.input_mean.iter())
            .zip(self.input_std.iter())
            .map(|((x, m), s)| (x - m) / s)
            .collect();
        let h1 = relu_layer(&normalized, &self.w1, &self.b1);
        let h2 = relu_layer(&h1, &self.w2, &self.b2);
        let mut out = sigmoid_layer(&h2, &self.w3, &self.b3);
        // Optional exploration noise (PPO training-time only). Each
        // dimension gets independent Gaussian noise; result clamped
        // to [0, 1] so caste/behavior triples stay in valid range
        // post-normalization in the sim.
        if self.explore_std > 0.0 {
            use rand::Rng;
            for x in out.iter_mut() {
                let z: f32 = self.rng.r#gen::<f32>() * 2.0 - 1.0;  // uniform [-1, 1]
                let noise = z * self.explore_std;
                *x = (*x + noise).clamp(0.0, 1.0);
            }
        }
        AiDecision {
            caste_ratio_worker: out[0],
            caste_ratio_soldier: out[1],
            caste_ratio_breeder: out[2],
            forage_weight: out[3],
            dig_weight: out[4],
            nurse_weight: out[5],
            research_choice: None,
        }
    }
}

// ============================================================
// AetherLmBrain — STUB. Trained checkpoint integration.
// ============================================================

/// Aether-LM-backed brain. Serializes `ColonyAiState` to a text prompt,
/// invokes `aether-infer.exe` with the prompt, parses the completion
/// back into an `AiDecision`.
///
/// # Status: integration ready, no checkpoint yet
///
/// The serialization round-trip + subprocess plumbing are in place. If
/// the configured `aether_infer_exe` or `checkpoint_path` is missing,
/// `decide()` falls back to a safe deterministic default and logs a
/// warning — so a misconfigured PvP setup won't crash, just won't learn.
///
/// # Wire format
///
/// Prompt (one line):
/// ```text
/// state food=100.0 inflow=0.5 workers=30 soldiers=5 breeders=1 \
///       eggs=10 larvae=5 pupae=5 queens=1 losses=0 ed=inf ew=0 es=0 \
///       doy=150 t=22.0 dia=0 day=1 action=
/// ```
/// (the trailing `action=` cues the model to complete the action).
///
/// Completion expected as:
/// ```text
/// w:0.65 s:0.30 b:0.05 f:0.55 d:0.20 n:0.25 r:none
/// ```
/// Robust to extra whitespace + trailing tokens. Missing fields fall
/// back to safe defaults.
///
/// # Future
///
/// In-process integration once aether exposes a Rust API would skip the
/// subprocess overhead entirely. The `to_prompt` / `from_completion`
/// helpers are public so a future in-process binding can reuse them.
pub struct AetherLmBrain {
    pub checkpoint_path: std::path::PathBuf,
    pub aether_infer_exe: std::path::PathBuf,
    pub label: String,
    /// How many output tokens to ask aether-infer for. The wire format
    /// above fits in ~30 tokens.
    pub max_new_tokens: u32,
    /// Number of consecutive infer failures before we stop trying and
    /// fall back permanently for the rest of the run.
    pub failure_budget: u32,
    /// Internal counter — reaches 0, brain stops shelling out.
    failures_remaining: u32,
}

impl AetherLmBrain {
    /// Construct with default `aether-infer.exe` path (`J:/aether/target/release/aether-infer.exe`).
    pub fn new(checkpoint_path: impl Into<std::path::PathBuf>, label: impl Into<String>) -> Self {
        Self {
            checkpoint_path: checkpoint_path.into(),
            aether_infer_exe: std::path::PathBuf::from("J:/aether/target/release/aether-infer.exe"),
            label: label.into(),
            max_new_tokens: 40,
            failure_budget: 3,
            failures_remaining: 3,
        }
    }

    /// Override the path to `aether-infer.exe`.
    pub fn with_exe(mut self, exe: impl Into<std::path::PathBuf>) -> Self {
        self.aether_infer_exe = exe.into();
        self
    }
}

/// Format a `ColonyAiState` as a single-line aether prompt. Public so
/// future in-process integration + tests can share the format.
pub fn state_to_prompt(s: &ColonyAiState) -> String {
    let ed = if s.enemy_distance_min.is_finite() {
        format!("{:.1}", s.enemy_distance_min)
    } else {
        "inf".to_string()
    };
    format!(
        "state food={:.1} inflow={:.2} workers={} soldiers={} breeders={} \
         eggs={} larvae={} pupae={} queens={} losses={} ed={} ew={} es={} \
         doy={} t={:.1} dia={} day={} action=",
        s.food_stored, s.food_inflow_recent,
        s.worker_count, s.soldier_count, s.breeder_count,
        s.brood_egg, s.brood_larva, s.brood_pupa, s.queens_alive,
        s.combat_losses_recent, ed, s.enemy_worker_count, s.enemy_soldier_count,
        s.day_of_year, s.ambient_temp_c,
        s.diapause_active as u8, s.is_daytime as u8,
    )
}

/// Parse an aether-infer completion into an `AiDecision`. Returns `None`
/// if the completion is unparseable enough that defaults are safer.
pub fn completion_to_decision(completion: &str) -> Option<AiDecision> {
    let mut w = None;
    let mut s = None;
    let mut b = None;
    let mut f = None;
    let mut d = None;
    let mut n = None;
    for tok in completion.split_whitespace() {
        let Some((k, v)) = tok.split_once(':') else { continue };
        let parsed: Option<f32> = v.parse().ok();
        match (k, parsed) {
            ("w", Some(x)) => w = Some(x),
            ("s", Some(x)) => s = Some(x),
            ("b", Some(x)) => b = Some(x),
            ("f", Some(x)) => f = Some(x),
            ("d", Some(x)) => d = Some(x),
            ("n", Some(x)) => n = Some(x),
            _ => {}
        }
    }
    // Need at least one caste field AND one weight field to consider it
    // a valid completion. Missing fields use safe defaults.
    if w.is_none() && s.is_none() && b.is_none() {
        return None;
    }
    if f.is_none() && d.is_none() && n.is_none() {
        return None;
    }
    Some(AiDecision {
        caste_ratio_worker: w.unwrap_or(0.7).clamp(0.0, 1.0),
        caste_ratio_soldier: s.unwrap_or(0.25).clamp(0.0, 1.0),
        caste_ratio_breeder: b.unwrap_or(0.05).clamp(0.0, 1.0),
        forage_weight: f.unwrap_or(0.5).clamp(0.0, 1.0),
        dig_weight: d.unwrap_or(0.2).clamp(0.0, 1.0),
        nurse_weight: n.unwrap_or(0.3).clamp(0.0, 1.0),
        research_choice: None,
    })
}

fn safe_default_decision() -> AiDecision {
    AiDecision {
        caste_ratio_worker: 0.7,
        caste_ratio_soldier: 0.25,
        caste_ratio_breeder: 0.05,
        forage_weight: 0.5,
        dig_weight: 0.2,
        nurse_weight: 0.3,
        research_choice: None,
    }
}

impl AiBrain for AetherLmBrain {
    fn name(&self) -> &str {
        &self.label
    }

    fn decide(&mut self, state: &ColonyAiState) -> AiDecision {
        if self.failures_remaining == 0 {
            return safe_default_decision();
        }
        if !self.aether_infer_exe.exists() {
            tracing::warn!(
                exe = %self.aether_infer_exe.display(),
                "AetherLmBrain: aether-infer.exe not found — falling back to safe default for the rest of the run",
            );
            self.failures_remaining = 0;
            return safe_default_decision();
        }
        // aether stores checkpoints as `<base>.weights` + `<base>.meta`,
        // so `--ckpt foo/bar` corresponds to files `foo/bar.weights` and
        // `foo/bar.meta`. Probe the .weights file since it's the bigger
        // one and unambiguously identifies a real checkpoint.
        let weights_file = {
            let mut p = self.checkpoint_path.clone();
            let new_name = format!(
                "{}.weights",
                p.file_name().and_then(|s| s.to_str()).unwrap_or(""),
            );
            p.set_file_name(new_name);
            p
        };
        if !weights_file.exists() {
            tracing::warn!(
                ckpt = %self.checkpoint_path.display(),
                weights_probe = %weights_file.display(),
                "AetherLmBrain: checkpoint not found — falling back to safe default for the rest of the run",
            );
            self.failures_remaining = 0;
            return safe_default_decision();
        }
        let prompt = state_to_prompt(state);
        // aether-infer requires its --ckpt argument to be relative to
        // the cwd it's spawned in (aether refuses paths that "escape
        // cwd"). We pass the canonical absolute exe path but set the
        // subprocess cwd to the exe's grandparent (e.g. J:/aether/),
        // and rewrite the ckpt arg to be relative to that.
        let exe_canon = self
            .aether_infer_exe
            .canonicalize()
            .unwrap_or_else(|_| self.aether_infer_exe.clone());
        // exe is at <aether_root>/target/release/aether-infer.exe → root is two parents up.
        let aether_root = exe_canon
            .parent()
            .and_then(|p| p.parent())
            .and_then(|p| p.parent())
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
        let ckpt_canon = self
            .checkpoint_path
            .canonicalize()
            .unwrap_or_else(|_| self.checkpoint_path.clone());
        let ckpt_rel = ckpt_canon
            .strip_prefix(&aether_root)
            .map(|p| p.to_path_buf())
            .unwrap_or(self.checkpoint_path.clone());
        let output = match std::process::Command::new(&exe_canon)
            .current_dir(&aether_root)
            .arg("--ckpt").arg(&ckpt_rel)
            .arg("--prompt").arg(&prompt)
            .arg("--max-new").arg(self.max_new_tokens.to_string())
            .output()
        {
            Ok(o) => o,
            Err(e) => {
                tracing::warn!(error = %e, "AetherLmBrain: aether-infer spawn failed");
                self.failures_remaining = self.failures_remaining.saturating_sub(1);
                return safe_default_decision();
            }
        };
        if !output.status.success() {
            tracing::warn!(
                status = ?output.status,
                stderr = %String::from_utf8_lossy(&output.stderr),
                "AetherLmBrain: aether-infer non-zero exit",
            );
            self.failures_remaining = self.failures_remaining.saturating_sub(1);
            return safe_default_decision();
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        // Strip the prompt prefix from completion if aether-infer echoes it.
        let completion = stdout.strip_prefix(&prompt).unwrap_or(&stdout);
        match completion_to_decision(completion) {
            Some(d) => d,
            None => {
                tracing::warn!(
                    completion = %completion.chars().take(80).collect::<String>(),
                    "AetherLmBrain: completion unparseable — using safe default",
                );
                self.failures_remaining = self.failures_remaining.saturating_sub(1);
                safe_default_decision()
            }
        }
    }
}

// ============================================================
// MixedBrain — stochastic per-tick archetype mixer.
// ============================================================

/// Per-decision random mix of archetype brains. On every `decide()`
/// call, samples one component brain by weight and forwards the call.
/// Each component keeps its own internal state across calls (so e.g.
/// the chosen DefenderBrain's brood-buffer history persists between
/// the ticks where it actually drives the decision).
///
/// Designed to widen the eval bench past the deterministic 7-archetype
/// Nash plateau (~47.1%) — a mix-strategy opponent has no fixed
/// best-response policy, so a learned policy can actually differentiate
/// against it without hitting a single-point equilibrium.
///
/// Spec format used by `build_brain` / `make_brain`:
/// ```text
///   mix:defender,aggressor,economist
///   mix:defender=2,aggressor=1     // explicit weights
/// ```
pub struct MixedBrain {
    label: String,
    components: Vec<(Box<dyn AiBrain>, f32)>,
    cumulative: Vec<f32>,
    total_weight: f32,
    rng: rand_chacha::ChaCha8Rng,
}

impl MixedBrain {
    /// Build from a list of (brain, weight) pairs. Weights need not
    /// sum to 1 — the sampler normalizes. `label` surfaces in logs +
    /// bench reports.
    pub fn new(label: impl Into<String>, components: Vec<(Box<dyn AiBrain>, f32)>, seed: u64) -> Self {
        use rand::SeedableRng;
        assert!(!components.is_empty(), "MixedBrain: empty component list");
        let total_weight: f32 = components.iter().map(|(_, w)| *w).sum();
        assert!(total_weight > 0.0, "MixedBrain: all weights zero or negative");
        let mut cumulative = Vec::with_capacity(components.len());
        let mut acc = 0.0;
        for (_, w) in &components {
            acc += w;
            cumulative.push(acc);
        }
        Self {
            label: label.into(),
            components,
            cumulative,
            total_weight,
            rng: rand_chacha::ChaCha8Rng::seed_from_u64(seed),
        }
    }

    /// Construct from a comma-separated archetype list:
    /// `defender,aggressor` (equal weights) or
    /// `defender=2,aggressor=1` (explicit weights).
    /// Only takes the no-arg archetypes (the same 7 the league seeds with).
    pub fn from_archetype_spec(spec: &str, seed: u64) -> Result<Self, String> {
        let mut components: Vec<(Box<dyn AiBrain>, f32)> = Vec::new();
        let mut label_parts: Vec<String> = Vec::new();
        for part in spec.split(',') {
            let part = part.trim();
            if part.is_empty() { continue; }
            let (name, weight) = match part.split_once('=') {
                Some((n, w)) => {
                    let w: f32 = w.trim().parse().map_err(|e| format!("bad weight `{w}`: {e}"))?;
                    (n.trim(), w)
                }
                None => (part, 1.0),
            };
            let brain: Box<dyn AiBrain> = match name {
                "heuristic" => Box::new(HeuristicBrain::new(5.0)),
                "random" => Box::new(RandomBrain::new(seed.wrapping_add(label_parts.len() as u64))),
                "defender" => Box::new(DefenderBrain::new()),
                "aggressor" => Box::new(AggressorBrain::new()),
                "economist" => Box::new(EconomistBrain::new()),
                "breeder" => Box::new(BreederBrain::new()),
                "forager" => Box::new(ForagerBrain::new()),
                "conservative" => Box::new(ConservativeBuilderBrain::new()),
                other => return Err(format!("MixedBrain: unknown archetype `{other}`")),
            };
            label_parts.push(if (weight - 1.0).abs() < 1e-6 { name.to_string() } else { format!("{name}={weight}") });
            components.push((brain, weight));
        }
        if components.is_empty() {
            return Err("MixedBrain: empty spec".into());
        }
        let label = format!("mix[{}]", label_parts.join(","));
        Ok(Self::new(label, components, seed))
    }
}

impl AiBrain for MixedBrain {
    fn name(&self) -> &str { &self.label }

    fn decide(&mut self, state: &ColonyAiState) -> AiDecision {
        use rand::Rng;
        let x: f32 = self.rng.r#gen::<f32>() * self.total_weight;
        let mut idx = self.cumulative.len() - 1;
        for (i, c) in self.cumulative.iter().enumerate() {
            if x <= *c { idx = i; break; }
        }
        self.components[idx].0.decide(state)
    }
}

// ============================================================
// Match-end detection.
// ============================================================

/// Outcome of an AI-vs-AI (or AI-vs-player) match at the moment of
/// inspection. The headless bench harness ticks until this transitions
/// out of `InProgress`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum MatchStatus {
    /// Both colonies still have at least one queen and at least one
    /// adult ant of any caste.
    InProgress,
    /// Exactly one colony lost (queens=0 OR adults=0). The survivor wins.
    Won { winner: u8, loser: u8, ended_at_tick: u64 },
    /// Both colonies died on the same tick (rare; only happens with
    /// simultaneous queen kills or environmental sweeps).
    Draw { ended_at_tick: u64 },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn neutral_state() -> ColonyAiState {
        ColonyAiState {
            food_stored: 100.0,
            food_inflow_recent: 0.5,
            worker_count: 30,
            soldier_count: 5,
            breeder_count: 1,
            brood_egg: 10,
            brood_larva: 5,
            brood_pupa: 5,
            queens_alive: 1,
            combat_losses_recent: 0,
            enemy_distance_min: f32::INFINITY,
            enemy_worker_count: 0,
            enemy_soldier_count: 0,
            day_of_year: 150,
            ambient_temp_c: 22.0,
            diapause_active: false,
            is_daytime: true,
        }
    }

    #[test]
    fn heuristic_brain_idle_state_returns_defaults() {
        let mut b = HeuristicBrain::new(5.0);
        let d = b.decide(&neutral_state());
        assert!(d.is_valid());
        assert_eq!(d.caste_ratio_soldier, 0.30);
        assert_eq!(d.forage_weight, 0.55);
    }

    #[test]
    fn heuristic_brain_escalates_soldier_under_losses() {
        let mut b = HeuristicBrain::new(5.0);
        let initial = b.decide(&neutral_state()).caste_ratio_soldier;
        let mut state = neutral_state();
        state.combat_losses_recent = 5;
        let after = b.decide(&state).caste_ratio_soldier;
        assert!(after > initial, "soldier ratio should rise after losses ({initial} → {after})");
    }

    #[test]
    fn heuristic_brain_escalates_forage_when_food_low() {
        let mut b = HeuristicBrain::new(5.0);
        let initial = b.decide(&neutral_state()).forage_weight;
        let mut state = neutral_state();
        state.food_stored = 2.0; // well below low_food_threshold (= 5.0 * 4 = 20)
        let after = b.decide(&state).forage_weight;
        assert!(after > initial, "forage weight should rise when food low ({initial} → {after})");
    }

    #[test]
    fn random_brain_outputs_are_in_band() {
        let mut b = RandomBrain::new(42);
        for _ in 0..100 {
            let d = b.decide(&neutral_state());
            assert!(d.is_valid(), "RandomBrain produced out-of-band decision: {d:?}");
        }
    }

    #[test]
    fn random_brain_is_deterministic_given_seed() {
        let mut a = RandomBrain::new(7);
        let mut b = RandomBrain::new(7);
        for _ in 0..10 {
            let s = neutral_state();
            let da = a.decide(&s);
            let db = b.decide(&s);
            assert_eq!(da.caste_ratio_worker, db.caste_ratio_worker);
            assert_eq!(da.forage_weight, db.forage_weight);
        }
    }

    #[test]
    fn aether_brain_stub_returns_safe_default() {
        let mut b = AetherLmBrain::new("nonexistent/checkpoint", "aether-test");
        let d = b.decide(&neutral_state());
        assert!(d.is_valid(), "missing exe/ckpt must return a valid safe-default decision");
        assert_eq!(b.name(), "aether-test");
    }

    #[test]
    fn aether_state_to_prompt_has_required_fields() {
        let p = state_to_prompt(&neutral_state());
        // Spot-check a handful of fields show up in the right format.
        assert!(p.starts_with("state food="), "prompt: {p}");
        assert!(p.ends_with("action="), "prompt should cue completion: {p}");
        assert!(p.contains("workers=30"), "prompt missing workers field: {p}");
        assert!(p.contains("queens=1"), "prompt missing queens field: {p}");
        assert!(p.contains("ed=inf"), "prompt should encode infinite enemy distance as 'inf'");
    }

    #[test]
    fn aether_completion_to_decision_parses_well_formed() {
        let c = "w:0.6 s:0.3 b:0.1 f:0.4 d:0.2 n:0.4 r:none";
        let d = completion_to_decision(c).expect("well-formed completion");
        assert!(d.is_valid());
        assert!((d.caste_ratio_worker - 0.6).abs() < 1e-5);
        assert!((d.caste_ratio_soldier - 0.3).abs() < 1e-5);
        assert!((d.forage_weight - 0.4).abs() < 1e-5);
    }

    #[test]
    fn aether_completion_to_decision_rejects_empty_or_partial() {
        // Missing both caste fields AND weight fields → None.
        assert!(completion_to_decision("garbage no:colon r:none").is_none());
        // Has caste but no weight → None.
        assert!(completion_to_decision("w:0.6 s:0.3 b:0.1").is_none());
        // Has weight but no caste → None.
        assert!(completion_to_decision("f:0.4 d:0.2 n:0.4").is_none());
    }

    #[test]
    fn aether_completion_to_decision_clamps_out_of_range() {
        // Out-of-band values clamp into [0,1].
        let c = "w:1.5 s:-0.2 b:0.05 f:2.0 d:0.2 n:-1.0";
        let d = completion_to_decision(c).expect("partial-band still parses");
        assert_eq!(d.caste_ratio_worker, 1.0);
        assert_eq!(d.caste_ratio_soldier, 0.0);
        assert_eq!(d.forage_weight, 1.0);
        assert_eq!(d.nurse_weight, 0.0);
    }

    #[test]
    fn mlp_brain_forward_pass_is_deterministic() {
        // Hand-crafted tiny MLP: 17 -> 2 -> 2 -> 6 with all weights 0
        // and biases that produce a known output. After sigmoid:
        // sigmoid(0) = 0.5 for every output.
        use std::io::Write;
        let weights = serde_json::json!({
            "input_dim": 17, "hidden_dim": 2, "output_dim": 6,
            "input_mean": vec![0.0_f32; 17],
            "input_std":  vec![1.0_f32; 17],
            "w1": vec![vec![0.0_f32; 17]; 2],
            "b1": vec![0.0_f32; 2],
            "w2": vec![vec![0.0_f32; 2]; 2],
            "b2": vec![0.0_f32; 2],
            "w3": vec![vec![0.0_f32; 2]; 6],
            "b3": vec![0.0_f32; 6],
        });
        let tmp = std::env::temp_dir().join(format!("antcolony-mlp-test-{}.json", std::process::id()));
        std::fs::File::create(&tmp).unwrap().write_all(weights.to_string().as_bytes()).unwrap();

        let mut brain = MlpBrain::load(&tmp, "mlp-test").expect("load");
        let s = neutral_state();
        let d1 = brain.decide(&s);
        let d2 = brain.decide(&s);
        // Determinism check.
        assert_eq!(d1.caste_ratio_worker, d2.caste_ratio_worker);
        // sigmoid(0) = 0.5 — every output should be ~0.5 with all-zero weights.
        for v in [d1.caste_ratio_worker, d1.caste_ratio_soldier, d1.caste_ratio_breeder,
                  d1.forage_weight, d1.dig_weight, d1.nurse_weight] {
            assert!((v - 0.5).abs() < 1e-5, "sigmoid(0) should be 0.5, got {v}");
        }
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn aether_round_trip_preserves_state_essentials() {
        // Spot check that a state's salient numbers survive the prompt format
        // — the model itself is what learns the input, but the format must
        // be lossless enough that we can train on it.
        let mut s = neutral_state();
        s.food_stored = 73.5;
        s.worker_count = 142;
        s.combat_losses_recent = 7;
        let p = state_to_prompt(&s);
        assert!(p.contains("food=73.5"));
        assert!(p.contains("workers=142"));
        assert!(p.contains("losses=7"));
    }
}
