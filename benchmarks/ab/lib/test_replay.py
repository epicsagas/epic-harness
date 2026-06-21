#!/usr/bin/env python3
"""Self-tests for replay.py — verifies the Python port matches the Rust semantics.
Run: python test_replay.py"""
import sys, os
sys.path.insert(0, os.path.dirname(__file__))
import tempfile, json
from replay import (hash_string, normalize_error, classify_failure, classify_tool,
                    extract_file, detect_patterns, parse_transcript, replay, Obs, neutral_errors)

failures = []

def check(name, got, want):
    ok = got == want
    if not ok:
        failures.append(f"{name}: got {got!r} want {want!r}")
    print(f"[{'OK' if ok else 'FAIL'}] {name}")

# ── hash_string: hand-computed reference (hash*31+byte, u32 wrap) ──
# "abc" = 97,98,99 -> 97 -> 3105 -> 96354 -> 0x00017862
check("hash_string(abc)", hash_string("abc"), "00017862")
check("hash_string stable", hash_string("x"), hash_string("x"))
# u32 wrap: a long string must stay 8 hex (wrapping)
check("hash_string wraps to 8 hex", len(hash_string("a" * 10000)), 8)

# ── normalize_error ──
ne = normalize_error("2024-01-02T03:04:05.123Z Error at /home/user/src/main.py:42:7 boom")
check("normalize strips ts/path/linecol", ne, "Error at /PATH/main.py:L:C boom")

# ── classify_failure: each rule ──
check("classify TypeError", classify_failure("foo TypeError: bad"), "type_error")
check("classify SyntaxError", classify_failure("SyntaxError: invalid"), "syntax_error")
check("classify AssertionError", classify_failure("AssertionError: 1!=2"), "test_fail")
check("classify test FAILED", classify_failure("FAILED (failures=1)"), "test_fail")
check("classify build fail", classify_failure("cargo build failed"), "build_fail")
check("classify permission", classify_failure("EACCES permission denied"), "permission_denied")
check("classify timeout", classify_failure("ETIMEDOUT"), "timeout")
check("classify ENOENT", classify_failure("ENOENT: No such file or directory"), "not_found")
check("classify traceback", classify_failure("Traceback (most recent call last)"), "runtime_error")
check("classify none", classify_failure("all good"), None)
check("classify empty", classify_failure(""), None)

# ── classify_tool / extract_file ──
check("tool bash", classify_tool("Bash"), "bash")
check("tool edit", classify_tool("Edit"), "edit")
check("tool other", classify_tool("WebSearch"), "other")
check("extract_file path", extract_file("edited /a/b/c.py done"), "/a/b/c.py")
check("extract_file none", extract_file("ls -la"), None)

# ── detect_patterns: each pattern trigger + non-trigger ──
def mk(cat, result, action, snippet=None, fcat=None):
    return Obs(cat, result, action, snippet, fcat)

# repeated_same_error: 3x same -> fires; 2x -> no
three = [mk("bash", "error", "/f.py", "Error: boom", "runtime_error") for _ in range(3)]
pt = [p["pattern_type"] for p in detect_patterns(three)]
check("repeated fires at 3", "repeated_same_error" in pt, True)
two = three[:2]
pt2 = [p["pattern_type"] for p in detect_patterns(two)]
check("repeated no fire at 2", "repeated_same_error" in pt2, False)
# different categories break the streak
mixed = [mk("bash", "error", "/f.py", "TypeError", "type_error"),
         mk("bash", "error", "/f.py", "TypeError", "type_error"),
         mk("bash", "error", "/f.py", "boom", "runtime_error")]
check("repeated needs same category",
      "repeated_same_error" in [p["pattern_type"] for p in detect_patterns(mixed)], False)

# fix_then_break: edit success then bash error mentioning file, x2
ftb = []
for _ in range(2):
    ftb.append(mk("edit", "success", "/app/x.py"))
    ftb.append(mk("bash", "error", "", "Error in /app/x.py", "runtime_error"))
check("ftb fires", "fix_then_break" in [p["pattern_type"] for p in detect_patterns(ftb)], True)
# only one cycle (min_cycles=2) -> no
ftb1 = ftb[:2]
check("ftb no fire at 1 cycle",
      "fix_then_break" in [p["pattern_type"] for p in detect_patterns(ftb1)], False)

# long_debug_loop: 5 consecutive edits same file
loop = [mk("edit", "success", "/m.py") for _ in range(5)]
check("debug_loop fires at 5",
      "long_debug_loop" in [p["pattern_type"] for p in detect_patterns(loop)], True)
loop4 = loop[:4]
check("debug_loop no fire at 4",
      "long_debug_loop" in [p["pattern_type"] for p in detect_patterns(loop4)], False)

# thrashing: 3 edits + 3 errors same file
thr = [mk("edit", "success", "/t.py") for _ in range(3)] + \
      [mk("bash", "error", "/t.py", "boom", "runtime_error") for _ in range(3)]
check("thrashing fires",
      "thrashing" in [p["pattern_type"] for p in detect_patterns(thr)], True)

# ── parse_transcript: minimal stream-json pairing ──
ndjson = "\n".join([
    json.dumps({"type": "assistant", "message": {"content": [
        {"type": "tool_use", "id": "t1", "name": "Edit",
         "input": {"file_path": "/p/x.py"}},
        {"type": "tool_use", "id": "t2", "name": "Bash",
         "input": {"command": "pytest /p/x.py"}}]}}),
    json.dumps({"type": "user", "message": {"content": [
        {"type": "tool_result", "tool_use_id": "t1", "content": "ok", "is_error": False},
        {"type": "tool_result", "tool_use_id": "t2",
         "content": "AssertionError: nope", "is_error": True}]}}),
    json.dumps({"type": "result", "total_cost_usd": 0.1, "num_turns": 2,
                "duration_ms": 1000, "usage": {"input_tokens": 50}, "is_error": False}),
])
with tempfile.NamedTemporaryFile("w", suffix=".ndjson", delete=False) as f:
    f.write(ndjson)
    tpath = f.name
obs, meta = parse_transcript(tpath)
check("parsed 2 tools", len(obs), 2)
check("parsed edit action", obs[0].action, "/p/x.py")
check("parsed bash action", obs[1].action, "pytest /p/x.py")
check("parsed error result", obs[1].result, "error")
check("parsed failure cat (test_fail)", obs[1].failure_category, "test_fail")
check("meta cost", meta.get("total_cost_usd"), 0.1)
# neutral_errors excludes test_fail
check("neutral_errors excludes test_fail", len(neutral_errors(obs)), 0)
os.unlink(tpath)

print()
print("ALL PASS" if not failures else f"{len(failures)} FAILURES:\n" + "\n".join(failures))
sys.exit(1 if failures else 0)
