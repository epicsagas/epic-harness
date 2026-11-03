#!/usr/bin/env python3
"""List recent fix commits that add tests, split each diff into src and test hunks.

A hunk is a test hunk when its file is a test file (tests/, *_test.*, test_*.py,
*.test.*) or, for Rust, when it starts at or after the file's `#[cfg(test)]` line
in the fixed version. Prints one JSON object per candidate.
"""
import json, re, subprocess, sys

def git(repo, *a):
    return subprocess.run(["git", "-C", repo, *a], capture_output=True, text=True).stdout

TEST_FILE = re.compile(r"(^|/)tests?/|_test\.\w+$|(^|/)test_[^/]*\.py$|\.test\.\w+$|\.spec\.\w+$")
NEW_TEST = re.compile(r"^\+\s*(#\[(tokio::)?test\]|def test_|(it|test)\()")

def split(repo, h):
    diff = git(repo, "show", h, "--format=", "-U3", "--no-color")
    files = re.split(r"(?m)^(?=diff --git )", diff)
    src, test = [], []
    for f in files:
        if not f.startswith("diff --git"):
            continue
        path = re.match(r"diff --git a/(\S+) b/(\S+)", f).group(2)
        head, *hunks = re.split(r"(?m)^(?=@@ )", f)
        if TEST_FILE.search(path):
            test.append(f); continue
        cfg = None
        if path.endswith(".rs"):
            new = git(repo, "show", f"{h}:{path}").splitlines()
            cfg = next((i + 1 for i, l in enumerate(new) if l.strip() == "#[cfg(test)]"), None)
        s_h, t_h = [], []
        for hk in hunks:
            start = int(re.match(r"@@ -\d+(?:,\d+)? \+(\d+)", hk).group(1))
            (t_h if cfg and start >= cfg - 3 else s_h).append(hk)
        if s_h: src.append(head + "".join(s_h))
        if t_h: test.append(head + "".join(t_h))
    return src, test

def main():
    since = sys.argv[1]
    for repo in sys.argv[2:]:
        for h in git(repo, "log", f"--since={since}", "--format=%H", "-E", "--grep=^fix").split():
            src, test = split(repo, h)
            if not src or not test or not any(NEW_TEST.search(l) for t in test for l in t.splitlines()):
                continue
            tests = sorted({m.group(1) for t in test for m in re.finditer(r"^\+\s*(?:pub )?(?:async )?fn (\w+)|^\+\s*def (test_\w+)", t, re.M) if m.group(1)} |
                           {m.group(1) for t in test for m in re.finditer(r"^\+\s*def (test_\w+)", t, re.M)})
            print(json.dumps({"repo": repo, "commit": h, "parent": h + "^",
                "subject": git(repo, "log", "-1", "--format=%s", h).strip(),
                "date": git(repo, "log", "-1", "--format=%cs", h).strip(),
                "src_bytes": sum(map(len, src)), "tests": tests,
                "src_files": sorted({re.match(r"diff --git a/(\S+)", s).group(1) for s in src}),
                "src_patch": "".join(src), "test_patch": "".join(test)}))

if __name__ == "__main__":
    main()
