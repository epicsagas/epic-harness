#!/usr/bin/env bash
# Collect harness data for /reflect skill — outputs JSON to stdout.
# Usage: bash scripts/reflect-context.sh [days=30]

set -euo pipefail

HARNESS_DIR=$(epic-harness path 2>/dev/null || echo "")
if [[ -z "$HARNESS_DIR" ]]; then
  echo '{"error":"epic-harness not found or path command failed"}' >&2
  exit 1
fi

DAYS="${1:-30}"
[[ "$DAYS" =~ ^[0-9]+$ ]] || { echo '{"error":"invalid days argument — must be a positive integer"}' >&2; exit 1; }
CUTOFF=$(date -v "-${DAYS}d" +%Y-%m-%dT%H:%M:%SZ 2>/dev/null || date -d "-${DAYS} days" +%Y-%m-%dT%H:%M:%SZ 2>/dev/null || echo "")

python3 - "$HARNESS_DIR" "$DAYS" "$CUTOFF" << 'PYEOF'
import json, sys, os, glob, collections, statistics
from datetime import datetime, timezone

harness_dir, days_str, cutoff = sys.argv[1], sys.argv[2], sys.argv[3]
days = int(days_str)

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

# ── 1. obs stats ──────────────────────────────────────────────────────────────
all_obs_files = sorted(glob.glob(f"{harness_dir}/obs/*.jsonl"))
if cutoff:
    cutoff_date = cutoff[:10].replace("-", "")
    obs_files = [f for f in all_obs_files if os.path.basename(f).replace("session_", "")[:8] >= cutoff_date]
else:
    obs_files = all_obs_files

tools = collections.Counter()
failure_cats = collections.Counter()
file_exts = collections.Counter()
scores = []
dim_sums = collections.defaultdict(float)
dim_counts = collections.defaultdict(int)
tool_success = collections.defaultdict(lambda: [0, 0])
total_obs = 0

for fp in obs_files:
    try:
        with open(fp) as f:
            for line in f:
                line = line.strip()
                if not line:
                    continue
                try:
                    d = json.loads(line)
                except Exception:
                    continue
                total_obs += 1
                tool = d.get("tool", "unknown")
                tools[tool] += 1
                fc = d.get("failure_category")
                if fc:
                    failure_cats[fc] += 1
                ext = d.get("file_ext", "")
                if ext:
                    file_exts[ext] += 1
                score = d.get("score")
                if score is not None:
                    scores.append(float(score))
                for k, v in d.get("dimensions", {}).items():
                    dim_sums[k] += float(v)
                    dim_counts[k] += 1
                tool_success[tool][1] += 1
                result = d.get("result")
                if result == "success" or (result is None and d.get("score", 0) >= 0.7):
                    tool_success[tool][0] += 1
    except Exception:
        pass

obs_stats = {
    "total": total_obs,
    "avg_score": round(sum(scores) / len(scores), 3) if scores else 0,
    "score_distribution": {
        "high_ge09": sum(1 for s in scores if s >= 0.9),
        "mid_06_09": sum(1 for s in scores if 0.6 <= s < 0.9),
        "low_lt06": sum(1 for s in scores if s < 0.6),
    },
    "top_tools": dict(tools.most_common(10)),
    "failure_categories": dict(failure_cats.most_common()),
    "top_file_exts": dict(file_exts.most_common(8)),
    "dimension_averages": {k: round(dim_sums[k] / dim_counts[k], 3) for k in dim_counts},
    "weak_tools": [t for t, (s, n) in tool_success.items() if n >= 5 and s / n < 0.6],
    "strong_tools": [t for t, (s, n) in tool_success.items() if n >= 5 and s / n >= 0.9],
    "tool_success_rate": round(sum(s for s, n in tool_success.values()) / max(sum(n for s, n in tool_success.values()), 1), 3),
}

# ── 2. evolution stats ────────────────────────────────────────────────────────
evo_file = f"{harness_dir}/evolution.jsonl"
evo_sessions = []
pattern_freq = collections.Counter()
trend_hist = []
skills_generated = 0

if os.path.exists(evo_file):
    with open(evo_file) as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            try:
                d = json.loads(line)
                evo_sessions.append(d)
                for p in d.get("patterns", []):
                    pattern_freq[p.get("type", "")] += 1
                t = d.get("trend", "")
                if t:
                    trend_hist.append(t)
                skills_generated += d.get("skills_generated", 0)
            except Exception:
                pass

recent_evo = evo_sessions[-10:] if evo_sessions else []
evo_stats = {
    "total_sessions": len(evo_sessions),
    "patterns_detected": sum(1 for r in evo_sessions if r.get("patterns", {}).get("detected")),
    "skills_generated": skills_generated,
    "pattern_frequency": dict(pattern_freq.most_common()),
    "trend_last10": trend_hist[-10:],
    "recent_weak_tools": list({t for s in recent_evo for t in s.get("weak_tools", [])}),
    "recent_seeded_skills": list({sk for s in recent_evo for sk in s.get("seeded_skills", [])}),
    "stagnation_count": sum(1 for s in evo_sessions if s.get("stagnation_triggered", False)),
}

# ── 3. metrics.json ───────────────────────────────────────────────────────────
metrics = safe_json_load(f"{harness_dir}/metrics.json") or {}

score_history = metrics.get("score_history", [])
score_trend = []
if len(score_history) >= 3:
    recent_avgs = [s.get("avg_score", 0) for s in score_history[-10:]]
    for i in range(1, len(recent_avgs)):
        score_trend.append(recent_avgs[i] - recent_avgs[i - 1])

# Score trend: first 5 vs last 5
score_comparison = None
if len(score_history) >= 10:
    first5_avg = statistics.mean(s.get("avg_score", 0) for s in score_history[:5])
    last5_avg = statistics.mean(s.get("avg_score", 0) for s in score_history[-5:])
    score_comparison = {
        "first_5_avg": round(first5_avg, 3),
        "last_5_avg": round(last5_avg, 3),
        "direction": "improving" if last5_avg > first5_avg else ("declining" if last5_avg < first5_avg else "stable"),
        "delta": round(last5_avg - first5_avg, 3),
    }

skill_attribution = {
    k: {
        "sessions_active": v.get("sessions_active", 0),
        "avg_score_with": v.get("avg_score_with", 0),
        "avg_score_without": v.get("avg_score_without", 0),
        "delta": round(v.get("avg_score_with", 0) - v.get("avg_score_without", 0), 3)
    } for k, v in metrics.get("skill_attribution", {}).items()
}

metrics_summary = {
    "total_sessions": metrics.get("total_sessions", 0),
    "avg_success_rate": metrics.get("avg_success_rate", 0),
    "total_evolved_skills": metrics.get("total_evolved_skills", 0),
    "last_session": metrics.get("last_session", ""),
    "trend": metrics.get("trend", "unknown"),
    "best_score": metrics.get("best_score", 0),
    "stagnation_count": metrics.get("stagnation_count", 0),
    "score_trend_delta": round(sum(score_trend) / len(score_trend), 4) if score_trend else 0,
    "score_comparison": score_comparison,
    "latest_avg_score": score_history[-1].get("avg_score", 0) if score_history else 0,
    "latest_dimensions": score_history[-1].get("dimension_averages", {}) if score_history else {},
    "skill_attribution": skill_attribution,
}

# ── 4. session snapshots ──────────────────────────────────────────────────────
snap_files = sorted(glob.glob(f"{harness_dir}/sessions/snapshot_*.json"))
snapshots = []
for sp in snap_files[-5:]:
    try:
        with open(sp) as f:
            d = json.load(f)
        snapshots.append({
            "timestamp": d.get("timestamp", ""),
            "type": d.get("type", ""),
            "summary": d.get("summary", "")[:400],
        })
    except Exception:
        pass

# ── 5. evolved skills ─────────────────────────────────────────────────────────
evolved_dir = f"{harness_dir}/evolved"
evolved_skills = []
evolved_by_type = {"preset": 0, "evolved": 0, "auto-evolved": 0}
if os.path.exists(evolved_dir):
    for item in os.listdir(evolved_dir):
        skill_path = os.path.join(evolved_dir, item, "SKILL.md")
        if os.path.isfile(skill_path):
            try:
                with open(skill_path) as f:
                    content = f.read()
                if "Evolved:" in content or "evolved_from:" in content:
                    stype = "evolved"
                elif "Auto-evolved" in content or "auto-evolved" in content.lower():
                    stype = "auto-evolved"
                else:
                    stype = "preset"
                evolved_by_type[stype] += 1
                evolved_skills.append({"name": item, "type": stype})
            except Exception:
                evolved_skills.append({"name": item, "type": "preset"})
                evolved_by_type["preset"] += 1

# ── compile ───────────────────────────────────────────────────────────────────
print(json.dumps({
    "generated_at": datetime.now(timezone.utc).isoformat(),
    "analysis_window_days": days,
    "obs_stats": obs_stats,
    "evolution_stats": evo_stats,
    "metrics_summary": metrics_summary,
    "session_snapshots": snapshots,
    "evolved_skills": evolved_skills,
    "evolved_skills_summary": {
        "total": len(evolved_skills),
        "by_type": evolved_by_type,
    },
}, indent=2, ensure_ascii=False))
PYEOF
