#!/usr/bin/env python3
"""GPU MLP trainer for the antcolony AI brain.

Reads matchup_bench JSONL trajectories, trains a small MLP that
maps ColonyAiState -> AiDecision, saves weights as JSON readable by
the Rust-side MlpBrain. CUDA on by default; falls back to CPU.

Wire format (matches Rust ColonyAiState / AiDecision in
crates/antcolony-sim/src/ai/brain.rs):

  Input features (17, in this exact order):
    food_stored, food_inflow_recent, worker_count, soldier_count,
    breeder_count, brood_egg, brood_larva, brood_pupa, queens_alive,
    combat_losses_recent, enemy_distance_min (inf -> 1e6),
    enemy_worker_count, enemy_soldier_count, day_of_year,
    ambient_temp_c, diapause_active (0/1), is_daytime (0/1)

  Output (6, sigmoid'd then renormalized client-side):
    caste_ratio_worker, caste_ratio_soldier, caste_ratio_breeder,
    forage_weight, dig_weight, nurse_weight

Outcome-weighted: each (state, decision) tuple's loss is weighted by
its `outcome_for_this_colony` so we behavior-clone winners more than
losers.

Usage:
    python scripts/train_mlp_brain.py \\
        --trajectories bench/ai-train-run/trajectories_filtered.jsonl \\
        --out bench/ai-train-run/mlp_weights.json \\
        --hidden 64 --epochs 50 --lr 1e-3 --device cuda

Output JSON shape:
    {
      "input_dim": 17,
      "hidden_dim": 64,
      "output_dim": 6,
      "input_mean":  [...17 floats...],   # for input normalization
      "input_std":   [...17 floats...],
      "w1": [[...]], "b1": [...],          # 17 -> hidden
      "w2": [[...]], "b2": [...],          # hidden -> hidden
      "w3": [[...]], "b3": [...]           # hidden -> 6
    }
"""

from __future__ import annotations

import argparse
import json
import math
import sys
from pathlib import Path

import torch
import torch.nn as nn
import torch.nn.functional as F


INPUT_DIM = 17
OUTPUT_DIM = 6
INF_PLACEHOLDER = 1e6


def state_to_features(state: dict) -> list[float]:
    ed = state.get("enemy_distance_min")
    if ed is None or (isinstance(ed, float) and math.isinf(ed)) or ed == "inf":
        ed = INF_PLACEHOLDER
    return [
        float(state["food_stored"]),
        float(state["food_inflow_recent"]),
        float(state["worker_count"]),
        float(state["soldier_count"]),
        float(state["breeder_count"]),
        float(state["brood_egg"]),
        float(state["brood_larva"]),
        float(state["brood_pupa"]),
        float(state["queens_alive"]),
        float(state["combat_losses_recent"]),
        float(ed),
        float(state["enemy_worker_count"]),
        float(state["enemy_soldier_count"]),
        float(state["day_of_year"]),
        float(state["ambient_temp_c"]),
        1.0 if state["diapause_active"] else 0.0,
        1.0 if state["is_daytime"] else 0.0,
    ]


def decision_to_targets(decision: dict) -> list[float]:
    return [
        float(decision["caste_ratio_worker"]),
        float(decision["caste_ratio_soldier"]),
        float(decision["caste_ratio_breeder"]),
        float(decision["forage_weight"]),
        float(decision["dig_weight"]),
        float(decision["nurse_weight"]),
    ]


def load_dataset(path: Path) -> tuple[torch.Tensor, torch.Tensor, torch.Tensor]:
    xs, ys, ws = [], [], []
    # encoding='utf-8-sig' strips a UTF-8 BOM if present (PowerShell's
    # Set-Content -Encoding utf8 writes a BOM by default; without this
    # json.loads chokes on the leading bytes).
    with path.open(encoding="utf-8-sig") as f:
        for line in f:
            line = line.strip().lstrip("﻿")
            if not line:
                continue
            r = json.loads(line)
            xs.append(state_to_features(r["state"]))
            ys.append(decision_to_targets(r["decision"]))
            ws.append(float(r.get("outcome_for_this_colony", 0.5)))
    if not xs:
        raise SystemExit(f"no trajectories found in {path}")
    return (
        torch.tensor(xs, dtype=torch.float32),
        torch.tensor(ys, dtype=torch.float32),
        torch.tensor(ws, dtype=torch.float32),
    )


class MlpBrain(nn.Module):
    def __init__(self, input_dim: int, hidden_dim: int, output_dim: int):
        super().__init__()
        self.fc1 = nn.Linear(input_dim, hidden_dim)
        self.fc2 = nn.Linear(hidden_dim, hidden_dim)
        self.fc3 = nn.Linear(hidden_dim, output_dim)

    def forward(self, x: torch.Tensor) -> torch.Tensor:
        h = F.relu(self.fc1(x))
        h = F.relu(self.fc2(h))
        return torch.sigmoid(self.fc3(h))


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--trajectories", required=True, type=Path)
    ap.add_argument("--out", required=True, type=Path)
    ap.add_argument("--hidden", type=int, default=64)
    ap.add_argument("--epochs", type=int, default=50)
    ap.add_argument("--batch", type=int, default=256)
    ap.add_argument("--lr", type=float, default=1e-3)
    ap.add_argument("--device", default="cuda")
    ap.add_argument("--seed", type=int, default=42)
    args = ap.parse_args()

    torch.manual_seed(args.seed)
    device = torch.device(args.device if torch.cuda.is_available() or args.device == "cpu" else "cpu")
    print(f"[train_mlp_brain] device={device} hidden={args.hidden} epochs={args.epochs} lr={args.lr}")

    X, Y, W = load_dataset(args.trajectories)
    print(f"[train_mlp_brain] dataset: {X.shape[0]} records, "
          f"X={tuple(X.shape)}, Y={tuple(Y.shape)}")

    # Normalize inputs (cheap z-score; helps the model learn faster).
    mean = X.mean(dim=0)
    std = X.std(dim=0).clamp(min=1e-6)
    X_norm = (X - mean) / std

    X_norm = X_norm.to(device)
    Y = Y.to(device)
    W = W.to(device)

    model = MlpBrain(INPUT_DIM, args.hidden, OUTPUT_DIM).to(device)
    opt = torch.optim.Adam(model.parameters(), lr=args.lr)

    n = X_norm.shape[0]
    for epoch in range(args.epochs):
        perm = torch.randperm(n, device=device)
        total_loss = 0.0
        n_batches = 0
        for i in range(0, n, args.batch):
            idx = perm[i:i + args.batch]
            xb = X_norm[idx]
            yb = Y[idx]
            wb = W[idx].unsqueeze(-1)  # (B, 1) for broadcasting
            pred = model(xb)
            # Outcome-weighted MSE so winning trajectories carry more loss.
            sq = (pred - yb).pow(2)
            loss = (sq * wb).mean()
            opt.zero_grad()
            loss.backward()
            opt.step()
            total_loss += loss.item()
            n_batches += 1
        avg = total_loss / max(n_batches, 1)
        if epoch % 5 == 0 or epoch == args.epochs - 1:
            print(f"  epoch {epoch:3d}  loss={avg:.5f}")

    # Export weights to JSON.
    state = {
        "input_dim": INPUT_DIM,
        "hidden_dim": args.hidden,
        "output_dim": OUTPUT_DIM,
        "input_mean": mean.tolist(),
        "input_std": std.tolist(),
        "w1": model.fc1.weight.detach().cpu().tolist(),
        "b1": model.fc1.bias.detach().cpu().tolist(),
        "w2": model.fc2.weight.detach().cpu().tolist(),
        "b2": model.fc2.bias.detach().cpu().tolist(),
        "w3": model.fc3.weight.detach().cpu().tolist(),
        "b3": model.fc3.bias.detach().cpu().tolist(),
    }
    args.out.parent.mkdir(parents=True, exist_ok=True)
    with args.out.open("w") as f:
        json.dump(state, f)
    print(f"[train_mlp_brain] wrote weights -> {args.out} ({args.out.stat().st_size} bytes)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
