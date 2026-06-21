#!/usr/bin/env python3
"""Replay a claude `--output-format stream-json` transcript into the same observation
model the epic-harness Rust uses, then run pattern detection — identically for BOTH arms.

This is the BLOCKER-3 instrument (METHODOLOGY §6.2). It reimplements, in Python, the
exact semantics of:
  - src/shared/classify.rs  : classify_failure (FAILURE_RULES), classify_tool, extract_file
  - src/shared/helpers.rs   : hash_string, normalize_error
  - src/evolve/analysis.rs  : detect_patterns (4 patterns)
Thresholds are PINNED from src/config.rs (PatternConfig::default) so a future harness change
cannot silently re-grade a run. If you bump the Rust defaults, bump PINNED_THRESHOLDS here
and record it in the report's pattern-threshold snapshot.

CLI:  python replay.py <transcript.ndjson>            # print patterns + summary
Lib:  from replay import replay; replay(path) -> dict
"""
from __future__ import annotations
import argparse, json, re, sys
from collections import Counter
from pathlib import Path

# ── PINNED from src/config.rs PatternConfig::default ──────────────────────────────
PINNED_THRESHOLDS = {
    "repeated_error_min": 3,
    "ftb_lookahead": 3,
    "ftb_min_cycles": 2,
    "debug_loop_min": 5,
    "thrash_min_edits": 3,
    "thrash_min_errors": 3,
}

# ── hash_string (helpers.rs:245) — u32 wrapping, hash*31 + byte, 8-hex ─────────────
def hash_string(s: str) -> str:
    h = 0
    for b in s.encode("utf-8", "ignore"):
        h = ((h << 5) - h + b) & 0xFFFFFFFF
    return f"{h:08x}"

# ── normalize_error (helpers.rs:256) ───────────────────────────────────────────────
_TS = re.compile(r"\d{4}-\d{2}-\d{2}[T ]\d{2}:\d{2}:\d{2}[.\dZ]*")
_LC = re.compile(r":\d+:\d+")
_PATH = re.compile(r"/[\w./-]+/")
_WS = re.compile(r"\s+")

def normalize_error(snippet: str) -> str:
    s = _TS.sub("", snippet)
    s = _LC.sub(":L:C", s)
    s = _PATH.sub("/PATH/", s)
    s = _WS.sub(" ", s).strip()
    return s[:200]

# ── classify_failure (classify.rs:58 + FAILURE_RULES) ──────────────────────────────
# (regex, flags, category) — flags ported from Rust inline (?i)/(?m).
_FAILURE_RULES = [
    (r"TypeError|type error", re.I, "type_error"),
    (r"SyntaxError|Unexpected token|Parse error", re.I, "syntax_error"),
    (r"FAIL(?:ED|ING)?[\s:]|test.*fail|AssertionError|assert\.\w+", re.I, "test_fail"),
    (r"\blint\b.*(?:error|fail)|eslint.*error|biome.*error|oxlint.*error", re.I, "lint_fail"),
    (r"build.*fail|tsc.*error|error TS\d+|compilation.*fail", re.I, "build_fail"),
    (r"EACCES|permission denied", re.I, "permission_denied"),
    (r"timeout|ETIMEDOUT|timed out", re.I, "timeout"),
    (r"ENOENT|No such file or directory", re.I, "not_found"),
    (r"(?:^|\n)\s*(?:Error|error|ERROR):|Traceback|at [\w.]+\s*\(|Unhandled|uncaught exception",
     re.M, "runtime_error"),
]
_COMPILED_FAILURE = [(re.compile(p, f), c) for p, f, c in _FAILURE_RULES]

def classify_failure(output: str) -> str | None:
    if not output:
        return None
    sample = output[:2000]
    for rx, cat in _COMPILED_FAILURE:
        if rx.search(sample):
            return cat
    return None

# ── classify_tool (classify.rs:71) ─────────────────────────────────────────────────
def classify_tool(name: str) -> str:
    return {"bash": "bash", "edit": "edit", "write": "write", "read": "read",
            "glob": "glob", "grep": "grep"}.get(name.lower(), "other")

# ── extract_file (classify.rs:152) ─────────────────────────────────────────────────
_FILE_RE = re.compile(r"/[\w./-]+\.\w+")
def extract_file(action: str) -> str | None:
    m = _FILE_RE.search(action or "")
    return m.group(0) if m else None


class Obs:
    __slots__ = ("tool", "tool_category", "result", "action", "error_snippet",
                 "failure_category")

    def __init__(self, tool_category: str, result: str, action: str,
                 error_snippet: str | None, failure_category: str | None, tool: str = ""):
        self.tool = tool
        self.tool_category = tool_category
        self.result = result            # "success" | "error"
        self.action = action
        self.error_snippet = error_snippet
        self.failure_category = failure_category


# ── stream-json NDJSON -> [Obs] ────────────────────────────────────────────────────
def parse_transcript(path: str) -> tuple[list[Obs], dict]:
    tool_uses: list[tuple[str, str, dict]] = []   # (id, name, input) in order
    results: dict[str, tuple[str, bool]] = {}      # id -> (content, is_error)
    meta = {}
    for line in Path(path).read_text().splitlines():
        line = line.strip()
        if not line:
            continue
        try:
            ev = json.loads(line)
        except json.JSONDecodeError:
            continue
        t = ev.get("type")
        if t == "result":
            meta["total_cost_usd"] = ev.get("total_cost_usd", 0)
            meta["num_turns"] = ev.get("num_turns", 0)
            meta["duration_ms"] = ev.get("duration_ms", 0)
            meta["usage"] = ev.get("usage", {})
            meta["is_error"] = ev.get("is_error", False)
        msg = ev.get("message") or {}
        if t == "assistant":
            for blk in msg.get("content", []):
                if blk.get("type") == "tool_use":
                    tool_uses.append((blk.get("id", ""), blk.get("name", ""),
                                      blk.get("input", {}) or {}))
        elif t == "user":
            for blk in msg.get("content", []):
                if blk.get("type") == "tool_result":
                    content = blk.get("content", "")
                    if isinstance(content, list):  # [{type:text,text:...}]
                        content = " ".join(c.get("text", "") for c in content
                                           if isinstance(c, dict))
                    results[blk.get("tool_use_id", "")] = (str(content),
                                                           bool(blk.get("is_error", False)))
    obs: list[Obs] = []
    for tid, name, inp in tool_uses:
        cat = classify_tool(name)
        if cat in ("edit", "write"):
            action = inp.get("file_path") or inp.get("notebook_path") or ""
        elif cat == "bash":
            action = inp.get("command") or ""
        else:
            action = ""
        content, is_err = results.get(tid, ("", False))
        snippet = content if is_err else None
        obs.append(Obs(cat, "error" if is_err else "success", action, snippet,
                       classify_failure(content) if is_err else None, name))
    return obs, meta


# ── detect_patterns (analysis.rs:225) — faithful port ──────────────────────────────
def detect_patterns(observations: list[Obs]) -> list[dict]:
    P = PINNED_THRESHOLDS
    scored = [o for o in observations]  # Rust filters result.is_some(); all ours have a result
    patterns: list[dict] = []

    # Pattern 1: repeated_same_error
    streak = 1
    streak_file = ""
    streak_cat = ""
    prev_hash = ""
    def flush(streak, streak_file, streak_cat, prev_hash):
        if streak >= P["repeated_error_min"]:
            patterns.append({
                "pattern_type": "repeated_same_error",
                "description": f"{streak_cat} repeated {streak}x on {streak_file}"
                               + (f" [hash:{prev_hash}]" if prev_hash else ""),
                "count": streak, "involved_files": [streak_file] if streak_file else [],
            })
    for i in range(1, len(scored)):
        prev, curr = scored[i - 1], scored[i]
        cur_snip = curr.error_snippet or ""
        prv_snip = prev.error_snippet or ""
        cur_hash = hash_string(normalize_error(cur_snip)) if cur_snip else ""
        prv_hash = hash_string(normalize_error(prv_snip)) if prv_snip else ""
        same = (curr.result == "error" and prev.result == "error"
                and curr.failure_category == prev.failure_category
                and curr.failure_category is not None
                and extract_file(curr.action) == extract_file(prev.action)
                and (cur_hash == prv_hash or not cur_hash or not prv_hash))
        if same:
            streak += 1
            streak_file = extract_file(curr.action) or ""
            streak_cat = curr.failure_category or ""
            prev_hash = cur_hash
        else:
            flush(streak, streak_file, streak_cat, prev_hash)
            streak = 1
            prev_hash = ""
    flush(streak, streak_file, streak_cat, prev_hash)

    # Pattern 2: fix_then_break
    ftb: dict[str, int] = {}
    for i, o in enumerate(scored):
        if o.tool_category in ("edit", "write") and o.result == "success" and o.action:
            file = extract_file(o.action) or o.action
            base = file.rsplit("/", 1)[-1]
            for nxt in scored[i + 1: i + 1 + P["ftb_lookahead"]]:
                if nxt.result == "error" and nxt.tool_category == "bash":
                    snip = nxt.error_snippet or ""
                    if file in snip or base in snip:
                        ftb[file] = ftb.get(file, 0) + 1
                        break
    ftb = {f: c for f, c in ftb.items() if c >= P["ftb_min_cycles"]}
    if ftb:
        patterns.append({"pattern_type": "fix_then_break",
                         "description": f"Edit→Break cycle on {', '.join(ftb)}",
                         "count": sum(ftb.values()), "involved_files": list(ftb)})

    # Pattern 3: long_debug_loop
    edit_only = [o for o in scored if o.tool_category in ("edit", "write")]
    prev_file = ""
    run = 0
    runs: dict[str, int] = {}
    for o in edit_only:
        file = extract_file(o.action) or ""
        if file and file == prev_file:
            run += 1
        else:
            if run >= P["debug_loop_min"] and prev_file:
                runs[prev_file] = max(runs.get(prev_file, 0), run)
            prev_file = file
            run = 1
    if run >= P["debug_loop_min"] and prev_file:
        runs[prev_file] = max(runs.get(prev_file, 0), run)
    for file, count in runs.items():
        patterns.append({"pattern_type": "long_debug_loop",
                         "description": f"Stuck on {file.rsplit('/', 1)[-1]} for {count} consecutive operations",
                         "count": count, "involved_files": [file]})

    # Pattern 4: thrashing
    stats: dict[str, list[int]] = {}
    for o in scored:
        file = extract_file(o.action) or ""
        if not file:
            continue
        s = stats.setdefault(file, [0, 0])
        if o.tool_category in ("edit", "write"):
            s[0] += 1
        if o.result == "error":
            s[1] += 1
    for file, (edits, errors) in stats.items():
        if edits >= P["thrash_min_edits"] and errors >= P["thrash_min_errors"]:
            patterns.append({"pattern_type": "thrashing",
                             "description": f"Edit↔Error thrashing on {file.rsplit('/', 1)[-1]} ({edits} edits, {errors} errors)",
                             "count": edits + errors, "involved_files": [file]})
    return patterns


def neutral_errors(observations: list[Obs]) -> list[Obs]:
    """R1 'genuine errors': bash non-zero on a non-test command, or build/runtime error.
    Excludes TDD 'Red' test_fail (which epic's failure_category would otherwise count)."""
    out = []
    for o in observations:
        if o.result != "error":
            continue
        cat = o.failure_category
        if cat == "test_fail":
            continue  # TDD red — not a robustness failure
        if o.tool_category == "bash" and cat in (None, "runtime_error", "build_fail",
                                                 "type_error", "syntax_error", "not_found",
                                                 "permission_denied", "timeout", "lint_fail"):
            out.append(o)
        elif cat in ("build_fail", "runtime_error", "syntax_error", "type_error"):
            out.append(o)
    return out


def replay(path: str) -> dict:
    obs, meta = parse_transcript(path)
    patterns = detect_patterns(obs)
    errs = [o for o in obs if o.result == "error"]
    cats = Counter(o.failure_category for o in errs)
    return {
        "meta": meta,
        "summary": {
            "n_tools": len(obs),
            "n_errors": len(errs),
            "n_neutral_errors": len(neutral_errors(obs)),
            "error_categories": dict(cats),
            "tool_categories": dict(Counter(o.tool_category for o in obs)),
        },
        "patterns": patterns,
    }


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("transcript", help="stream-json NDJSON file")
    args = ap.parse_args()
    res = replay(args.transcript)
    print(json.dumps(res, indent=2))


if __name__ == "__main__":
    main()
