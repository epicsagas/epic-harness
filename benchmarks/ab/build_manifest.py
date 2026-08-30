#!/usr/bin/env python3
"""Build a run_swebench.sh manifest.jsonl from difficulty_map.json.

The preregistered A/B (benchmarks/ab/preregistration.md) samples instances
per difficulty band. This emits the manifest run_swebench.sh consumes:
one JSON object per line with instance_id/repo/base_commit/band.

Usage:
  python3 build_manifest.py --per-band 5 --seed 7 --out manifest.jsonl   # pilot (5/band)
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

BANDS = ["B1", "B2", "B3", "B4"]


def load_map(path: Path) -> dict:
    data = json.loads(path.read_text())
    if not isinstance(data, dict):
        sys.exit(f"ERR: {path} is not a {{instance_id: ...}} map")
    return data


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    here = Path(__file__).resolve().parent
    ap.add_argument("--map", type=Path, default=here / "difficulty_map.json")
    ap.add_argument("--out", type=Path, default=here / "manifest.jsonl")
    ap.add_argument("--per-band", type=int, default=5,
                    help="instances sampled per band (default 5 = pilot size)")
    ap.add_argument("--bands", default=",".join(BANDS),
                    help=f"comma-separated bands to include (default {','.join(BANDS)})")
    ap.add_argument("--seed", type=int, default=7,
                    help="RNG seed; same seed = same sample (reproducible pilots)")
    ap.add_argument("--list-bands", action="store_true",
                    help="print per-band counts from the map and exit")
    args = ap.parse_args()

    entries = load_map(args.map)
    by_band: dict[str, list[dict]] = defaultdict(list)
    for row in entries.values():
        band = row.get("band")
        if band in BANDS:
            by_band[band].append(row)

    if args.list_bands:
        for band in BANDS:
            print(f"{band}: {len(by_band[band])}")
        return

    wanted = [b.strip().upper() for b in args.bands.split(",") if b.strip()]
    unknown = [b for b in wanted if b not in BANDS]
    if unknown:
        sys.exit(f"ERR: unknown band(s): {', '.join(unknown)}; valid: {','.join(BANDS)}")

    rng = random.Random(args.seed)
    lines: list[str] = []
    for band in wanted:
        pool = sorted(by_band[band], key=lambda r: r["instance_id"])
        if len(pool) < args.per_band:
            sys.exit(f"ERR: band {band} has {len(pool)} instances < --per-band {args.per_band}")
        for row in rng.sample(pool, args.per_band):
            lines.append(json.dumps({
                "instance_id": row["instance_id"],
                "repo": row["repo"],
                "base_commit": row["base_commit"],
                "band": band,
            }))
    args.out.write_text("\n".join(lines) + "\n")
    print(f"wrote {len(lines)} instances ({args.per_band}/band x {len(wanted)} bands, seed={args.seed}) -> {args.out}")


if __name__ == "__main__":
    main()
