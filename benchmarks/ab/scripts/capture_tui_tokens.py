#!/usr/bin/env python3
"""
Aggregate token usage from a Claude Code session JSONL between two ISO8601 timestamps.

Usage:
    capture_tui_tokens.py --jsonl <path> --start <ISO8601> --end <ISO8601>

Output (JSON to stdout):
    {
      "model": "claude-sonnet-4-6",
      "input_tokens": 12345,
      "output_tokens": 678,
      "num_turns": 4,
      "duration_ms": 15000
    }

Counts only `assistant` records with a non-synthetic model name that fall
between --start (inclusive) and --end (inclusive).
"""

import argparse
import json
import sys
from datetime import datetime, timezone


def parse_iso(s: str) -> datetime:
    # Accept both Z and +00:00 suffixes
    s = s.replace("Z", "+00:00")
    return datetime.fromisoformat(s)


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--jsonl", required=True, help="Path to session .jsonl file")
    ap.add_argument("--start", required=True, help="Start timestamp (ISO8601, inclusive)")
    ap.add_argument("--end", required=True, help="End timestamp (ISO8601, inclusive)")
    ap.add_argument(
        "--model-filter",
        default=None,
        help="Only include records for this model (default: last non-synthetic model)",
    )
    args = ap.parse_args()

    try:
        t_start = parse_iso(args.start)
        t_end = parse_iso(args.end)
    except ValueError as e:
        print(f"ERROR: invalid timestamp: {e}", file=sys.stderr)
        sys.exit(1)

    input_tokens = 0
    output_tokens = 0
    num_turns = 0
    last_model = None
    first_ts: datetime | None = None
    last_ts: datetime | None = None

    try:
        with open(args.jsonl) as fh:
            for line in fh:
                line = line.strip()
                if not line:
                    continue
                try:
                    rec = json.loads(line)
                except json.JSONDecodeError:
                    continue

                if rec.get("type") != "assistant":
                    continue

                ts_raw = rec.get("timestamp")
                if not ts_raw:
                    continue
                try:
                    ts = parse_iso(ts_raw)
                except ValueError:
                    continue

                if ts < t_start or ts > t_end:
                    continue

                msg = rec.get("message", {})
                model = msg.get("model", "")
                if model and model != "<synthetic>":
                    if args.model_filter and model != args.model_filter:
                        continue
                    last_model = model

                usage = msg.get("usage", {})
                inp = usage.get("input_tokens", 0) or 0
                out = usage.get("output_tokens", 0) or 0

                input_tokens += inp
                output_tokens += out
                num_turns += 1

                if first_ts is None:
                    first_ts = ts
                last_ts = ts

    except FileNotFoundError:
        print(f"ERROR: file not found: {args.jsonl}", file=sys.stderr)
        sys.exit(1)

    if num_turns == 0:
        print(
            "WARNING: no assistant records found in the specified time range",
            file=sys.stderr,
        )

    duration_ms = 0
    if first_ts and last_ts:
        duration_ms = int((last_ts - first_ts).total_seconds() * 1000)

    result = {
        "model": last_model or "unknown",
        "input_tokens": input_tokens,
        "output_tokens": output_tokens,
        "num_turns": num_turns,
        "duration_ms": duration_ms,
    }
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
