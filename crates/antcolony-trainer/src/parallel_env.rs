//! Parallel-env rollout for single-GPU Phase-3 training.
//!
//! Holds N `MatchEnv`s, each with the left colony driven by the HAC and the
//! right by a league-sampled `AiBrain`. The sim steps on CPU; the left
//! colony's observations are batched across all *active* envs into single
//! GPU forwards (commander every DECISION_CADENCE ticks, ants every tick),
//! and outputs scattered back. Emits the existing `JointRollout` with
//! left-colony records only, `match_idx = env_idx`, `colony = 0`, so the
//! Phase-2b-2 `joint_update` + GAE are reused unchanged.

use anyhow::Result;
use candle_core::{Device, Tensor};
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

use antcolony_sim::ai::observation::AntModulators;
use antcolony_sim::{AiBrain, AiDecision, MatchStatus};

use crate::env::{MatchEnv, DECISION_CADENCE};
use crate::hierarchical::obs_to_tensors::{ant_obs_to_tensors, rich_batch_to_tensors};
use crate::hierarchical::sizing::{Sizing, A1, FIXED_INTENT_D, FIXED_MODULATOR_D};
use crate::joint_ppo::{AntBatch, CommanderRecord, JointRollout};
use crate::reward::{compute_step_reward, ColonyMetrics, RewardConfig};
use crate::self_play::{load_frozen_hac, OpponentKind, OpponentSampler, SnapshotPool};
use crate::HierarchicalActorCritic;
use crate::League;

pub struct ParallelEnv {
    pub n_envs: usize,
    pub rollout_cycles: usize,
    pub league: League,
    /// Master switch for SP1 self-play opponent selection. Defaults to `false`
    /// so legacy callers (phase3) are BYTE-IDENTICAL to the pre-Task-4 path:
    /// per-env round-robin league opponents driven by `decide`, and the
    /// training `rng` is NEVER touched for opponent selection. When `true`,
    /// `collect_rollout` samples ONE pool opponent per rollout (archetype or
    /// frozen-HAC snapshot) via a SEPARATE, independently-seeded RNG, so the
    /// training `rng` stream is identical to the `false` case for the same
    /// seed. Task 5 flips this from `Phase3Config::self_play_enabled`.
    pub self_play_enabled: bool,
    /// SP1 self-play opponent pool (7 archetypes + capped FIFO of frozen HAC
    /// self-snapshots). Defaults to archetypes-only so legacy callers behave
    /// exactly as before.
    pub pool: SnapshotPool,
    /// How the pool is sampled to pick the single per-rollout opponent.
    pub sampler: OpponentSampler,
    /// Network sizing used to materialize a frozen-HAC snapshot opponent.
    /// Must match the training HAC's sizing (default A1).
    pub sizing: Sizing,
    /// Index (into `pool.entries`) of the opponent chosen for the most recent
    /// `collect_rollout`. Phase3/Task 5 reads this to call `pool.record_result`.
    pub last_opponent_idx: usize,
    /// Scalar in [0,1] summarizing how the LEFT (training) HAC fared against the
    /// chosen opponent in the most recent rollout — mean over envs of
    /// `left_workers / (left_workers + right_workers)` at rollout end
    /// (0.5 when undefined). Feeds the PFSP EMA; not a perfect win metric.
    pub last_hac_winshare: f32,
    /// `None` ⇒ legacy same-species `MatchEnv::new` per env (BYTE-IDENTICAL to the
    /// pre-cross-species path). `Some` ⇒ each env is a cross-species (nest) arena
    /// with a species pair drawn deterministically from the roster per env seed,
    /// mirroring the flat trainer's `CrossSpeciesCurriculum`.
    pub cross_species: Option<crate::ppo::CrossSpeciesCurriculum>,
}

impl ParallelEnv {
    pub fn new(n_envs: usize, rollout_cycles: usize) -> Self {
        Self {
            n_envs,
            rollout_cycles,
            league: League::default_pool(),
            self_play_enabled: false,
            pool: SnapshotPool::with_archetypes(8, 0.1),
            sampler: OpponentSampler::Uniform,
            sizing: A1,
            last_opponent_idx: 0,
            last_hac_winshare: 0.5,
            cross_species: None,
        }
    }
}

/// Decode one row of a [B, 6] squashed-action tensor into an AiDecision.
fn row_to_decision(action: &Tensor, row: usize) -> Result<AiDecision> {
    let v: Vec<f32> = action.narrow(0, row, 1)?.squeeze(0)?.to_vec1()?;
    // L11: document the 6-dim action invariant (can't fire — dims pinned).
    debug_assert_eq!(v.len(), 6, "row_to_decision expects 6 action dims");
    Ok(AiDecision {
        caste_ratio_worker: v[0],
        caste_ratio_soldier: v[1],
        caste_ratio_breeder: v[2],
        forage_weight: v[3],
        dig_weight: v[4],
        nurse_weight: v[5],
        research_choice: None,
    })
}

/// One row of a [B, 64] intent tensor into a fixed [f32; 64] array.
fn row_to_intent(intent: &Tensor, row: usize) -> Result<[f32; FIXED_INTENT_D]> {
    let v: Vec<f32> = intent.narrow(0, row, 1)?.flatten_all()?.to_vec1()?;
    let mut arr = [0.0f32; FIXED_INTENT_D];
    arr.copy_from_slice(&v);
    Ok(arr)
}

/// Cumulative cross-colony losses for a colony (never reset by the sim), used
/// to derive the per-window combat-loss delta. H7: the tick-local
/// `combat_losses_this_tick` is cleared every tick, so it can't be sampled at
/// cycle cadence — the cumulative counter's delta over the window is the
/// correct window-summed loss count.
fn colony_combat_losses(env: &MatchEnv, k: u8) -> u32 {
    env.sim.colonies.get(k as usize).map(|c| c.combat_losses).unwrap_or(0)
}

impl ParallelEnv {
    /// Collect one parallel rollout. Creates `n_envs` fresh matches (left =
    /// HAC training colony), runs up to `rollout_cycles` commander cycles, and
    /// returns left-colony records bucketed by env. `base_seed` decorrelates
    /// this rollout's env/opponent seeds.
    ///
    /// Right-colony (opponent) selection is GATED on `self_play_enabled`:
    /// - `false` (default): per-env league round-robin
    ///   (`(i + base_seed) % league.entries.len()`), each env driven by its own
    ///   `make_brain(spec).decide`. Nothing is drawn from the training `rng` for
    ///   opponent selection — this is byte-identical to the pre-Task-4 path.
    /// - `true`: ONE pool opponent is sampled per rollout (archetype or
    ///   frozen-HAC snapshot) using a SEPARATE, independently-seeded RNG. The
    ///   training `rng` (consumed only by `sample_commander`/`sample_ant`) is
    ///   therefore identical to the `false` case for the same seed, preserving
    ///   the "byte-identical with self-play OFF" Global Constraint.
    pub fn collect_rollout(
        &mut self,
        hac: &HierarchicalActorCritic,
        device: &Device,
        rng: &mut ChaCha8Rng,
        reward: &RewardConfig,
        base_seed: u64,
    ) -> Result<JointRollout> {
        tracing::debug!(n_envs = self.n_envs, rollout_cycles = self.rollout_cycles, base_seed, self_play_enabled = self.self_play_enabled, "parallel rollout start");

        // ── SP1: pick the rollout opponent, GATED on `self_play_enabled` ──
        //
        // CRITICAL backward-compat constraint: with self-play OFF this path must
        // be byte-identical to the pre-Task-4 code, and the training `rng` (which
        // the LEFT path's `sample_commander`/`sample_ant` consume) must NEVER be
        // perturbed by opponent selection. So when ON we sample from a SEPARATE,
        // independently-seeded RNG derived from `base_seed` — never the passed-in
        // training `rng`. The training `rng` stream is therefore identical across
        // both modes for the same seed.
        let opp_idx = if self.self_play_enabled && !self.pool.entries.is_empty() {
            // Separate, deterministic opponent RNG seeded from base_seed only.
            // Distinct from the env-seed mixing constant so the two streams don't
            // alias. NOTHING here touches the training `rng`.
            let mut opp_rng = ChaCha8Rng::seed_from_u64(
                base_seed.wrapping_mul(0x9E3779B97F4A7C15) ^ 0x5F1A_5EED_0DD0_0FF5,
            );
            self.sampler.sample(&self.pool, &mut opp_rng)
        } else {
            // self-play OFF (or empty pool): no pool sampling at all. The pre-Task-4
            // contract sets these to fixed values; round-robin selection happens
            // per-env below and does not draw from any rng.
            0
        };
        self.last_opponent_idx = opp_idx;

        // On the self-play path a snapshot opponent may be a frozen HAC; resolve
        // it ONCE. With self-play OFF, `frozen_opp` is always None and `opp_kind`
        // is unused (the round-robin league brains drive the right colony).
        let opp_kind = if self.self_play_enabled {
            self.pool.entries.get(opp_idx).map(|e| e.kind.clone())
        } else {
            None
        };

        // If the chosen opponent is a frozen-HAC snapshot, try to load it ONCE.
        // A bad/missing snapshot must NOT abort the run: log and fall back to the
        // per-env archetype path (heuristic spec), mirroring the existing M16
        // fallback. `frozen_opp` is None unless a snapshot loaded successfully.
        let (opp_spec, frozen_opp): (Option<String>, Option<HierarchicalActorCritic>) = match &opp_kind {
            Some(OpponentKind::Snapshot { name, path }) => {
                match load_frozen_hac(path, self.sizing, device) {
                    Ok(hac) => {
                        tracing::debug!(opp_idx, %name, "snapshot opponent loaded; right colony driven by frozen HAC");
                        (None, Some(hac))
                    }
                    Err(e) => {
                        tracing::error!(opp_idx, %name, path = %path.display(), error = %e, "bad snapshot opponent; falling back to heuristic archetype");
                        // Fall back to a single shared heuristic spec for all envs.
                        (Some("heuristic".to_string()), None)
                    }
                }
            }
            // Self-play ON + archetype entry: one shared archetype spec for all envs.
            Some(OpponentKind::Archetype(spec)) => (Some(spec.clone()), None),
            // Self-play OFF (opp_kind == None): per-env round-robin handled below.
            None => (None, None),
        };
        let snapshot_path = frozen_opp.is_some();

        let mut envs: Vec<MatchEnv> = Vec::with_capacity(self.n_envs);
        // Per-env archetype brains are only needed when NOT on the snapshot path.
        let mut opponents: Vec<Box<dyn AiBrain>> = Vec::with_capacity(self.n_envs);
        for i in 0..self.n_envs {
            let seed = base_seed ^ ((i as u64).wrapping_mul(0x9E3779B97F4A7C15));
            // Cross-species curriculum: each env is a distinct (species_a,
            // species_b) nest/flat arena drawn deterministically from the roster
            // by `seed` (mirrors the flat trainer, ppo.rs). `None` ⇒ legacy
            // `MatchEnv::new(seed)` (byte-identical). Species choice is
            // seed-derived only, so it never perturbs the training `rng`.
            match &self.cross_species {
                Some(cur) if !cur.roster.is_empty() => {
                    let n = cur.roster.len() as u64;
                    let a = (seed % n) as usize;
                    let b = (((seed / n) % (n.saturating_sub(1).max(1))) as usize + a + 1)
                        % cur.roster.len();
                    let mut e = if cur.nest {
                        MatchEnv::new_cross_species_nest_arena(&cur.roster[a], &cur.roster[b], seed)
                    } else {
                        MatchEnv::new_cross_species_arena(&cur.roster[a], &cur.roster[b], seed)
                    };
                    e.sim.config.combat.venom_cycle_strength = cur.venom_cycle_strength;
                    envs.push(e);
                }
                _ => envs.push(MatchEnv::new(seed)),
            }
            if snapshot_path {
                continue;
            }
            // Pick this env's opponent spec:
            // - self-play OFF: pre-Task-4 per-env league round-robin (byte-identical).
            // - self-play ON (archetype / snapshot-load-failed): the single shared
            //   spec sampled once above (all envs use it).
            let spec = match &opp_spec {
                Some(s) => s.clone(),
                None => {
                    let pick = (i + (base_seed as usize)) % self.league.entries.len();
                    self.league.entries[pick].spec.clone()
                }
            };
            // M16: a bad spec (typo'd --add-opp, missing snapshot) must not
            // abort the whole run. Log it and fall back to the heuristic brain
            // for this env so training continues. `heuristic` is hardcoded and
            // infallible, so the unwrap_or_else can't itself fail.
            let brain = League::make_brain(&spec, seed.wrapping_add(1)).unwrap_or_else(|e| {
                tracing::error!(env = i, spec = %spec, error = %e, "bad opponent spec; falling back to heuristic");
                League::make_brain("heuristic", seed.wrapping_add(1))
                    .expect("heuristic brain is infallible")
            });
            opponents.push(brain);
        }

        let mut out = JointRollout::default();
        let mut done = vec![false; self.n_envs];
        // Baseline metrics: no window has elapsed yet, so window combat losses = 0.
        let mut prev: Vec<[ColonyMetrics; 2]> = (0..self.n_envs)
            .map(|i| {
                [
                    ColonyMetrics::from_sim(&envs[i].sim, 0, 0),
                    ColonyMetrics::from_sim(&envs[i].sim, 1, 0),
                ]
            })
            .collect();

        for cycle in 0..self.rollout_cycles {
            let active: Vec<usize> = (0..self.n_envs)
                .filter(|&i| !done[i] && envs[i].sim.colony_rich_observation(0).is_some())
                .collect();
            if active.is_empty() {
                break;
            }

            // H7: snapshot the cumulative combat-loss counter per active env
            // BEFORE the inner tick loop; the per-cycle reward uses the delta
            // over the window (the tick-local counter is cleared every tick).
            let loss_base: std::collections::HashMap<usize, [u32; 2]> = active
                .iter()
                .map(|&i| (i, [colony_combat_losses(&envs[i], 0), colony_combat_losses(&envs[i], 1)]))
                .collect();

            // ── Commander forward, batched across active envs ──
            let riches: Vec<_> = active
                .iter()
                .map(|&i| {
                    envs[i]
                        .sim
                        .colony_rich_observation(0)
                        .expect("active => Some: filtered by is_some() above")
                })
                .collect();
            let rich_refs: Vec<_> = riches.iter().collect();
            let (state_b, pher_b, hist_b) = rich_batch_to_tensors(&rich_refs, device)?;
            let cmdr = hac.sample_commander(&state_b, &pher_b, &hist_b, rng)?;
            let cmdr_lp: Vec<f32> = cmdr.log_prob.to_vec1()?;
            let cmdr_val: Vec<f32> = cmdr.value.to_vec1()?;

            for (j, &i) in active.iter().enumerate() {
                let dec = row_to_decision(&cmdr.action, j)?;
                envs[i].sim.apply_ai_decision(0, &dec);
                let intent = row_to_intent(&cmdr.intent, j)?;
                envs[i].sim.apply_commander_intent(0, &intent);
            }

            // ── Right colony (the opponent) commander forward ──
            // Two mutually exclusive paths: a frozen-HAC self-snapshot drives the
            // right colony with its own MEAN commander action (batched across
            // active envs, mirroring the left), OR cheap per-env archetype brains
            // drive it via `decide`. `right_intent` maps env idx -> the frozen
            // HAC's right-colony intent tensor [1,64] for that cycle, consumed by
            // the per-tick right ant forward below (empty on the archetype path).
            let mut right_intent: std::collections::HashMap<usize, Tensor> =
                std::collections::HashMap::new();
            if let Some(frozen) = frozen_opp.as_ref() {
                // Batch the right colony's rich observations over active envs that
                // still have a live right colony. Envs whose right colony is gone
                // simply get no decision this cycle (the match is effectively over
                // for them).
                let r_active: Vec<usize> = active
                    .iter()
                    .copied()
                    .filter(|&i| envs[i].sim.colony_rich_observation(1).is_some())
                    .collect();
                if !r_active.is_empty() {
                    let r_riches: Vec<_> = r_active
                        .iter()
                        .map(|&i| {
                            envs[i]
                                .sim
                                .colony_rich_observation(1)
                                .expect("r_active => Some: filtered by is_some() above")
                        })
                        .collect();
                    let r_refs: Vec<_> = r_riches.iter().collect();
                    let (r_state, r_pher, r_hist) = rich_batch_to_tensors(&r_refs, device)?;
                    // MEAN action (no sampling, no gradient) — the opponent is frozen.
                    let (r_action, r_intent_b, _r_value) =
                        frozen.mean_commander_action(&r_state, &r_pher, &r_hist)?;
                    for (j, &i) in r_active.iter().enumerate() {
                        let dec = row_to_decision(&r_action, j)?;
                        envs[i].sim.apply_ai_decision(1, &dec);
                        let intent = row_to_intent(&r_intent_b, j)?;
                        envs[i].sim.apply_commander_intent(1, &intent);
                        // Keep this env's [1,64] intent row for its ant forward.
                        right_intent.insert(i, r_intent_b.narrow(0, j, 1)?);
                    }
                }
            } else {
                // `opponents` is indexed by env idx (built for all n_envs above).
                for &i in &active {
                    if let Some(sr) = envs[i].sim.colony_ai_state(1) {
                        let dr = opponents[i].decide(&sr);
                        envs[i].sim.apply_ai_decision(1, &dr);
                    }
                }
            }

            // ── Tick loop with per-tick batched ant decisions over active envs ──
            for _ in 0..DECISION_CADENCE {
                let mut cones: Vec<Tensor> = Vec::new();
                let mut internals: Vec<Tensor> = Vec::new();
                let mut intents: Vec<Tensor> = Vec::new();
                let mut index_map: Vec<(usize, u32)> = Vec::new();
                for (j, &i) in active.iter().enumerate() {
                    if done[i] {
                        continue;
                    }
                    let obs = envs[i].sim.per_ant_observations(0);
                    if obs.is_empty() {
                        continue;
                    }
                    // Slice one intent row [1, 64] for this env's ants.
                    let intent_row = cmdr.intent.narrow(0, j, 1)?;
                    let (cone, internal, intent_b) =
                        ant_obs_to_tensors(&obs, &intent_row, device)?;
                    for o in &obs {
                        index_map.push((i, o.ant_id));
                    }
                    cones.push(cone);
                    internals.push(internal);
                    intents.push(intent_b);
                }
                if !index_map.is_empty() {
                    let cone = Tensor::cat(&cones, 0)?;
                    let internal = Tensor::cat(&internals, 0)?;
                    let intent = Tensor::cat(&intents, 0)?;
                    let ant = hac.sample_ant(&cone, &internal, &intent, rng)?;
                    // Pull the whole batch host-side ONCE (one GPU->CPU sync each)
                    // instead of per-ant narrow+to_vec1. On CUDA a per-ant sync per
                    // row costs ~ms; with tens of thousands of ant rows that was the
                    // dominant cost (≈50 min/iter). to_vec2 transfers [M,5] in one go.
                    let lp: Vec<f32> = ant.log_prob.to_vec1()?;
                    let val: Vec<f32> = ant.value.to_vec1()?;
                    let mod_rows: Vec<Vec<f32>> = ant.modulator.to_vec2()?;
                    // Scatter modulators back to each env's sim (per-env grouped).
                    let mut row = 0usize;
                    for &i in &active {
                        if done[i] {
                            continue;
                        }
                        let mut mods: Vec<AntModulators> = Vec::new();
                        let mut ids: Vec<u32> = Vec::new();
                        // index_map is grouped by env in active order; consume contiguous block.
                        while row < index_map.len() && index_map[row].0 == i {
                            let m = &mod_rows[row];
                            mods.push(AntModulators {
                                alpha_mult: m[0],
                                beta_mult: m[1],
                                exploration_mod: m[2],
                                deposit_mult: m[3],
                                state_bias: m[4],
                            });
                            ids.push(index_map[row].1);
                            row += 1;
                        }
                        if !ids.is_empty() {
                            envs[i].sim.apply_ant_modulators(0, &mods, &ids);
                        }
                    }
                    // Compile-time guard: the m[0..5] decode above assumes a 5-d modulator.
                    const _: () = assert!(FIXED_MODULATOR_D == 5);
                    // Store the WHOLE tick batch (one tensor each), not per-ant —
                    // keeps the GPU cat in joint_update cheap. Row order matches
                    // index_map (env-grouped); match_idx = env index, colony = 0.
                    out.ant.push(AntBatch {
                        match_idx: index_map.iter().map(|&(e, _)| e).collect(),
                        colony: vec![0u8; index_map.len()],
                        cycle,
                        cone: cone.detach(),
                        internal: internal.detach(),
                        intent: intent.detach(),
                        modulator: ant.modulator.detach(),
                        log_prob: lp,
                        value: val,
                    });
                }

                // ── Frozen-HAC right colony: per-tick MEAN ant modulators ──
                // Only on the snapshot path. Batched across active envs (same
                // weights), mirroring the left side, but written to colony 1 and
                // NOT recorded in `out` (the right colony is not being trained).
                if let Some(frozen) = frozen_opp.as_ref() {
                    let mut r_cones: Vec<Tensor> = Vec::new();
                    let mut r_internals: Vec<Tensor> = Vec::new();
                    let mut r_intents: Vec<Tensor> = Vec::new();
                    let mut r_index: Vec<(usize, u32)> = Vec::new();
                    for &i in &active {
                        if done[i] {
                            continue;
                        }
                        // Need this env's right-colony intent row from the cycle's
                        // frozen commander forward; skip if the right colony had no
                        // observation this cycle.
                        let Some(intent_row) = right_intent.get(&i) else {
                            continue;
                        };
                        let obs = envs[i].sim.per_ant_observations(1);
                        if obs.is_empty() {
                            continue;
                        }
                        let (cone, internal, intent_b) =
                            ant_obs_to_tensors(&obs, intent_row, device)?;
                        for o in &obs {
                            r_index.push((i, o.ant_id));
                        }
                        r_cones.push(cone);
                        r_internals.push(internal);
                        r_intents.push(intent_b);
                    }
                    if !r_index.is_empty() {
                        let cone = Tensor::cat(&r_cones, 0)?;
                        let internal = Tensor::cat(&r_internals, 0)?;
                        let intent = Tensor::cat(&r_intents, 0)?;
                        let mods_t = frozen.mean_ant_modulator(&cone, &internal, &intent)?;
                        let mod_rows: Vec<Vec<f32>> = mods_t.to_vec2()?;
                        let mut row = 0usize;
                        for &i in &active {
                            if done[i] {
                                continue;
                            }
                            let mut mods: Vec<AntModulators> = Vec::new();
                            let mut ids: Vec<u32> = Vec::new();
                            while row < r_index.len() && r_index[row].0 == i {
                                let m = &mod_rows[row];
                                mods.push(AntModulators {
                                    alpha_mult: m[0],
                                    beta_mult: m[1],
                                    exploration_mod: m[2],
                                    deposit_mult: m[3],
                                    state_bias: m[4],
                                });
                                ids.push(r_index[row].1);
                                row += 1;
                            }
                            if !ids.is_empty() {
                                envs[i].sim.apply_ant_modulators(1, &mods, &ids);
                            }
                        }
                    }
                }

                for &i in &active {
                    if done[i] {
                        continue;
                    }
                    envs[i].sim.tick();
                    if !matches!(envs[i].sim.match_status(), MatchStatus::InProgress)
                        || envs[i].sim.tick >= envs[i].max_ticks
                    {
                        done[i] = true;
                    }
                }
            }

            // ── Per-cycle reward + commander records for active envs ──
            // Host-side copies of the per-env state/action rows, pulled ONCE
            // (two GPU->CPU syncs) for the history tokens below — avoids a
            // per-record narrow+to_vec1 sync per active env.
            let state_rows: Vec<Vec<f32>> = state_b.to_vec2()?;
            let action_rows: Vec<Vec<f32>> = cmdr.action.to_vec2()?;
            for (j, &i) in active.iter().enumerate() {
                // Window-summed combat losses = cumulative-counter delta over
                // the inner tick loop (H7). `loss_base` always has `i` (built
                // from `active` above).
                let base = loss_base.get(&i).copied().unwrap_or([0, 0]);
                let win0 = colony_combat_losses(&envs[i], 0).saturating_sub(base[0]);
                let win1 = colony_combat_losses(&envs[i], 1).saturating_sub(base[1]);
                let cur = [
                    ColonyMetrics::from_sim(&envs[i].sim, 0, win0),
                    ColonyMetrics::from_sim(&envs[i].sim, 1, win1),
                ];
                let status = envs[i].sim.match_status();
                let (reward_left, _reward_right) =
                    compute_step_reward(reward, &prev[i], &cur, done[i], status);
                prev[i] = cur;
                out.commander.push(CommanderRecord {
                    match_idx: i,
                    colony: 0,
                    cycle,
                    state: state_b.narrow(0, j, 1)?.detach(),
                    pheromone: pher_b.narrow(0, j, 1)?.detach(),
                    history: hist_b.narrow(0, j, 1)?.detach(),
                    action: cmdr.action.narrow(0, j, 1)?.detach(),
                    log_prob: cmdr_lp[j],
                    value: cmdr_val[j],
                    reward: reward_left,
                    done: done[i],
                });
                let mut st = [0.0f32; 17];
                st.copy_from_slice(&state_rows[j]);
                let mut ac = [0.0f32; 6];
                ac.copy_from_slice(&action_rows[j]);
                envs[i].sim.push_commander_history(0, st, ac, reward_left);
            }
        }
        // ── HAC win-share vs this opponent (feeds the PFSP EMA) ──
        // Only meaningful on the self-play path (PFSP needs it). With self-play
        // OFF we keep the pre-Task-4 contract and report the neutral 0.5 sentinel.
        // Mean over envs of left_workers / (left_workers + right_workers) at
        // rollout end. Per-env div-by-zero (both colonies wiped) is graded 0.5.
        // The overall mean falls back to 0.5 if there are no envs.
        self.last_hac_winshare = if !self.self_play_enabled || envs.is_empty() {
            0.5
        } else {
            let mut share_sum = 0.0f32;
            for env in &envs {
                let lw = env.sim.colonies.first().map(|c| c.population.workers).unwrap_or(0) as f32;
                let rw = env.sim.colonies.get(1).map(|c| c.population.workers).unwrap_or(0) as f32;
                let denom = lw + rw;
                share_sum += if denom > 0.0 { lw / denom } else { 0.5 };
            }
            share_sum / envs.len() as f32
        };

        tracing::debug!(
            commander_records = out.commander.len(),
            ant_records = out.ant.len(),
            opponent_idx = self.last_opponent_idx,
            hac_winshare = self.last_hac_winshare,
            "parallel rollout complete"
        );
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hierarchical::sizing::A1;
    use crate::reward::RewardConfig;
    use candle_core::{DType, Device};
    use candle_nn::{VarBuilder, VarMap};
    use rand::SeedableRng;

    #[test]
    fn parallel_env_constructs_with_default_league() {
        let pe = ParallelEnv::new(4, 8);
        assert_eq!(pe.n_envs, 4);
        assert_eq!(pe.rollout_cycles, 8);
        assert_eq!(pe.league.entries.len(), 7, "default pool = 7 archetypes");
    }

    #[test]
    fn collect_rollout_fills_buffer_left_only_env_bucketed() {
        let varmap = VarMap::new();
        let device = Device::Cpu;
        let vb = VarBuilder::from_varmap(&varmap, DType::F32, &device);
        let hac = HierarchicalActorCritic::new(vb, A1).unwrap();
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(0xa1);
        let reward = RewardConfig::default();

        let mut pe = ParallelEnv::new(3, 4);
        let roll = pe.collect_rollout(&hac, &device, &mut rng, &reward, 0xfeed).unwrap();

        assert!(!roll.commander.is_empty());
        assert!(!roll.ant.is_empty());
        assert!(roll.commander.iter().all(|r| r.colony == 0));
        // Left-vs-league: every ant row is colony 0, every match_idx is an env.
        assert!(roll.ant.iter().all(|a| a.colony.iter().all(|&c| c == 0)));
        assert!(roll.ant.iter().all(|a| a.match_idx.iter().all(|&e| e < 3)));
        let envs_seen: std::collections::HashSet<usize> =
            roll.commander.iter().map(|r| r.match_idx).collect();
        assert!(envs_seen.iter().all(|&e| e < 3));
        assert!(!envs_seen.is_empty());
        for r in &roll.commander {
            assert_eq!(r.state.dims(), &[1, 17]);
            assert!(r.reward.is_finite() && r.value.is_finite() && r.log_prob.is_finite());
        }
        for a in &roll.ant {
            let mt = a.len();
            assert!(mt >= 1);
            assert_eq!(a.modulator.dims(), &[mt, 5]);
            assert_eq!(a.match_idx.len(), mt);
            assert!(a.log_prob.iter().chain(a.value.iter()).all(|x| x.is_finite()));
        }
    }

    /// Cross-species curriculum wiring: with a roster set, collect_rollout builds
    /// cross-species nest-arena envs and produces a finite rollout (no freeze, no
    /// NaN). Skips if the species dir is unavailable.
    #[test]
    fn collect_rollout_cross_species_produces_finite_rollout() {
        let dir = std::path::PathBuf::from(
            std::env::var("ANTCOLONY_SPECIES_DIR").unwrap_or_else(|_| "../../assets/species".to_owned()),
        );
        let roster = match antcolony_sim::species::load_species_dir(&dir) {
            Ok(r) if !r.is_empty() => r,
            _ => { eprintln!("skip: no species dir"); return; }
        };
        let varmap = VarMap::new();
        let device = Device::Cpu;
        let vb = VarBuilder::from_varmap(&varmap, DType::F32, &device);
        let hac = HierarchicalActorCritic::new(vb, A1).unwrap();
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(0xa1);
        let reward = RewardConfig::default();

        let mut pe = ParallelEnv::new(3, 4);
        pe.cross_species = Some(crate::ppo::CrossSpeciesCurriculum {
            roster: std::sync::Arc::new(roster),
            venom_cycle_strength: 3.0,
            nest: true,
        });
        let roll = pe.collect_rollout(&hac, &device, &mut rng, &reward, 0xfeed).unwrap();
        assert!(!roll.commander.is_empty(), "cross-species rollout produced no commander records");
        for r in &roll.commander {
            assert!(r.reward.is_finite() && r.value.is_finite() && r.log_prob.is_finite());
        }
    }
}
