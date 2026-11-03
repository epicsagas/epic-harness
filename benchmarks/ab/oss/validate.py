#!/usr/bin/env python3
"""Keep candidates whose hidden tests fail at base+tests and pass at the fix.

Usage: validate.py <candidates.jsonl> <repos_root> <out.jsonl>
Base failure must be a test failure, not a compile error: the hidden tests must
compile against the unfixed code, so the task never requires guessing an API.
"""
import json, os, re, subprocess, sys, tempfile

SKIP_SUBJECT = re.compile(r"^fix\(tests\)|^fix: frontier-model review|^fix\(secrets,mcp,tokens\)")
MAX_SRC = 16000

def sh(cmd, cwd, env=None, timeout=1800):
    p = subprocess.run(cmd, cwd=cwd, shell=True, capture_output=True, text=True,
                       env={**os.environ, **(env or {})}, timeout=timeout)
    return p.returncode, p.stdout + p.stderr

# llm-kernel gates modules behind features; enable only the one the fix touches
# (`--features full` pulls fastembed/ort/mlx and several GB of build output).
LLM_KERNEL_FEATURES = [("src/llm/", "client-async"), ("src/dlp", "dlp"), ("src/secrets", "secrets")]

def cargo_args(repo, src_files):
    if repo != "llm-kernel":
        return ""
    feats = sorted({f for prefix, f in LLM_KERNEL_FEATURES for p in src_files if p.startswith(prefix)})
    return "--features " + ",".join(feats) if feats else ""

def test_cmd(lang, tests, repo="", src_files=()):
    if lang == "py":
        return "python3 -m pytest -q -p no:cacheprovider -k " + repr(" or ".join(tests)) + " -rA"
    extra = cargo_args(repo, src_files)
    return f"cargo test --no-fail-fast {extra} -- --test-threads=2 ".replace("  ", " ") + " ".join(tests)

def statuses(lang, out, tests):
    st = {}
    for t in tests:
        if lang == "py":
            m = re.findall(rf"^(PASSED|FAILED|ERROR) \S*::{t}\b", out, re.M)
        else:
            m = re.findall(rf"^test (?:\S+::)?{t} \.\.\. (ok|FAILED|ignored)", out, re.M)
        st[t] = "pass" if m and all(x in ("ok", "PASSED") for x in m) else ("fail" if m else "missing")
    return st

def main():
    cands, root, out_path = sys.argv[1], sys.argv[2], sys.argv[3]
    out = open(out_path, "w")
    for c in map(json.loads, open(cands)):
        if SKIP_SUBJECT.search(c["subject"]) or c["src_bytes"] > MAX_SRC or not c["tests"]:
            continue
        repo = os.path.join(root, c["repo"])
        lang = "py" if c["repo"] == "AgentGlance" else "rust"
        env = {"CARGO_TARGET_DIR": os.path.expanduser(f"~/.cache/oss-target/{c['repo']}"), "CI": "1"}
        wt = tempfile.mkdtemp(prefix="ossv-")
        os.rmdir(wt)
        sh(f"git worktree add --detach -q {wt} {c['commit']}^", repo)
        try:
            with open(os.path.join(wt, ".t.patch"), "w") as f: f.write(c["test_patch"])
            with open(os.path.join(wt, ".s.patch"), "w") as f: f.write(c["src_patch"])
            rc, o = sh("git apply .t.patch", wt)
            if rc: print("SKIP test-apply", c["commit"][:8], file=sys.stderr); continue
            cmd = test_cmd(lang, c["tests"], c["repo"], c["src_files"])
            _, base_out = sh(cmd, wt, env)
            compile_err = lang == "rust" and "error[E" in base_out and "test result" not in base_out
            base = statuses(lang, base_out, c["tests"])
            sh("git apply .s.patch", wt)
            _, fix_out = sh(cmd, wt, env)
            fix = statuses(lang, fix_out, c["tests"])
            f2p = [t for t in c["tests"] if base[t] == "fail" and fix[t] == "pass"]
            p2p = [t for t in c["tests"] if base[t] == "pass" and fix[t] == "pass"]
            ok = bool(f2p) and not compile_err and all(fix[t] == "pass" for t in f2p + p2p)
            print(("KEEP " if ok else "DROP ") + c["commit"][:8], c["repo"], "compile_err" if compile_err else "", "base", base, "fix", fix, file=sys.stderr)
            if ok:
                c.update(lang=lang, test_cmd=cmd, FAIL_TO_PASS=f2p, PASS_TO_PASS=p2p)
                out.write(json.dumps(c) + "\n"); out.flush()
        finally:
            sh(f"git worktree remove --force {wt}", repo)

if __name__ == "__main__":
    main()
