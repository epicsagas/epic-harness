#!/usr/bin/env python3
"""Grade predictions against the OSS task set.

Usage: grade.py <tasks.jsonl> <repos_root> <predictions.jsonl> [<predictions.jsonl> ...]
For each prediction: worktree at the base commit, apply the model patch, apply the
hidden test patch (3-way fallback), run the task's test command. Resolved when every
FAIL_TO_PASS and PASS_TO_PASS test passes. Prints one JSON line per prediction.
"""
import json, os, re, sys, tempfile
sys.path.insert(0, os.path.dirname(__file__))
from validate import sh, statuses  # noqa: E402

def apply_test_patch(patch, wt):
    """Apply the hidden test patch file by file. A file whose hunks the model
    already wrote verbatim reverse-applies cleanly and counts as applied. The
    reverse check runs before --3way, which can leave conflict markers behind."""
    for i, part in enumerate(re.split(r"(?m)^(?=diff --git )", patch)):
        if not part.startswith("diff --git"):
            continue
        f = os.path.join(wt, f".t{i}.patch")
        open(f, "w").write(part)
        if sh(f"git apply {f}", wt)[0] and sh(f"git apply -R --check {f}", wt)[0] \
                and sh(f"git apply --3way {f}", wt)[0]:
            return False
    return True

def main():
    tasks = {t["instance_id"]: t for t in map(json.loads, open(sys.argv[1]))}
    root = sys.argv[2]
    for pf in sys.argv[3:]:
        for p in map(json.loads, open(pf)):
            t = tasks[p["instance_id"]]
            name = t["repo"].split("/")[-1]
            repo = os.path.join(root, name)
            env = {"CARGO_TARGET_DIR": os.path.expanduser(f"~/.cache/oss-target/{name}"), "CI": "1"}
            res = {"instance_id": t["instance_id"], "arm": p["model_name_or_path"], "resolved": False}
            if not p["model_patch"].strip():
                res["status"] = "empty_patch"; print(json.dumps(res)); continue
            wt = tempfile.mkdtemp(prefix="ossg-"); os.rmdir(wt)
            sh(f"git worktree add --detach -q {wt} {t['base_commit']}", repo)
            try:
                # the runner stores `$(git diff)`, and command substitution strips the
                # final newline that git apply requires
                mp = p["model_patch"] if p["model_patch"].endswith("\n") else p["model_patch"] + "\n"
                open(os.path.join(wt, ".m.patch"), "w").write(mp)
                if sh("git apply .m.patch", wt)[0]:
                    res["status"] = "model_patch_failed"; print(json.dumps(res)); continue
                if not apply_test_patch(t["test_patch"], wt):
                    res["status"] = "test_patch_conflict"; print(json.dumps(res)); continue
                _, out = sh(t["test_cmd"], wt, env)
                st = statuses(t["lang"], out, t["FAIL_TO_PASS"] + t["PASS_TO_PASS"])
                res["tests"] = st
                res["resolved"] = all(v == "pass" for v in st.values())
                res["status"] = "graded"
                print(json.dumps(res), flush=True)
            finally:
                sh(f"git worktree remove --force {wt}", repo)

if __name__ == "__main__":
    main()
