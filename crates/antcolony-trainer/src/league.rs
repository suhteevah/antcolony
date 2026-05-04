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
}

pub struct League {
    pub entries: Vec<LeagueEntry>,
}

impl League {
    /// Default league = 7 hardcoded archetypes (the fixed exploiters).
    pub fn default_pool() -> Self {
        let entries = vec![
            LeagueEntry { name: "heuristic".into(), spec: "heuristic".into() },
            LeagueEntry { name: "defender".into(), spec: "defender".into() },
            LeagueEntry { name: "aggressor".into(), spec: "aggressor".into() },
            LeagueEntry { name: "economist".into(), spec: "economist".into() },
            LeagueEntry { name: "breeder".into(), spec: "breeder".into() },
            LeagueEntry { name: "forager".into(), spec: "forager".into() },
            LeagueEntry { name: "conservative".into(), spec: "conservative".into() },
        ];
        Self { entries }
    }

    pub fn add_mlp_snapshot(&mut self, name: impl Into<String>, weights_path: impl AsRef<std::path::Path>) {
        self.entries.push(LeagueEntry {
            name: name.into(),
            spec: format!("mlp:{}", weights_path.as_ref().display()),
        });
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
