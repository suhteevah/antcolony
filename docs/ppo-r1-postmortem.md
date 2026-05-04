# PPO r1 Postmortem — 2026-05-03

**Status:** Failed first attempt, instructive failures.
**Goal:** Replace BC with proper RL per the May-2026 literature review (`docs/ai-literature-review-2026-05.md`). Break past the 45.7% BC ceiling.
**Result:** Model frozen across 20 iterations — eval stayed at exactly 41.4% (29/70), identical per-archetype breakdown across iter 5/10/15/20.

## What we built

`scripts/train_ppo_colony.py` — Python+PyTorch+CUDA PPO trainer that:
- Uses an actor-critic with Gaussian policy over the 6-dim AiDecision space
- Calls Rust matchup_bench as a subprocess for environment rollouts
- Reads trajectory JSONL output for state/action sequences
- Trains via PPO with importance-ratio clipping
- Eval calls deterministic `mlp:` spec; training calls `noisy_mlp:<path>:<std>` for exploration

`crates/antcolony-sim/src/ai/brain.rs` — added `MlpBrain.set_explore_std()` for training-time exploration.
`crates/antcolony-sim/examples/matchup_bench.rs` — added `noisy_mlp:<path>:<std>` spec parser.

## Two bugs that made it fail

### Bug 1: Action distribution mismatch
The Rust `noisy_mlp` adds **uniform** noise to the sigmoid output: `out + Uniform(-std, std)`.
The Python PPO computes log-probabilities under a **Gaussian** distribution: `Normal(mean, exp(log_std))`.

PPO's importance ratio `exp(new_log_prob - old_log_prob)` is mathematically meaningless when old_log_prob comes from a different distribution than new_log_prob. The clip term doesn't actually clip anything coherent. Net effect: PPO updates do nothing useful.

### Bug 2: Reward signal vanishes
The matchup_bench writes the final outcome to every trajectory record (post-match patching). My trainer set per-tick reward = 0 except the final tick = outcome. With ~260 timesteps per episode and gamma=0.99, the reward at step 0 propagates as `0.99^260 * R = 0.07 * R`. Gradients on early decisions are ~7% of what they should be, and the value estimate never has a strong signal to learn from.

## Why the eval was IDENTICAL across 20 iterations (the smoking gun)

Iter 5, 10, 15, 20 all evaluated to exactly `28-29/70` with the SAME per-archetype breakdown `heur:4 defe:3 aggr:2 econ:5 bree:5 fora:5 cons:5`. That's not noise — it's the model not changing AT ALL.

Combined diagnosis: the importance ratios are roughly 1.0 because the log_prob math is broken. The clipped surrogate loss is roughly entropy bonus + tiny value loss. The actor weights barely move. The eval (deterministic, deterministic-sim seeds) produces identical outcomes.

## What needs to change for r2

1. **Match the noise distribution.** Two options:
   - Make Rust `noisy_mlp` use Gaussian noise (sample from std::distr or external rand_distr crate)
   - OR: change Python to compute log_probs as Uniform PDF (simpler — `log(1/(2*std))` per dim if action is in range, else -inf)
   - PRESCRIPTION: switch to **Tanh-squashed Gaussian** policy (standard PPO practice). Sample pre-squash from Normal, apply tanh, scale to [0,1]. Compute log_probs with the squash correction. Match in Rust.

2. **Reward shaping.** Per-tick reward should reflect colony health changes:
   - `r_t = (workers_t - workers_{t-1}) / 10 + (queen_alive ? 0 : -1)` per tick
   - Final outcome bonus: `+1` for win, `-1` for loss
   - This gives a dense gradient that doesn't vanish

3. **GAE for advantages.** Currently using Monte Carlo returns. GAE with lambda=0.95 reduces variance and gives the critic a better target.

4. **Trajectory replay buffer.** Currently rollout-then-update. PPO benefits from a small replay buffer (4-8 iterations of data) so each batch has more diversity.

5. **Value function bootstrap.** Currently `value_pred ≈ R`, but R is binary 0/±1 — saturates the sigmoid. Either use raw scores (workers_share continuous) or use Huber loss to handle outliers.

6. **Verify gradients flow.** Print the L2 norm of actor.w1.weight before/after training step. If it doesn't change >0.01%, the training is broken.

## What we DID prove

- The infrastructure works end-to-end: Python → subprocess matchup_bench → trajectory JSONL → tensor → PPO update → JSON weights → MlpBrain inference. The plumbing is fine.
- The eval gives correctly correlated results when the model genuinely changes (verified by warm-starting → 64/140 = 45.7% baseline matches Rust-side eval).
- The architectural decision (drop BC for outcome-driven RL) is correct per the literature; only the implementation has known PPO bugs.

## Path forward

The MVP architecture (single-policy, Gaussian-actor, sparse-reward) is the simplest possible PPO setup but ALSO the most fragile to these specific bugs. For r2, fix the 6 items above OR jump straight to a more proven framework (CleanRL, RL Games, Stable-Baselines3) that handles these correctly out of the box.

**Recommendation:** use Stable-Baselines3 PPO with a custom Gym-like wrapper around matchup_bench. Battle-tested implementation, known-good defaults, ~2 hours of integration work. This is what we should have done from the start instead of rolling our own PPO.

Best model: still MLP_v1 from FSP-r1 at 45.7%.
