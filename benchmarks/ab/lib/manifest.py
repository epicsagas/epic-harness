#!/usr/bin/env python3
"""Build the run manifest consumed by run_swebench.sh.

Joins the SWE-bench Verified dataset (repo, base_commit, problem_statement) with the
committed difficulty_map.json (band) and security_subset.json (security_relevant) into one
JSONL the runner can stream without touching HuggingFace at run time.

Usage:
  python manifest.py --instances all                    --out manifest.jsonl
  python manifest.py --instances sympy__sympy-23950,... --out manifest.jsonl
  python manifest.py --band B3,B4 --limit 20            --out manifest.jsonl   # oversample hard
"""
from __future__ import annotations
import argparse, json
from datasets import load_dataset


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", default="manifest.jsonl")
    ap.add_argument("--dataset", default="princeton-nlp/SWE-bench_Verified")
    ap.add_argument("--difficulty-map", default="difficulty_map.json")
    ap.add_argument("--security-subset", default="security_subset.json")
    ap.add_argument("--instances", default="all", help="comma list or 'all'")
    ap.add_argument("--band", default="", help="restrict to bands, e.g. B3,B4")
    ap.add_argument("--non-trivial", action="store_true", help="exclude trivial flag")
    ap.add_argument("--limit", type=int, default=0, help="cap N (0=no cap)")
    args = ap.parse_args()

    dmap = json.load(open(args.difficulty_map))
    sec = json.load(open(args.security_subset))["instances"]
    want = None if args.instances == "all" else set(args.instances.split(","))
    bands = set(args.band.split(",")) if args.band else None

    ds = load_dataset(args.dataset, split="test")
    n = 0
    with open(args.out, "w") as f:
        for r in ds:
            iid = r["instance_id"]
            if want is not None and iid not in want:
                continue
            m = dmap.get(iid, {})
            if bands and m.get("band") not in bands:
                continue
            if args.non_trivial and m.get("trivial"):
                continue
            f.write(json.dumps({
                "instance_id": iid,
                "repo": r["repo"],
                "base_commit": r["base_commit"],
                "problem_statement": r["problem_statement"],
                "band": m.get("band", "B?"),
                "trivial": m.get("trivial", False),
                "security_relevant": sec.get(iid, {}).get("security_relevant", False),
            }) + "\n")
            n += 1
            if args.limit and n >= args.limit:
                break
    print(f"wrote {args.out}: {n} instances"
          + (f"  bands={args.band}" if args.band else "")
          + ("  (non-trivial)" if args.non_trivial else ""))


if __name__ == "__main__":
    main()
