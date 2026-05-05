//! League — the opponent pool. Seeded with the 7 hardcoded archetypes;
//! grows as PPO snapshots are added.

use antcolony_sim::{
    AiBrain, AggressorBrain, BreederBrain, ConservativeBuilderBrain,
    DefenderBrain, EconomistBrain, ForagerBrain, HeuristicBrain, MlpBrain, RandomBrain,
};

/// Identifier for a league member. Closure box returned via `make_brain`.
pub struct LeagueEntry {
    pub name: String,
    pub spec: String,  // "heuristic", "mlp:<path>", etc.
    /// Difficulty tier — used by the curriculum sampler. 0 = easiest
    /// (heuristic), 1 = mid (single-axis archetypes), 2 = hard
    /// (MLP baseline / self-snapshots). Higher tiers gain weight as
    /// training progresses.
    pub tier: u8,
}

pub struct League {
    pub entries: Vec<LeagueEntry>,
}

impl League {
    /// Default league = 7 hardcoded archetypes (the fixed exploiters).
    pub fn default_pool() -> Self {
        let entries = vec![
            LeagueEntry { name: "heuristic".into(), spec: "heuristic".into(), tier: 0 },
            LeagueEntry { name: "defender".into(), spec: "defender".into(), tier: 1 },
            LeagueEntry { name: "aggressor".into(), spec: "aggressor".into(), tier: 1 },
            LeagueEntry { name: "economist".into(), spec: "economist".into(), tier: 1 },
            LeagueEntry { name: "breeder".into(), spec: "breeder".into(), tier: 1 },
            LeagueEntry { name: "forager".into(), spec: "forager".into(), tier: 1 },
            LeagueEntry { name: "conservative".into(), spec: "conservative".into(), tier: 1 },
        ];
        Self { entries }
    }

    pub fn add_mlp_snapshot(&mut self, name: impl Into<String>, weights_path: impl AsRef<std::path::Path>) {
        self.entries.push(LeagueEntry {
            name: name.into(),
            spec: format!("mlp:{}", weights_path.as_ref().display()),
            tier: 2,
        });
    }

    /// Curriculum opponent picker. `progress` in [0, 1] = how far through
    /// training we are; 0 = warm-up (heavily favor tier 0/1), 1 = late
    /// game (heavily favor tier 2 = MLP baseline + self-snapshots).
    /// Falls back to uniform if no tier-2 entries exist yet.
    pub fn sample_curriculum(&self, progress: f32, rng: &mut impl rand::Rng) -> usize {
        let progress = progress.clamp(0.0, 1.0);
        // Tier weight ramps: tier0 fades, tier1 stays mid, tier2 grows.
        let w0 = 1.0 - 0.7 * progress;          // 1.0 -> 0.3
        let w1 = 1.0;                            // flat
        let w2 = 0.2 + 1.8 * progress;          // 0.2 -> 2.0
        let weights: Vec<f32> = self.entries.iter().map(|e| match e.tier {
            0 => w0, 1 => w1, _ => w2,
        }).collect();
        let total: f32 = weights.iter().sum();
        if total <= 0.0 || self.entries.is_empty() {
            return 0;
        }
        let mut x: f32 = rng.r#gen::<f32>() * total;
        for (i, w) in weights.iter().enumerate() {
            x -= w;
            if x <= 0.0 { return i; }
        }
        self.entries.len() - 1
    }

    /// Materialize a brain for the given spec. Mirrors matchup_bench's
    /// build_brain() so league entries use the same parser.
    pub fn make_brain(spec: &str, seed: u64) -> Box<dyn AiBrain> {
        if let Some(rest) = spec.strip_prefix("mlp:") {
            return Box::new(MlpBrain::load(rest, format!("mlp-{seed}"))
                .expect("failed to load mlp weights"));
        }
        match spec {
            "heuristic" => Box::new(HeuristicBrain::new(5.0)),
            "random" => Box::new(RandomBrain::new(seed)),
            "defender" => Box::new(DefenderBrain::new()),
            "aggressor" => Box::new(AggressorBrain::new()),
            "economist" => Box::new(EconomistBrain::new()),
            "breeder" => Box::new(BreederBrain::new()),
            "forager" => Box::new(ForagerBrain::new()),
            "conservative" => Box::new(ConservativeBuilderBrain::new()),
            other => panic!("league: unknown spec `{other}`"),
        }
    }
}
