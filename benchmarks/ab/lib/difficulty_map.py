#!/usr/bin/env python3
"""Assign difficulty bands to every SWE-bench Verified instance.

METHODOLOGY §2.1. NOTE (correction): SWE-bench Verified DOES ship a native
`difficulty` column (human time-estimate) — this is more authoritative than the
patch proxy, so it is the PRIMARY band. The patch proxy (files_touched / net_loc /
f2p_count quartiles) is kept as a secondary signal for cross-checking and for
balancing, because the native field is skewed (B3+B4 = only ~45/500).

Bands:
  B1 easy    : native "<15 min fix"
  B2 medium  : native "15 min - 1 hour"
  B3 hard    : native "1-4 hours"
  B4 hardest : native ">4 hours"

Trivial flag (skill-engagement capacity): band==B1 AND files_touched<=1 AND
f2p_count<=1. NETS pre-flight gate wants non-trivial fraction > 85%.

Usage:  python difficulty_map.py --out difficulty_map.json
"""
from __future__ import annotations
import argparse, json, re, statistics
from datasets import load_dataset

NATIVE_TO_BAND = {
    "<15 min fix": "B1",
    "15 min - 1 hour": "B2",
    "1-4 hours": "B3",
    ">4 hours": "B4",
}

DIFF_GIT = re.compile(r"^diff --git a/(\S+) b/(\S+)$", re.MULTILINE)
HUNK_ADDED = re.compile(r"^\+(?!\+)", re.MULTILINE)
HUNK_REMOVED = re.compile(r"^-(?!-)", re.MULTILINE)


def parse_patch_stats(patch: str, f2p_raw: str) -> dict:
    files = sorted({m.group(2) for m in DIFF_GIT.finditer(patch or "")})
    files_touched = len(files)
    net_loc = len(HUNK_ADDED.findall(patch or "")) + len(HUNK_REMOVED.findall(patch or ""))
    try:
        f2p = json.loads(f2p_raw) if f2p_raw else []
        f2p_count = len(f2p) if isinstance(f2p, list) else 0
    except json.JSONDecodeError:
        f2p_count = 0
    return {"files_touched": files_touched, "net_loc": net_loc,
            "f2p_count": f2p_count, "files": files}


def zscore(values: list[float]) -> list[float]:
    if len(values) < 2:
        return [0.0] * len(values)
    mu, sd = statistics.mean(values), statistics.pstdev(values)
    if sd == 0:
        return [0.0] * len(values)
    return [(v - mu) / sd for v in values]


def quantile_band(score: float, cuts: tuple[float, float, float]) -> str:
    q1, q2, q3 = cuts
    if score <= q1:
        return "B1"
    if score <= q2:
        return "B2"
    if score <= q3:
        return "B3"
    return "B4"


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", default="difficulty_map.json")
    ap.add_argument("--dataset", default="princeton-nlp/SWE-bench_Verified")
    args = ap.parse_args()

    ds = load_dataset(args.dataset, split="test")
    rows = []
    for r in ds:
        st = parse_patch_stats(r["patch"], r.get("FAIL_TO_PASS", "[]"))
        native = r.get("difficulty", "")
        rows.append({
            "instance_id": r["instance_id"], "repo": r["repo"],
            "base_commit": r["base_commit"], "version": r.get("version"),
            "native_difficulty": native, "band": NATIVE_TO_BAND.get(native, "B?"),
            **st,
        })

    # proxy score = z(net_loc) + z(files_touched) + z(f2p_count); quartile bands
    zloc = zscore([r["net_loc"] for r in rows])
    zfil = zscore([r["files_touched"] for r in rows])
    zf2p = zscore([r["f2p_count"] for r in rows])
    scores = [zloc[i] + zfil[i] + zf2p[i] for i in range(len(rows))]
    ssorted = sorted(scores)
    n = len(ssorted)
    cuts = (ssorted[n // 4 - 1], ssorted[n // 2 - 1], ssorted[3 * n // 4 - 1])
    for i, r in enumerate(rows):
        r["proxy_score"] = round(scores[i], 4)
        r["proxy_band"] = quantile_band(scores[i], cuts)
        r["trivial"] = (r["band"] == "B1" and r["files_touched"] <= 1 and r["f2p_count"] <= 1)

    out = {r["instance_id"]: r for r in rows}
    with open(args.out, "w") as f:
        json.dump(out, f, indent=2)

    from collections import Counter
    band_counts = Counter(r["band"] for r in rows)
    proxy_counts = Counter(r["proxy_band"] for r in rows)
    trivial_n = sum(1 for r in rows if r["trivial"])
    nets = 1 - trivial_n / len(rows)
    agree = sum(1 for r in rows if r["band"] == r["proxy_band"]) / len(rows)
    print(f"wrote {args.out}: {len(rows)} instances")
    print(f"native band counts : {dict(sorted(band_counts.items()))}")
    print(f"proxy  band counts : {dict(sorted(proxy_counts.items()))}")
    print(f"native/proxy agree  : {agree:.1%}")
    print(f"trivial instances   : {trivial_n} ({trivial_n/len(rows):.1%})")
    print(f"NETS coverage       : {nets:.1%}  (gate: >85%)")
    print(f"hard (B3+B4) total  : {band_counts.get('B3',0)+band_counts.get('B4',0)}  <- statistical-power flag")


if __name__ == "__main__":
    main()
