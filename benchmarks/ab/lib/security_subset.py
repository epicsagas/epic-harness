#!/usr/bin/env python3
"""Mark the security-relevant subset of SWE-bench Verified, identically for both arms.

METHODOLOGY §2.3. Computed ONCE before the run from the gold patch (deterministic).
A instance is security_relevant iff:
  (a) any gold-patch file path matches the security regex, OR
  (b) Bandit (and/or Semgrep p/owasp-top-ten) flags >=1 finding on the patch's added lines.

Report the actual N. tools_used records which detectors ran (so a null is interpretable).

Usage:
  python security_subset.py --out security_subset.json            # path-regex + bandit (if present)
  python security_subset.py --out security_subset.json --semgrep  # also run semgrep p/owasp-top-ten
"""

from __future__ import annotations
import argparse, json, os, re, subprocess, tempfile
from datasets import load_dataset

SEC_PATH = re.compile(
    r"(auth|login|session|password|token|permission|middleware|views|forms|"
    r"query|sql|serializ|crypto|hash|secret)",
    re.IGNORECASE,
)
DIFF_GIT = re.compile(r"^diff --git a/(\S+) b/(\S+)$", re.MULTILINE)
HUNK_ADDED = re.compile(r"^\+(?!\+)(.*)$", re.MULTILINE)


def patch_paths(patch: str) -> list[str]:
    return [m.group(2) for m in DIFF_GIT.finditer(patch or "")]


def added_lines(patch: str) -> str:
    return "\n".join(m.group(1) for m in HUNK_ADDED.finditer(patch or ""))


def run_bandit(added: str, exe: str) -> tuple[bool, list[str]]:
    if not added.strip():
        return False, []
    with tempfile.NamedTemporaryFile("w", suffix=".py", delete=False) as f:
        f.write(added)
        path = f.name
    try:
        out = subprocess.run(
            [exe, "-f", "json", "-q", path], capture_output=True, text=True, timeout=30
        )
        if out.returncode not in (0, 1):  # bandit: 0 clean, 1 findings
            return False, [f"bandit_rc={out.returncode}"]
        try:
            res = json.loads(out.stdout or "{}")
        except json.JSONDecodeError:
            return False, ["bandit_unparsable"]
        findings = res.get("results", [])
        return len(findings) > 0, [
            f"{x.get('issue_cwe', {}).get('id', '?')}:{x.get('test_id', '?')}"
            for x in findings
        ]
    finally:
        os.unlink(path)


def run_semgrep(added: str, exe: str) -> tuple[bool, list[str]]:
    if not added.strip():
        return False, []
    with tempfile.NamedTemporaryFile("w", suffix=".py", delete=False) as f:
        f.write(added)
        path = f.name
    try:
        out = subprocess.run(
            [exe, "--config", "p/owasp-top-ten", "--json", "--quiet", path],
            capture_output=True,
            text=True,
            timeout=120,
        )
        try:
            res = json.loads(out.stdout or "{}")
        except json.JSONDecodeError:
            return False, ["semgrep_unparsable"]
        findings = res.get("results", [])
        return len(findings) > 0, [x.get("check_id", "?") for x in findings]
    finally:
        os.unlink(path)


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", default="security_subset.json")
    ap.add_argument("--dataset", default="princeton-nlp/SWE-bench_Verified")
    ap.add_argument(
        "--semgrep", action="store_true", help="also run semgrep p/owasp-top-ten"
    )
    args = ap.parse_args()

    bandit_exe = next((p for p in ("bandit",) if shutil_which(p)), None)
    semgrep_exe = (
        next((p for p in ("semgrep",) if shutil_which(p)), None)
        if args.semgrep
        else None
    )
    tools = (
        ["path-regex"]
        + (["bandit"] if bandit_exe else [])
        + (["semgrep"] if semgrep_exe else [])
    )

    ds = load_dataset(args.dataset, split="test")
    out = {}
    n_sec = 0
    for r in ds:
        paths = patch_paths(r["patch"])
        reasons = []
        sec = bool(paths and any(SEC_PATH.search(p) for p in paths))
        if sec:
            reasons.append("path-regex")
        added = added_lines(r["patch"])
        if bandit_exe:
            hit, _ = run_bandit(added, bandit_exe)
            if hit:
                sec = True
                reasons.append("bandit")
        if semgrep_exe:
            hit, _ = run_semgrep(added, semgrep_exe)
            if hit:
                sec = True
                reasons.append("semgrep")
        if sec:
            n_sec += 1
        out[r["instance_id"]] = {"security_relevant": sec, "reason": reasons}

    with open(args.out, "w") as f:
        json.dump(
            {
                "tools_used": tools,
                "n_total": len(out),
                "n_security_relevant": n_sec,
                "instances": out,
            },
            f,
            indent=2,
        )
    print(
        f"wrote {args.out}: {n_sec}/{len(out)} security-relevant  (tools: {', '.join(tools)})"
    )
    if n_sec < 40:
        print(
            "  ⚠️ N<40: binary clean-rate metric (S1) is descriptive-only / underpowered"
        )
    print(
        f"  path-regex hits only: {sum(1 for v in out.values() if 'path-regex' in v['reason'])}"
    )


def shutil_which(name: str) -> str | None:
    from shutil import which

    return which(name)


if __name__ == "__main__":
    main()
