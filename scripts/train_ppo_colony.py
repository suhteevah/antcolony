"""PPO trainer for the colony AI brain.

Replaces the BC pipeline with proper outcome-driven RL:
- Actor-critic Gaussian policy over the 6-dim AiDecision space
- Outcome reward at match end (+1 win, -1 loss, graded workers_share on timeout)
- League self-play seeded with the 7 hardcoded archetypes as fixed exploiters
- Snapshots of the main agent join the league each round

Architecture decision: re-use the existing MlpBrain JSON weight format
(17->64->64->6 with ReLU + sigmoid + z-score normalization) so the Rust
side needs ZERO changes to inference. Training-time exploration is
handled by the noisy_mlp:<path>:<std> spec which adds Gaussian noise
to the sigmoid output. Eval is deterministic (mlp:<path> with std=0).

Per the May 2026 literature review (docs/ai-literature-review-2026-05.md):
the colony IS the RL agent; outcome reward + league play breaks the BC
ceiling that vanilla iteration cannot exceed (Ren et al. NeurIPS 2025).

Usage:
  python scripts/train_ppo_colony.py --iterations 50 \\
      --start bench/iterative-fsp/round_1/mlp_weights_v1.json \\
      --out bench/ppo-run/

Args:
  --iterations N          PPO update iterations (default 50)
  --matches-per-iter N    Matches collected per iter (default 32)
  --start <path>          Optional warm-start weights (else random init)
  --out <dir>             Output directory for weights + logs
  --explore-std FLOAT     Exploration noise std (default 0.2)
  --lr FLOAT              Adam lr (default 3e-4)
  --gamma FLOAT           Discount (default 0.99)
  --clip FLOAT            PPO clip ratio (default 0.2)
  --epochs-per-batch N    PPO epochs per batch (default 4)
  --eval-every N          Eval vs original 7 every N iterations (default 5)
  --snapshot-every N      Snapshot to league every N iterations (default 10)
"""
import argparse, json, os, subprocess, sys, time
from pathlib import Path
import random

import numpy as np
import torch
import torch.nn as nn
import torch.nn.functional as F
from torch.distributions import Normal
from torch.optim import Adam

sys.stdout.reconfigure(encoding='utf-8', errors='replace')

# Match MlpBrain layout in crates/antcolony-sim/src/ai/brain.rs
INPUT_DIM = 17
HIDDEN_DIM = 64
OUTPUT_DIM = 6  # caste W/S/B + behavior F/D/N

ARCHETYPES = ["heuristic","defender","aggressor","economist","breeder","forager","conservative"]
BENCH_EXE = "J:/antcolony/target/release/examples/matchup_bench.exe"

# ---------------------------------------------------------------------------

class ActorCritic(nn.Module):
    """Shared-trunk actor-critic mirroring MlpBrain inference architecture."""
    def __init__(self):
        super().__init__()
        # Actor: 17 -> 64 -> 64 -> 6, ReLU, sigmoid output
        self.actor_w1 = nn.Linear(INPUT_DIM, HIDDEN_DIM)
        self.actor_w2 = nn.Linear(HIDDEN_DIM, HIDDEN_DIM)
        self.actor_w3 = nn.Linear(HIDDEN_DIM, OUTPUT_DIM)
        # Critic: same shape but scalar output
        self.critic_w1 = nn.Linear(INPUT_DIM, HIDDEN_DIM)
        self.critic_w2 = nn.Linear(HIDDEN_DIM, HIDDEN_DIM)
        self.critic_w3 = nn.Linear(HIDDEN_DIM, 1)
        # Learnable per-dim log-std for the Gaussian action distribution.
        self.log_std = nn.Parameter(torch.full((OUTPUT_DIM,), -1.0))
        # Z-score normalization params (fit from corpus, frozen as buffers).
        self.register_buffer("input_mean", torch.zeros(INPUT_DIM))
        self.register_buffer("input_std", torch.ones(INPUT_DIM))

    def normalize(self, x):
        return (x - self.input_mean) / self.input_std

    def actor_forward(self, x):
        x = self.normalize(x)
        x = F.relu(self.actor_w1(x))
        x = F.relu(self.actor_w2(x))
        return torch.sigmoid(self.actor_w3(x))

    def critic_forward(self, x):
        x = self.normalize(x)
        x = F.relu(self.critic_w1(x))
        x = F.relu(self.critic_w2(x))
        return self.critic_w3(x).squeeze(-1)

    def policy(self, x):
        mean = self.actor_forward(x)
        std = self.log_std.exp().expand_as(mean)
        return Normal(mean, std)

    def export_mlp_weights(self, path):
        """Export actor weights in the MlpBrain JSON format the Rust side reads."""
        out = {
            "input_dim": INPUT_DIM,
            "hidden_dim": HIDDEN_DIM,
            "output_dim": OUTPUT_DIM,
            "input_mean": self.input_mean.tolist(),
            "input_std": self.input_std.tolist(),
            "w1": self.actor_w1.weight.detach().cpu().tolist(),
            "b1": self.actor_w1.bias.detach().cpu().tolist(),
            "w2": self.actor_w2.weight.detach().cpu().tolist(),
            "b2": self.actor_w2.bias.detach().cpu().tolist(),
            "w3": self.actor_w3.weight.detach().cpu().tolist(),
            "b3": self.actor_w3.bias.detach().cpu().tolist(),
        }
        with open(path, "w", encoding="utf-8") as f:
            json.dump(out, f)

    def load_mlp_weights(self, path):
        """Warm-start actor from an existing MlpBrain JSON file."""
        with open(path, "r", encoding="utf-8-sig") as f:
            d = json.load(f)
        with torch.no_grad():
            self.input_mean.copy_(torch.tensor(d["input_mean"], dtype=torch.float32))
            self.input_std.copy_(torch.tensor(d["input_std"], dtype=torch.float32))
            self.actor_w1.weight.copy_(torch.tensor(d["w1"], dtype=torch.float32))
            self.actor_w1.bias.copy_(torch.tensor(d["b1"], dtype=torch.float32))
            self.actor_w2.weight.copy_(torch.tensor(d["w2"], dtype=torch.float32))
            self.actor_w2.bias.copy_(torch.tensor(d["b2"], dtype=torch.float32))
            self.actor_w3.weight.copy_(torch.tensor(d["w3"], dtype=torch.float32))
            self.actor_w3.bias.copy_(torch.tensor(d["b3"], dtype=torch.float32))

# ---------------------------------------------------------------------------

def state_to_features(s):
    ed = s["enemy_distance_min"]
    if not (isinstance(ed, (int, float)) and ed < 1e9):
        ed = 1e6
    return [
        s["food_stored"], s["food_inflow_recent"],
        float(s["worker_count"]), float(s["soldier_count"]), float(s["breeder_count"]),
        float(s["brood_egg"]), float(s["brood_larva"]), float(s["brood_pupa"]),
        float(s["queens_alive"]), float(s["combat_losses_recent"]),
        float(ed), float(s["enemy_worker_count"]), float(s["enemy_soldier_count"]),
        float(s["day_of_year"]), float(s["ambient_temp_c"]),
        1.0 if s["diapause_active"] else 0.0,
        1.0 if s["is_daytime"] else 0.0,
    ]

def decision_to_vector(d):
    return [
        d["caste_ratio_worker"], d["caste_ratio_soldier"], d["caste_ratio_breeder"],
        d["forage_weight"], d["dig_weight"], d["nurse_weight"],
    ]

# ---------------------------------------------------------------------------

def play_match(weights_path, opponent_spec, seed, explore_std=0.2, max_ticks=10000):
    """Run one match via matchup_bench, return list of (state, action, reward) trajectories."""
    traj_path = Path(weights_path).parent / f"_tmp_traj_{os.getpid()}_{seed}.jsonl"
    cmd = [
        BENCH_EXE,
        "--left", f"noisy_mlp:{weights_path}:{explore_std}",
        "--right", opponent_spec,
        "--matches", "1",
        "--max-ticks", str(max_ticks),
        "--dump-trajectories", str(traj_path),
        "--left-seed", str(seed), "--right-seed", str(seed + 1),
    ]
    try:
        subprocess.run(cmd, capture_output=True, timeout=120, check=False)
    except subprocess.TimeoutExpired:
        print(f"  WARNING: match timed out at 120s")
        return []
    if not traj_path.exists():
        return []
    records = []
    try:
        with open(traj_path, "r", encoding="utf-8-sig") as f:
            for line in f:
                line = line.strip()
                if not line:
                    continue
                try:
                    r = json.loads(line)
                    if r["colony"] != 0:  # only the LEFT brain (our PPO agent) is the learner
                        continue
                    records.append({
                        "state": state_to_features(r["state"]),
                        "action": decision_to_vector(r["decision"]),
                        "reward": float(r["outcome_for_this_colony"]) * 2.0 - 1.0,  # [0,1] -> [-1, 1]
                    })
                except (json.JSONDecodeError, KeyError):
                    continue
    finally:
        try:
            traj_path.unlink()
        except Exception:
            pass
    return records

def evaluate(weights_path, n_matches=20, max_ticks=10000):
    """Eval current MLP vs each of the 7 archetypes. Returns (wins, total, per_arch_dict).

    matchup_bench's sim_seed = 100 + match_index, so we MUST call --matches N
    once per archetype and parse all N outcomes from the same stdout — calling
    --matches 1 N times gives N identical sims because match_index=0 always.
    """
    total_wins = 0
    total = 0
    per_arch = {}
    for arch in ARCHETYPES:
        wins = 0
        cmd = [
            BENCH_EXE,
            "--left", f"mlp:{weights_path}",
            "--right", arch,
            "--matches", str(n_matches),
            "--max-ticks", str(max_ticks),
        ]
        try:
            r = subprocess.run(cmd, capture_output=True, text=True, timeout=600, check=False)
            for line in r.stdout.splitlines():
                # Per-match line format: "  match   N: tick=NNN <status> winner=Some(K) ..."
                if "winner=Some(0)" in line and "match" in line:
                    wins += 1
        except subprocess.TimeoutExpired:
            pass
        per_arch[arch] = wins
        total_wins += wins
        total += n_matches
    return total_wins, total, per_arch

# ---------------------------------------------------------------------------

def compute_returns(rewards, gamma):
    """Monte Carlo returns. Reward is sparse (only at match end), so each
    record gets the discounted final-reward backed up to its timestep."""
    R = 0.0
    returns = []
    for r in reversed(rewards):
        R = r + gamma * R
        returns.insert(0, R)
    return returns

def ppo_update(model, optimizer, states, actions, returns, advantages, old_log_probs,
               clip=0.2, epochs=4, batch_size=512, value_coef=0.5, entropy_coef=0.01):
    states = torch.tensor(states, dtype=torch.float32, device=next(model.parameters()).device)
    actions = torch.tensor(actions, dtype=torch.float32, device=states.device)
    returns = torch.tensor(returns, dtype=torch.float32, device=states.device)
    advantages = torch.tensor(advantages, dtype=torch.float32, device=states.device)
    old_log_probs = torch.tensor(old_log_probs, dtype=torch.float32, device=states.device)
    advantages = (advantages - advantages.mean()) / (advantages.std() + 1e-8)

    n = len(states)
    losses = []
    for _ in range(epochs):
        perm = torch.randperm(n)
        for start in range(0, n, batch_size):
            idx = perm[start:start + batch_size]
            dist = model.policy(states[idx])
            new_log_probs = dist.log_prob(actions[idx]).sum(-1)
            ratio = (new_log_probs - old_log_probs[idx]).exp()
            surr1 = ratio * advantages[idx]
            surr2 = torch.clamp(ratio, 1 - clip, 1 + clip) * advantages[idx]
            policy_loss = -torch.min(surr1, surr2).mean()
            value_pred = model.critic_forward(states[idx])
            value_loss = F.mse_loss(value_pred, returns[idx])
            entropy = dist.entropy().sum(-1).mean()
            loss = policy_loss + value_coef * value_loss - entropy_coef * entropy
            optimizer.zero_grad()
            loss.backward()
            torch.nn.utils.clip_grad_norm_(model.parameters(), 0.5)
            optimizer.step()
            losses.append(loss.item())
    return sum(losses) / max(len(losses), 1)

# ---------------------------------------------------------------------------

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--iterations", type=int, default=50)
    ap.add_argument("--matches-per-iter", type=int, default=32)
    ap.add_argument("--start", type=str, default=None)
    ap.add_argument("--out", type=str, required=True)
    ap.add_argument("--explore-std", type=float, default=0.2)
    ap.add_argument("--lr", type=float, default=3e-4)
    ap.add_argument("--gamma", type=float, default=0.99)
    ap.add_argument("--clip", type=float, default=0.2)
    ap.add_argument("--epochs-per-batch", type=int, default=4)
    ap.add_argument("--eval-every", type=int, default=5)
    ap.add_argument("--snapshot-every", type=int, default=10)
    ap.add_argument("--device", type=str, default="cuda" if torch.cuda.is_available() else "cpu")
    args = ap.parse_args()

    out_dir = Path(args.out)
    out_dir.mkdir(parents=True, exist_ok=True)
    log_path = out_dir / "ppo_train.log"
    league_dir = out_dir / "league"
    league_dir.mkdir(exist_ok=True)
    weights_path = out_dir / "current.json"

    def log(msg):
        ts = time.strftime("%H:%M:%S")
        line = f"[{ts}] {msg}"
        print(line)
        with open(log_path, "a", encoding="utf-8") as f:
            f.write(line + "\n")

    log(f"=== PPO trainer starting ({args.iterations} iters, {args.matches_per_iter} matches/iter, device={args.device}) ===")

    model = ActorCritic().to(args.device)
    if args.start:
        log(f"warm-start from {args.start}")
        model.load_mlp_weights(args.start)
    optimizer = Adam(model.parameters(), lr=args.lr)
    model.export_mlp_weights(weights_path)

    # League: dict of name -> matchup_bench spec.
    # Seed with 7 hardcoded archetypes as fixed exploiters.
    league = {arch: arch for arch in ARCHETYPES}

    rng = random.Random(0)
    for it in range(1, args.iterations + 1):
        # Sample opponents from league with weight on archetypes early,
        # past selves later.
        all_states, all_actions, all_rewards, all_log_probs = [], [], [], []
        episode_lens = []
        for m in range(args.matches_per_iter):
            opp_name = rng.choice(list(league.keys()))
            opp_spec = league[opp_name]
            traj = play_match(str(weights_path), opp_spec, seed=10000 * it + m,
                              explore_std=args.explore_std)
            if not traj:
                continue
            episode_lens.append(len(traj))
            states = [r["state"] for r in traj]
            actions = [r["action"] for r in traj]
            # Sparse reward: only the last step gets the actual outcome
            # (already encoded in each record but they're all the same
            # value since the bench writes the final outcome to all
            # records of the match). We treat as terminal-only for GAE.
            ep_reward = traj[-1]["reward"]
            rewards = [0.0] * (len(traj) - 1) + [ep_reward]
            returns = compute_returns(rewards, args.gamma)

            # Compute log_probs of actions under the CURRENT policy
            # (since trajectories were collected with the current
            # weights — slight off-policyness from noise but acceptable).
            with torch.no_grad():
                s_t = torch.tensor(states, dtype=torch.float32, device=args.device)
                a_t = torch.tensor(actions, dtype=torch.float32, device=args.device)
                dist = model.policy(s_t)
                lp = dist.log_prob(a_t).sum(-1).cpu().numpy().tolist()
                values = model.critic_forward(s_t).cpu().numpy().tolist()
            advantages = [returns[i] - values[i] for i in range(len(returns))]

            all_states.extend(states)
            all_actions.extend(actions)
            all_rewards.extend(returns)
            all_log_probs.extend(lp)

        if not all_states:
            log(f"  iter {it}: NO TRAJECTORIES — skipping")
            continue

        # Compute advantages from the collected returns (already done
        # per-episode above; here we just recompute against fresh values
        # since the model hasn't been updated yet this iter).
        with torch.no_grad():
            s_all = torch.tensor(all_states, dtype=torch.float32, device=args.device)
            v_all = model.critic_forward(s_all).cpu().numpy().tolist()
        advantages = [all_rewards[i] - v_all[i] for i in range(len(all_rewards))]

        loss = ppo_update(model, optimizer, all_states, all_actions, all_rewards,
                          advantages, all_log_probs,
                          clip=args.clip, epochs=args.epochs_per_batch)
        avg_ep_len = sum(episode_lens) / max(len(episode_lens), 1)
        avg_reward = sum(r["reward"] for ep in [[{"reward": all_rewards[-1]}]] for r in ep) / max(len(episode_lens), 1)
        log(f"  iter {it}: {len(all_states)} samples, {len(episode_lens)} eps, avg_ep_len={avg_ep_len:.0f}, loss={loss:.4f}")

        # Update weights file for next iteration's matches.
        model.export_mlp_weights(weights_path)

        if it % args.eval_every == 0:
            wins, total, per = evaluate(str(weights_path), n_matches=10)
            pct = 100.0 * wins / max(total, 1)
            per_str = ", ".join(f"{k[:4]}:{v}/10" for k, v in per.items())
            log(f"  *** iter {it} EVAL vs original 7: {wins}/{total}  ({pct:.1f}%)  [{per_str}]")

        if it % args.snapshot_every == 0:
            snap_path = league_dir / f"main_v{it}.json"
            model.export_mlp_weights(snap_path)
            league[f"main_v{it}"] = f"mlp:{snap_path}"
            log(f"  snapshot main_v{it} -> league (now {len(league)} opponents)")

    # Final eval
    log("=== Final eval (20 matches per archetype) ===")
    wins, total, per = evaluate(str(weights_path), n_matches=20)
    pct = 100.0 * wins / max(total, 1)
    per_str = ", ".join(f"{k}:{v}/20" for k, v in per.items())
    log(f"*** FINAL vs original 7: {wins}/{total}  ({pct:.1f}%)")
    log(f"    per-archetype: {per_str}")
    log(f"=== Done. Best weights: {weights_path} ===")

if __name__ == "__main__":
    main()
