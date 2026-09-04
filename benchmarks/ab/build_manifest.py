#!/usr/bin/env python3
"""Build a run_swebench.sh manifest.jsonl from difficulty_map.json and SWE-bench_Verified.

The preregistered A/B (benchmarks/ab/preregistration.md) samples instances
per difficulty band. This emits the manifest run_swebench.sh consumes:
one JSON object per line with instance_id/repo/base_commit/band/problem_statement.

Usage:
  python3 build_manifest.py --per-band 5 --seed 7 --out manifest.jsonl   # pilot (5/band = 20 total)
  python3 build_manifest.py --per-band 125 --seed 7 --out manifest.jsonl # full run
  python3 build_manifest.py --list-bands
"""
from __future__ import annotations

import argparse
import json
import random
import sys
from collections import defaultdict
from pathlib import Path

from datasets import load_dataset

BANDS = ["B1", "B2", "B3", "B4"]


def load_map(path: Path) -> dict:
    if not path.is_file():
        sys.exit(f"ERR: {path} not found")
    data = json.loads(path.read_text())
    if not isinstance(data, dict):
        sys.exit(f"ERR: {path} is not a {{instance_id: ...}} map")
    return data


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    here = Path(__file__).resolve().parent
    ap.add_argument("--map", type=Path, default=here / "difficulty_map.json")
    ap.add_argument("--security-subset", type=Path, default=here / "security_subset.json")
    ap.add_argument("--dataset", default="princeton-nlp/SWE-bench_Verified")
    ap.add_argument("--out", type=Path, default=here / "manifest.jsonl")
    ap.add_argument("--per-band", type=int, default=5,
                    help="instances sampled per band (default 5 = pilot size)")
    ap.add_argument("--band-source", choices=["proxy_band", "band"], default="proxy_band",
                    help="band field to use ('proxy_band' for 4 equal quartiles, 'band' for native time estimates)")
    ap.add_argument("--bands", default=",".join(BANDS),
                    help=f"comma-separated bands to include (default {','.join(BANDS)})")
    ap.add_argument("--seed", type=int, default=7,
                    help="RNG seed; same seed = same sample (reproducible pilots)")
    ap.add_argument("--list-bands", action="store_true",
                    help="print per-band counts from the map and exit")
    args = ap.parse_args()

    entries = load_map(args.map)

    if args.list_bands:
        for src in ["proxy_band", "band"]:
            counts: dict[str, int] = defaultdict(int)
            for row in entries.values():
                b = row.get(src)
                if b in BANDS:
                    counts[b] += 1
            print(f"[{src} counts] " + ", ".join(f"{b}: {counts[b]}" for b in BANDS))
        return

    sec_data = {}
    if args.security_subset.is_file():
        try:
            sec_data = json.loads(args.security_subset.read_text()).get("instances", {})
        except Exception:
            pass

    by_band: dict[str, list[dict]] = defaultdict(list)
    for row in entries.values():
        band = row.get(args.band_source)
        if band in BANDS:
            by_band[band].append(row)

    wanted = [b.strip().upper() for b in args.bands.split(",") if b.strip()]
    unknown = [b for b in wanted if b not in BANDS]
    if unknown:
        sys.exit(f"ERR: unknown band(s): {', '.join(unknown)}; valid: {','.join(BANDS)}")

    rng = random.Random(args.seed)
    chosen_ids = set()
    sampled_rows: list[tuple[str, dict]] = []

    for band in wanted:
        pool = sorted(by_band[band], key=lambda r: r["instance_id"])
        sample_size = min(len(pool), args.per_band)
        if len(pool) < args.per_band:
            print(f"WARN: band {band} has only {len(pool)} instances (requested {args.per_band}), taking all {len(pool)}", file=sys.stderr)
        for row in rng.sample(pool, sample_size):
            chosen_ids.add(row["instance_id"])
            sampled_rows.append((band, row))

    # Load problem statements from SWE-bench Verified
    print(f"Loading {args.dataset} to retrieve problem statements for {len(chosen_ids)} instances...", file=sys.stderr)
    ds = load_dataset(args.dataset, split="test")
    ds_by_id = {r["instance_id"]: r for r in ds if r["instance_id"] in chosen_ids}

    lines: list[str] = []
    for band, row in sampled_rows:
        iid = row["instance_id"]
        ds_row = ds_by_id.get(iid)
        problem_statement = ds_row["problem_statement"] if ds_row else ""
        repo = row.get("repo") or (ds_row["repo"] if ds_row else "")
        base_commit = row.get("base_commit") or (ds_row["base_commit"] if ds_row else "")
        sec_info = sec_data.get(iid, {})

        lines.append(json.dumps({
            "instance_id": iid,
            "repo": repo,
            "base_commit": base_commit,
            "band": band,
            "problem_statement": problem_statement,
            "trivial": row.get("trivial", False),
            "security_relevant": sec_info.get("security_relevant", False),
        }))

    args.out.write_text("\n".join(lines) + "\n")
    print(f"wrote {len(lines)} instances ({args.per_band}/band x {len(wanted)} bands, source={args.band_source}, seed={args.seed}) -> {args.out}")


if __name__ == "__main__":
    main()

