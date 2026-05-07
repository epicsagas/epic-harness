#!/usr/bin/env bash
# evaluate.sh — Collect evaluation context data for /evaluate skill
set -euo pipefail

HARNESS_DIR="$(epic-harness path 2>/dev/null || echo "$HOME/.harness/projects/$(basename "$(pwd)")-unknown")"

if [ ! -d "$HARNESS_DIR" ]; then
  echo '{"error": "harness directory not found", "path": "'"$HARNESS_DIR"'"}' >&2
  exit 1
fi

python3 << 'PYEOF'
import json, os, sys, glob, statistics
from datetime import datetime

harness_dir = os.environ.get("HARNESS_DIR", "")
if not harness_dir:
    print(json.dumps({"error": "HARNESS_DIR not set"}))
    sys.exit(1)

def safe_json_load(path):
    try:
        with open(path) as f:
            return json.load(f)
    except Exception:
        return None

def read_jsonl_files(pattern):
    records = []
    for f in sorted(glob.glob(pattern)):
        try:
            with open(f) as fh:
                for line in fh:
                    line = line.strip()
                    if line:
                        records.append(json.loads(line))
        except Exception:
            pass
    return records

# ── 1. Observations ──
obs_records = read_jsonl_files(os.path.join(harness_dir, "obs", "*.jsonl"))

tool_success_vals = []
output_quality_vals = []
execution_cost_vals = []
by_tool = {}
by_ext = {}

for r in obs_records:
    dims = r.get("dimensions", {})
    ts = dims.get("tool_success", r.get("tool_success", None))
    oq = dims.get("output_quality", r.get("output_quality", None))
    ec = dims.get("execution_cost", r.get("execution_cost", None))
    tool = r.get("tool", "unknown")
    ext = r.get("file_ext", "unknown")

    if ts is not None:
        tool_success_vals.append(float(ts))
    if oq is not None:
        output_quality_vals.append(float(oq))
    if ec is not None:
        execution_cost_vals.append(float(ec))

    by_tool.setdefault(tool, {"count": 0, "success": 0})
    by_tool[tool]["count"] += 1
    if ts == 1 or ts == 1.0:
        by_tool[tool]["success"] += 1

    by_ext.setdefault(ext, {"count": 0, "scores": []})
    by_ext[ext]["count"] += 1
    if oq is not None:
        by_ext[ext]["scores"].append(float(oq))

obs_stats = {
    "total": len(obs_records),
    "tool_success_avg": round(statistics.mean(tool_success_vals), 3) if tool_success_vals else 0,
    "output_quality_avg": round(statistics.mean(output_quality_vals), 3) if output_quality_vals else 0,
    "execution_cost_avg": round(statistics.mean(execution_cost_vals), 3) if execution_cost_vals else 0,
    "by_tool": {k: {"count": v["count"], "success_rate": round(v["success"]/v["count"], 3) if v["count"] else 0} for k, v in sorted(by_tool.items(), key=lambda x: -x[1]["count"])[:10]},
    "by_ext": {k: {"count": v["count"], "avg_quality": round(statistics.mean(v["scores"]), 3) if v["scores"] else 0} for k, v in sorted(by_ext.items(), key=lambda x: -x[1]["count"])[:10]},
}

# ── 2. Evolution ──
evo_records = read_jsonl_files(os.path.join(harness_dir, "evolution.jsonl"))
patterns_detected = sum(1 for r in evo_records if r.get("patterns", {}).get("detected"))
skills_generated = sum(r.get("skills_generated", 0) for r in evo_records)
last_5_evo = evo_records[-5:] if evo_records else []

evo_stats = {
    "total_entries": len(evo_records),
    "patterns_detected": patterns_detected,
    "skills_generated": skills_generated,
    "last_5": [{"ts": r.get("timestamp", "?"), "patterns": r.get("patterns", {}).get("detected", []), "skills": r.get("skills_generated", 0)} for r in last_5_evo],
}

# ── 3. Sessions ──
metrics = safe_json_load(os.path.join(harness_dir, "metrics.json"))
session_stats = {}
if metrics:
    sh = metrics.get("score_history", [])
    session_stats = {
        "total_sessions": metrics.get("total_sessions", 0),
        "trend": metrics.get("trend", "unknown"),
        "best_score": metrics.get("best_score", 0),
        "best_session": metrics.get("best_session", ""),
        "stagnation_count": metrics.get("stagnation_count", 0),
        "last_10_scores": sh[-10:] if sh else [],
        "skill_attribution": {
            k: {
                "sessions_active": v.get("sessions_active", 0),
                "avg_score_with": v.get("avg_score_with", 0),
                "avg_score_without": v.get("avg_score_without", 0),
                "delta": round(v.get("avg_score_with", 0) - v.get("avg_score_without", 0), 3)
            } for k, v in metrics.get("skill_attribution", {}).items()
        },
    }

    # Score trend: first 5 vs last 5
    if len(sh) >= 10:
        first5_avg = statistics.mean(s.get("avg_score", 0) for s in sh[:5])
        last5_avg = statistics.mean(s.get("avg_score", 0) for s in sh[-5:])
        session_stats["score_trend"] = {
            "first_5_avg": round(first5_avg, 3),
            "last_5_avg": round(last5_avg, 3),
            "direction": "improving" if last5_avg > first5_avg else ("declining" if last5_avg < first5_avg else "stable"),
            "delta": round(last5_avg - first5_avg, 3),
        }
    else:
        session_stats["score_trend"] = None

# ── 4. Evolved Skills ──
evolved_dir = os.path.join(harness_dir, "evolved")
evolved_skills = []
if os.path.isdir(evolved_dir):
    for skill_dir in sorted(os.listdir(evolved_dir)):
        skill_path = os.path.join(evolved_dir, skill_dir, "SKILL.md")
        if os.path.isfile(skill_path):
            try:
                with open(skill_path) as f:
                    content = f.read()
                # Determine type
                if "Evolved:" in content or "evolved_from:" in content:
                    skill_type = "evolved"
                elif "Auto-evolved" in content or "auto-evolved" in content.lower():
                    skill_type = "auto-evolved"
                else:
                    skill_type = "preset"

                # Extract trigger info
                trigger = ""
                for line in content.split("\n"):
                    if line.startswith("trigger_ext:"):
                        trigger = line.split(":", 1)[1].strip()
                    elif line.startswith("trigger_tools:"):
                        trigger = line.split(":", 1)[1].strip()

                evolved_skills.append({
                    "name": skill_dir,
                    "type": skill_type,
                    "trigger": trigger,
                })
            except Exception:
                pass

# ── 5. Output ──
output = {
    "timestamp": datetime.now().isoformat(),
    "harness_dir": harness_dir,
    "observations": obs_stats,
    "evolution": evo_stats,
    "sessions": session_stats,
    "evolved_skills": evolved_skills,
    "evolved_skills_summary": {
        "total": len(evolved_skills),
        "by_type": {
            "preset": sum(1 for s in evolved_skills if s["type"] == "preset"),
            "evolved": sum(1 for s in evolved_skills if s["type"] == "evolved"),
            "auto-evolved": sum(1 for s in evolved_skills if s["type"] == "auto-evolved"),
        }
    },
}

print(json.dumps(output, indent=2, ensure_ascii=False))
PYEOF
