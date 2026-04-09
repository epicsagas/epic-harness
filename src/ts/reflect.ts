#!/usr/bin/env node
/**
 * Ring 3: reflect — SessionEnd
 *
 * Evolution engine fusing A-Evolve's benchmark patterns:
 * 1. Collect observations → SessionAnalysis (like BatchAnalysis)
 * 2. Detect failure patterns (like FailurePatternDetector)
 * 3. Stagnation gating with rollback (like AdaptiveEvolveEngine)
 * 4. Smart skill seeding based on patterns + weak areas
 * 5. Score history, trend tracking, metrics
 */
import { readFileSync, writeFileSync, existsSync } from "node:fs";
import { join, basename as pathBasename } from "node:path";
import {
  runHook, hint, harnessExists, ensureDir,
  copyDirSync, rmDirSync, defaultMetrics,
  OBS_DIR, EVOLVED_DIR, EVOLVED_BACKUP_DIR, EVOLUTION_FILE, METRICS_FILE,
  GLOBAL_HARNESS_DIR, GLOBAL_PATTERNS_FILE, CROSS_PROJECT_OPT_IN_FILE,
  MAX_EVOLVED_SKILLS, STAGNATION_LIMIT, IMPROVEMENT_THRESHOLD,
  REPEATED_ERROR_MIN, FTB_LOOKAHEAD, FTB_MIN_CYCLES,
  DEBUG_LOOP_MIN, THRASH_MIN_EDITS, THRASH_MIN_ERRORS,
  WEAK_TOOL_RATE, WEAK_TOOL_MIN_OBS, WEAK_EXT_RATE, WEAK_EXT_MIN_OBS, HIGH_FREQ_ERROR_MIN,
  CWD, hashString, normalizeError,
  listDirs, listFiles, readJsonSafe, readJsonlSafe, appendJsonl, now,
  type ObsRecord, type EvolutionRecord, type Metrics,
  type SessionAnalysis, type SessionScoreEntry, type ToolStats,
  type DetectedPattern, type ScoreDimensions, type ToolCategory, type SkillAttribution,
} from "./common.js";
import { existsSync as fsExistsSync } from "node:fs";

// ═══════════════════════════════════════════════════════
// Phase 1: Session Analysis (A-Evolve BatchAnalysis equivalent)
// ═══════════════════════════════════════════════════════

export function analyzeSession(observations: ObsRecord[]): SessionAnalysis {
  const scored = observations.filter(o => o.score !== null);
  const total = scored.length;
  const errors = scored.filter(o => o.result === "error");
  const successRate = total > 0 ? (total - errors.length) / total : 1;
  const avgScore = total > 0 ? scored.reduce((sum, o) => sum + (o.score ?? 0), 0) / total : 0;

  // Score distribution buckets (like A-Evolve's score_buckets)
  const buckets: Record<string, number> = { "0.0-0.2": 0, "0.2-0.4": 0, "0.4-0.6": 0, "0.6-0.8": 0, "0.8-1.0": 0 };
  for (const o of scored) {
    const s = o.score ?? 0;
    if (s < 0.2) buckets["0.0-0.2"]++;
    else if (s < 0.4) buckets["0.2-0.4"]++;
    else if (s < 0.6) buckets["0.4-0.6"]++;
    else if (s < 0.8) buckets["0.6-0.8"]++;
    else buckets["0.8-1.0"]++;
  }

  // Per-tool stats (like A-Evolve's tool_error_counts)
  const toolMap: Record<string, ToolStats> = {};
  for (const o of scored) {
    const cat = o.tool_category ?? "other";
    if (!toolMap[cat]) {
      toolMap[cat] = { tool_category: cat as ToolCategory, total: 0, successes: 0, errors: 0, avg_score: 0, failure_categories: {} };
    }
    const ts = toolMap[cat];
    ts.total++;
    if (o.result === "error") {
      ts.errors++;
      const fc = o.failure_category ?? "other";
      ts.failure_categories[fc] = (ts.failure_categories[fc] ?? 0) + 1;
    } else {
      ts.successes++;
    }
    ts.avg_score = ((ts.avg_score * (ts.total - 1)) + (o.score ?? 0)) / ts.total;
  }

  // Per-error stats
  const errorStats: Record<string, number> = {};
  for (const o of errors) {
    const fc = o.failure_category ?? "other";
    errorStats[fc] = (errorStats[fc] ?? 0) + 1;
  }

  // Per-file-extension stats (like A-Evolve's TaskTypePerformanceTracker)
  const extMap: Record<string, { total: number; errors: number; success_rate: number }> = {};
  for (const o of scored) {
    const ext = o.file_ext ?? "unknown";
    if (!extMap[ext]) extMap[ext] = { total: 0, errors: 0, success_rate: 0 };
    extMap[ext].total++;
    if (o.result === "error") extMap[ext].errors++;
  }
  for (const ext of Object.keys(extMap)) {
    const e = extMap[ext];
    e.success_rate = e.total > 0 ? Math.round(((e.total - e.errors) / e.total) * 1000) / 1000 : 1;
  }

  // Dimension averages
  const dimsScored = scored.filter(o => o.dimensions !== null);
  const dimAvg: ScoreDimensions = { tool_success: 0, output_quality: 0, execution_cost: 0 };
  if (dimsScored.length > 0) {
    for (const o of dimsScored) {
      dimAvg.tool_success += o.dimensions!.tool_success;
      dimAvg.output_quality += o.dimensions!.output_quality;
      dimAvg.execution_cost += o.dimensions!.execution_cost;
    }
    dimAvg.tool_success = Math.round((dimAvg.tool_success / dimsScored.length) * 1000) / 1000;
    dimAvg.output_quality = Math.round((dimAvg.output_quality / dimsScored.length) * 1000) / 1000;
    dimAvg.execution_cost = Math.round((dimAvg.execution_cost / dimsScored.length) * 1000) / 1000;
  }

  return {
    total_observations: total,
    success_rate: Math.round(successRate * 1000) / 1000,
    avg_score: Math.round(avgScore * 1000) / 1000,
    score_distribution: buckets,
    per_tool_stats: toolMap,
    per_error_stats: errorStats,
    per_ext_stats: extMap,
    failure_patterns: [], // filled in Phase 2
    dimension_averages: dimAvg,
  };
}

// ═══════════════════════════════════════════════════════
// Phase 2: Failure Pattern Detection
// (A-Evolve FailurePatternDetector equivalent)
// ═══════════════════════════════════════════════════════

export function detectPatterns(observations: ObsRecord[]): DetectedPattern[] {
  const patterns: DetectedPattern[] = [];
  const scored = observations.filter(o => o.result !== null);

  // --- Pattern 1: repeated_same_error ---
  // Same failure_category + same file + same error hash in consecutive observations (#4 dedup)
  {
    let streak = 1;
    let streakFile = "";
    let streakCategory = "";
    let prevHash = "";
    for (let i = 1; i < scored.length; i++) {
      const prev = scored[i - 1];
      const curr = scored[i];
      const currSnippet = ((curr as unknown as Record<string, unknown>).error_snippet as string) ?? "";
      const prevSnippet = ((prev as unknown as Record<string, unknown>).error_snippet as string) ?? "";
      const currHash = currSnippet ? hashString(normalizeError(currSnippet)) : "";
      const prevHashVal = prevSnippet ? hashString(normalizeError(prevSnippet)) : "";

      if (
        curr.result === "error" && prev.result === "error" &&
        curr.failure_category === prev.failure_category &&
        curr.failure_category !== null &&
        curr.action && prev.action && extractFile(curr.action) === extractFile(prev.action) &&
        (currHash === prevHashVal || !currHash || !prevHashVal)  // same error or no snippet
      ) {
        streak++;
        streakFile = extractFile(curr.action) ?? "";
        streakCategory = curr.failure_category ?? "";
        prevHash = currHash;
      } else {
        if (streak >= REPEATED_ERROR_MIN) {
          patterns.push({
            pattern_type: "repeated_same_error",
            description: `${streakCategory} repeated ${streak}x on ${streakFile}${prevHash ? ` [hash:${prevHash}]` : ""}`,
            count: streak,
            involved_files: streakFile ? [streakFile] : [],
            suggested_remediation: `Stop retrying the same approach for ${streakCategory}. Re-read the full error, check if fixing symptom vs root cause. Consider reverting to last working state.`,
          });
        }
        streak = 1;
        prevHash = "";
      }
    }
    if (streak >= REPEATED_ERROR_MIN) {
      patterns.push({
        pattern_type: "repeated_same_error",
        description: `${streakCategory} repeated ${streak}x on ${streakFile}${prevHash ? ` [hash:${prevHash}]` : ""}`,
        count: streak,
        involved_files: streakFile ? [streakFile] : [],
        suggested_remediation: `Stop retrying the same approach for ${streakCategory}. Re-read the full error, check if fixing symptom vs root cause.`,
      });
    }
  }

  // --- Pattern 2: fix_then_break ---
  // Edit/Write success → Bash error within 3 steps, referencing same file
  {
    const ftbFiles: Record<string, number> = {};
    for (let i = 0; i < scored.length; i++) {
      const o = scored[i];
      if ((o.tool_category === "edit" || o.tool_category === "write") && o.result === "success" && o.action) {
        const file = extractFile(o.action) ?? o.action;
        // Look ahead up to 3 steps
        for (let j = i + 1; j < Math.min(i + FTB_LOOKAHEAD + 1, scored.length); j++) {
          const next = scored[j];
          if (next.result === "error" && next.tool_category === "bash") {
            const snippet = ((next as unknown as Record<string, unknown>).error_snippet as string) ?? "";
            if (snippet.includes(file) || snippet.includes(pathBasename(file))) {
              ftbFiles[file] = (ftbFiles[file] ?? 0) + 1;
              break;
            }
          }
        }
      }
    }
    const ftbEntries = Object.entries(ftbFiles).filter(([_, c]) => c >= FTB_MIN_CYCLES);
    if (ftbEntries.length > 0) {
      patterns.push({
        pattern_type: "fix_then_break",
        description: `Edit→Break cycle on ${ftbEntries.map(e => e[0]).join(", ")}`,
        count: ftbEntries.reduce((s, e) => s + e[1], 0),
        involved_files: ftbEntries.map(e => e[0]),
        suggested_remediation: "Before editing, run the build/test to establish a baseline. After editing, immediately verify. If the edit broke something, revert and reconsider approach.",
      });
    }
  }

  // --- Pattern 3: long_debug_loop ---
  // Same file appears in 5+ consecutive observations
  {
    const fileRuns: Record<string, number> = {};
    let prevFile = "";
    let runLength = 0;
    for (const o of scored) {
      const file = extractFile(o.action ?? "") ?? "";
      if (file && file === prevFile) {
        runLength++;
      } else {
        if (runLength >= DEBUG_LOOP_MIN && prevFile) {
          fileRuns[prevFile] = Math.max(fileRuns[prevFile] ?? 0, runLength);
        }
        prevFile = file;
        runLength = 1;
      }
    }
    if (runLength >= DEBUG_LOOP_MIN && prevFile) {
      fileRuns[prevFile] = Math.max(fileRuns[prevFile] ?? 0, runLength);
    }
    for (const [file, count] of Object.entries(fileRuns)) {
      patterns.push({
        pattern_type: "long_debug_loop",
        description: `Stuck on ${pathBasename(file)} for ${count} consecutive operations`,
        count,
        involved_files: [file],
        suggested_remediation: "Stuck in debug loop. Stop, re-read the surrounding code context (100+ lines), form a complete mental model before the next attempt.",
      });
    }
  }

  // --- Pattern 4: thrashing ---
  // Alternating edit-error on same file. O(n) single pass with per-file counters.
  {
    const fileStats: Record<string, { edits: number; errors: number }> = {};
    for (const o of scored) {
      const file = extractFile(o.action ?? "") ?? "";
      if (!file) continue;
      if (!fileStats[file]) fileStats[file] = { edits: 0, errors: 0 };
      if (o.tool_category === "edit" || o.tool_category === "write") fileStats[file].edits++;
      if (o.result === "error") fileStats[file].errors++;
    }
    for (const [file, stats] of Object.entries(fileStats)) {
      // Thrashing = multiple edits AND multiple errors on the same file
      if (stats.edits >= THRASH_MIN_EDITS && stats.errors >= THRASH_MIN_ERRORS) {
        patterns.push({
          pattern_type: "thrashing",
          description: `Edit↔Error thrashing on ${pathBasename(file)} (${stats.edits} edits, ${stats.errors} errors)`,
          count: stats.edits + stats.errors,
          involved_files: [file],
          suggested_remediation: "Alternating edit-error cycle detected. Stop and read the surrounding context. Form a complete understanding before the next edit.",
        });
      }
    }
  }

  return patterns;
}

function extractFile(action: string): string | null {
  if (!action) return null;
  // Match file paths
  const match = action.match(/(\/[\w./-]+\.\w+)/);
  return match ? match[1] : null;
}

// Use pathBasename from node:path (imported above)

// ═══════════════════════════════════════════════════════
// Phase 3: Stagnation Gating
// (A-Evolve _check_stagnation_gate equivalent)
// ═══════════════════════════════════════════════════════

interface StagnationResult {
  shouldRollback: boolean;
  improved: boolean;
  rolledBackCount: number;
}

export function checkStagnation(metrics: Metrics, currentScore: number): StagnationResult {
  const result: StagnationResult = { shouldRollback: false, improved: false, rolledBackCount: 0 };

  // First session — no comparison
  if (metrics.total_sessions === 0 || metrics.best_score === 0) {
    result.improved = true;
    return result;
  }

  const improvement = currentScore - metrics.best_score;

  if (improvement >= IMPROVEMENT_THRESHOLD) {
    // Improved! Backup current evolved skills as checkpoint
    result.improved = true;
    if (existsSync(EVOLVED_DIR)) {
      rmDirSync(EVOLVED_BACKUP_DIR);
      copyDirSync(EVOLVED_DIR, EVOLVED_BACKUP_DIR);
    }
    return result;
  }

  // No improvement — check stagnation
  metrics.stagnation_count++;
  if (metrics.stagnation_count >= STAGNATION_LIMIT) {
    const degradation = metrics.best_score - currentScore;
    // Rollback if degraded >5% OR best was below 90%
    if (degradation > 0.05 || metrics.best_score < 0.90) {
      if (existsSync(EVOLVED_BACKUP_DIR)) {
        const beforeCount = existsSync(EVOLVED_DIR) ? listDirs(EVOLVED_DIR).length : 0;
        rmDirSync(EVOLVED_DIR);
        copyDirSync(EVOLVED_BACKUP_DIR, EVOLVED_DIR);
        result.shouldRollback = true;
        result.rolledBackCount = beforeCount;
        metrics.stagnation_count = 0;
        hint("reflect", `⚠ Stagnation detected (${STAGNATION_LIMIT} sessions). Rolled back evolved skills to best checkpoint.`);
      }
    }
  }

  return result;
}

// ═══════════════════════════════════════════════════════
// Phase 4: Smart Skill Seeding
// ═══════════════════════════════════════════════════════

function seedSmartSkills(analysis: SessionAnalysis, existingSkills: string[]): number {
  let seeded = 0;
  if (existingSkills.length >= MAX_EVOLVED_SKILLS) return 0;
  const remaining = () => MAX_EVOLVED_SKILLS - existingSkills.length - seeded;

  // 4a. Seed from detected failure patterns
  for (const pattern of analysis.failure_patterns) {
    if (remaining() <= 0) break;
    const skillName = `evo-${pattern.pattern_type}`;
    if (existingSkills.includes(skillName)) continue;

    const content = buildPatternSkill(pattern);
    writeSkill(skillName, content);
    seeded++;
  }

  // 4b. Seed from weak tool categories (<60% success, 5+ obs)
  for (const [_cat, stats] of Object.entries(analysis.per_tool_stats)) {
    if (remaining() <= 0) break;
    const successRate = stats.total > 0 ? stats.successes / stats.total : 1;
    if (successRate >= WEAK_TOOL_RATE || stats.total < WEAK_TOOL_MIN_OBS) continue;

    const skillName = `evo-${stats.tool_category}-discipline`;
    if (existingSkills.includes(skillName)) continue;

    const content = buildToolSkill(stats);
    writeSkill(skillName, content);
    seeded++;
  }

  // 4c. Seed from weak file extensions (<50% success, 3+ obs)
  for (const [ext, stats] of Object.entries(analysis.per_ext_stats)) {
    if (remaining() <= 0) break;
    if (stats.success_rate >= WEAK_EXT_RATE || stats.total < WEAK_EXT_MIN_OBS || ext === "unknown") continue;

    const cleanExt = ext.replace(".", "");
    const skillName = `evo-${cleanExt}-care`;
    if (existingSkills.includes(skillName)) continue;

    const content = buildExtSkill(ext, stats);
    writeSkill(skillName, content);
    seeded++;
  }

  // 4d. Seed from high-frequency failure categories (5+ occurrences)
  for (const [category, count] of Object.entries(analysis.per_error_stats)) {
    if (remaining() <= 0) break;
    if (count < HIGH_FREQ_ERROR_MIN) continue;

    const skillName = `evo-fix-${category.replace(/_/g, "-")}`;
    if (existingSkills.includes(skillName)) continue;

    const content = buildFailureCategorySkill(category, count);
    writeSkill(skillName, content);
    seeded++;
  }

  return seeded;
}

function writeSkill(skillName: string, content: string): void {
  const skillDir = join(EVOLVED_DIR, skillName);
  ensureDir(skillDir);
  writeFileSync(join(skillDir, "SKILL.md"), content);
}

function buildPatternSkill(pattern: DetectedPattern): string {
  return [
    "---",
    `name: ${pattern.pattern_type}`,
    `description: "Auto-evolved from ${pattern.count}x ${pattern.pattern_type} pattern detection."`,
    "---",
    "",
    `# ${pattern.pattern_type}`,
    "",
    `**Detected pattern**: ${pattern.description}`,
    `**Files involved**: ${pattern.involved_files.join(", ") || "various"}`,
    "",
    "## Remediation",
    pattern.suggested_remediation,
    "",
    "## Red Flags",
    "- Retrying the same approach that already failed",
    "- Not reading the full error context before acting",
    "- Patching symptoms instead of root cause",
  ].join("\n");
}

function buildToolSkill(stats: ToolStats): string {
  const rate = stats.total > 0 ? ((stats.successes / stats.total) * 100).toFixed(0) : "0";
  const topFailures = Object.entries(stats.failure_categories)
    .sort((a, b) => b[1] - a[1])
    .slice(0, 3)
    .map(([cat, count]) => `- ${cat}: ${count} occurrences`)
    .join("\n");
  return [
    "---",
    `name: ${stats.tool_category}-discipline`,
    `description: "Auto-evolved. ${stats.tool_category} tool success rate was ${rate}%."`,
    "---",
    "",
    `# ${stats.tool_category} discipline`,
    "",
    `Success rate: ${rate}% (${stats.successes}/${stats.total})`,
    "",
    "## Top Failure Types",
    topFailures || "- various errors",
    "",
    "## Process",
    `1. Before using ${stats.tool_category}: verify preconditions`,
    "2. Check the expected output format",
    "3. Validate paths and arguments exist",
    "4. On failure: read the FULL error, don't retry blindly",
    "",
    "## Red Flags",
    `- ${stats.tool_category} success rate still below 60%`,
    "- Same error type repeating",
  ].join("\n");
}

function buildExtSkill(ext: string, stats: { total: number; errors: number; success_rate: number }): string {
  const rate = (stats.success_rate * 100).toFixed(0);
  return [
    "---",
    `name: ${ext.replace(".", "")}-care`,
    `description: "Auto-evolved. ${ext} files had ${rate}% success rate."`,
    "---",
    "",
    `# ${ext} file care`,
    "",
    `Success rate: ${rate}% across ${stats.total} operations`,
    "",
    "## Process",
    `1. Before editing ${ext} files: run type-check / lint / build`,
    "2. After editing: immediately verify (build, test, lint)",
    "3. If error: read the full diagnostic before re-editing",
    "4. Consider running the relevant test file specifically",
    "",
    "## Red Flags",
    `- Editing ${ext} files without verifying afterward`,
    "- Ignoring compiler/linter warnings",
  ].join("\n");
}

function buildFailureCategorySkill(category: string, count: number): string {
  const remediations: Record<string, string> = {
    type_error: "Check variable types. Read the full type signature. Use explicit type annotations to catch issues early.",
    syntax_error: "Check brackets, commas, semicolons. Ensure template literals are properly closed. Run a formatter.",
    test_fail: "Read the assertion message carefully. Check expected vs actual values. Run the specific failing test in isolation.",
    lint_fail: "Run the linter and fix all warnings. Configure the editor to show lint errors inline.",
    build_fail: "Run `tsc --noEmit` (or equivalent) to see all errors. Fix type errors before runtime testing.",
    permission_denied: "Check file permissions. Don't try to write to system directories. Use sudo only when explicitly requested.",
    timeout: "Command took too long. Consider using a more targeted approach or adding a timeout flag.",
    not_found: "File or command not found. Verify the path exists. Use glob/find to locate the correct file.",
    runtime_error: "Unexpected runtime error. Read the full stack trace. Check for undefined values and edge cases.",
  };
  return [
    "---",
    `name: fix-${category.replace(/_/g, "-")}`,
    `description: "Auto-evolved from ${count}x ${category} failures."`,
    "---",
    "",
    `# Fix ${category.replace(/_/g, " ")}`,
    "",
    `Detected ${count} occurrences of ${category} in recent session.`,
    "",
    "## Remediation",
    remediations[category] ?? "Read the full error message. Identify root cause. Fix the cause, not the symptom.",
    "",
    "## Process",
    "1. Stop — do not retry blindly",
    "2. Read the full error message and stack trace",
    "3. Form a hypothesis about root cause",
    "4. Fix the root cause, not the symptom",
    "5. Verify with a test or build",
    "",
    "## Red Flags",
    "- Retrying the same approach unchanged",
    "- Patching symptoms instead of root cause",
    "- Ignoring the error message details",
  ].join("\n");
}

// ═══════════════════════════════════════════════════════
// Phase 5: Trend Computation
// ═══════════════════════════════════════════════════════

export function computeTrend(history: SessionScoreEntry[]): "improving" | "stable" | "declining" {
  const recent = history.slice(-5);
  if (recent.length < 2) return "stable";
  // Simple linear slope of avg_score
  const n = recent.length;
  let sumX = 0, sumY = 0, sumXY = 0, sumXX = 0;
  for (let i = 0; i < n; i++) {
    sumX += i;
    sumY += recent[i].avg_score;
    sumXY += i * recent[i].avg_score;
    sumXX += i * i;
  }
  const slope = (n * sumXY - sumX * sumY) / (n * sumXX - sumX * sumX);
  if (slope > 0.01) return "improving";
  if (slope < -0.01) return "declining";
  return "stable";
}

// ═══════════════════════════════════════════════════════
// Phase 6: Summary Builder
// ═══════════════════════════════════════════════════════

function buildSummary(analysis: SessionAnalysis): string {
  const parts: string[] = [];
  parts.push(`${analysis.total_observations} obs, ${(analysis.success_rate * 100).toFixed(1)}% success, avg=${analysis.avg_score}`);

  const topErrors = Object.entries(analysis.per_error_stats)
    .sort((a, b) => b[1] - a[1])
    .slice(0, 3)
    .map(([cat, count]) => `${cat}:${count}`);
  if (topErrors.length > 0) parts.push(`errors=[${topErrors.join(",")}]`);

  if (analysis.failure_patterns.length > 0) {
    parts.push(`patterns=[${analysis.failure_patterns.map(p => p.pattern_type).join(",")}]`);
  }

  return parts.join(" | ");
}

// ═══════════════════════════════════════════════════════
// Phase 6b: Skill Attribution (#6)
// ═══════════════════════════════════════════════════════

function updateSkillAttribution(
  metrics: Metrics,
  analysis: SessionAnalysis,
  evolvedSkills: string[],
): void {
  if (!metrics.skill_attribution) metrics.skill_attribution = {};

  for (const skill of evolvedSkills) {
    if (!metrics.skill_attribution[skill]) {
      metrics.skill_attribution[skill] = {
        skill_name: skill,
        sessions_active: 0,
        avg_score_with: 0,
        avg_score_without: 0,
        first_seen: now(),
      };
    }
    const attr = metrics.skill_attribution[skill];
    attr.sessions_active++;
    // Running average of score when this skill is active
    attr.avg_score_with = Math.round(
      ((attr.avg_score_with * (attr.sessions_active - 1)) + analysis.avg_score) / attr.sessions_active * 1000,
    ) / 1000;
  }

  // Compute avg_score_without for each skill (sessions where it wasn't present)
  const totalSessions = metrics.total_sessions + 1;
  for (const [name, attr] of Object.entries(metrics.skill_attribution)) {
    const sessionsWithout = totalSessions - attr.sessions_active;
    if (sessionsWithout > 0 && metrics.score_history.length > 0) {
      // Approximate: total_avg = (with * active + without * inactive) / total
      const totalAvg = metrics.avg_success_rate;
      attr.avg_score_without = Math.round(
        ((totalAvg * totalSessions) - (attr.avg_score_with * attr.sessions_active)) / sessionsWithout * 1000,
      ) / 1000;
    }
  }

  // Remove attribution for skills that no longer exist
  for (const name of Object.keys(metrics.skill_attribution)) {
    if (!evolvedSkills.includes(name)) {
      delete metrics.skill_attribution[name];
    }
  }
}

// ═══════════════════════════════════════════════════════
// Phase 7: Cross-Project Learning (#2)
// ═══════════════════════════════════════════════════════

function exportToGlobalPatterns(analysis: SessionAnalysis, patterns: DetectedPattern[]): void {
  // Only export if opt-in
  if (!fsExistsSync(CROSS_PROJECT_OPT_IN_FILE)) return;

  ensureDir(GLOBAL_HARNESS_DIR);
  const projectName = CWD.split("/").pop() ?? "unknown";
  const record = {
    timestamp: now(),
    project: projectName,
    success_rate: analysis.success_rate,
    avg_score: analysis.avg_score,
    per_error_stats: analysis.per_error_stats,
    failure_patterns: patterns.map(p => ({
      pattern_type: p.pattern_type,
      count: p.count,
      remediation: p.suggested_remediation,
    })),
    weak_tools: Object.entries(analysis.per_tool_stats)
      .filter(([_, s]) => s.total >= 5 && (s.successes / s.total) < 0.6)
      .map(([cat]) => cat),
  };
  appendJsonl(GLOBAL_PATTERNS_FILE, record);
}

// ═══════════════════════════════════════════════════════
// Main Hook
// ═══════════════════════════════════════════════════════

// Only run as hook when invoked directly (not when imported by tests)
const isMain = import.meta.url === `file://${process.argv[1]}`;
if (isMain) runHook(() => {
  if (!harnessExists()) return;
  if (!existsSync(OBS_DIR)) return;

  // 1. Collect observations from all session files for today
  const todayStr = new Date().toISOString().slice(0, 10).replace(/-/g, "");
  const obsFiles = listFiles(OBS_DIR, ".jsonl")
    .filter(f => f.includes(todayStr))
    .sort();
  if (obsFiles.length === 0) return;

  // Merge all today's session files (supports session-ID based separation #3)
  let observations: ObsRecord[] = [];
  for (const f of obsFiles) {
    const records = readJsonlSafe(join(OBS_DIR, f)) as unknown as ObsRecord[];
    observations = observations.concat(records);
  }
  if (observations.length < 3) return;

  // 2. Analyze session
  const analysis = analyzeSession(observations);

  // 3. Detect failure patterns
  analysis.failure_patterns = detectPatterns(observations);

  // 4. Load metrics + stagnation gating
  const metrics = readJsonSafe<Metrics>(METRICS_FILE, defaultMetrics());
  const stagnation = checkStagnation(metrics, analysis.avg_score);

  // 5. Seed evolved skills (skip if just rolled back)
  ensureDir(EVOLVED_DIR);
  const existingSkills = listDirs(EVOLVED_DIR);
  let seeded = 0;
  if (!stagnation.shouldRollback) {
    seeded = seedSmartSkills(analysis, existingSkills);
  }

  // 6. Gate: validate evolved skills (format check + cap)
  gateSkills();

  // 6b. Skill attribution (#6)
  updateSkillAttribution(metrics, analysis, listDirs(EVOLVED_DIR));

  // 7. Cross-project export (#2)
  exportToGlobalPatterns(analysis, analysis.failure_patterns);

  // 8. Record evolution history
  const record: EvolutionRecord = {
    timestamp: now(),
    observations: analysis.total_observations,
    success_rate: analysis.success_rate,
    avg_score: analysis.avg_score,
    error_patterns: analysis.per_error_stats,
    failure_patterns: analysis.failure_patterns,
    skills_seeded: seeded,
    skills_rolled_back: stagnation.rolledBackCount,
    total_evolved: listDirs(EVOLVED_DIR).length,
    analysis_summary: buildSummary(analysis),
  };
  appendJsonl(EVOLUTION_FILE, record);

  // 9. Save session handoff context (#10)
  const lastErrors = observations
    .filter(o => o.result === "error")
    .slice(-3)
    .map(o => `${o.failure_category}: ${((o as unknown as Record<string, unknown>).error_snippet as string)?.slice(0, 100) ?? o.action}`)
    .join(" | ");
  if (lastErrors) metrics.last_error_context = lastErrors;

  // 10. Update metrics
  const scoreEntry: SessionScoreEntry = {
    timestamp: now(),
    success_rate: analysis.success_rate,
    avg_score: analysis.avg_score,
    observations: analysis.total_observations,
    dimension_averages: analysis.dimension_averages,
  };
  metrics.score_history.push(scoreEntry);
  // Keep last 50 sessions
  if (metrics.score_history.length > 50) {
    metrics.score_history = metrics.score_history.slice(-50);
  }

  metrics.total_sessions++;
  metrics.avg_success_rate = Math.round(
    ((metrics.avg_success_rate * (metrics.total_sessions - 1)) + analysis.success_rate) / metrics.total_sessions * 1000,
  ) / 1000;
  metrics.total_evolved_skills = record.total_evolved;
  metrics.last_session = now();

  if (stagnation.improved) {
    metrics.best_score = analysis.avg_score;
    metrics.best_session = now();
    metrics.stagnation_count = 0;
  }
  metrics.trend = computeTrend(metrics.score_history);

  writeFileSync(METRICS_FILE, JSON.stringify(metrics, null, 2));

  // 11. Report
  hint("reflect", `Session: ${(analysis.success_rate * 100).toFixed(1)}% success, avg_score=${analysis.avg_score} (${analysis.total_observations} obs)`);

  // Report weak areas
  const weakTools = Object.entries(analysis.per_tool_stats)
    .filter(([_, s]) => s.total >= 5 && (s.successes / s.total) < 0.6)
    .map(([cat, s]) => `${cat} ${((s.successes / s.total) * 100).toFixed(0)}%`);
  if (weakTools.length > 0) hint("reflect", `Weak tools: ${weakTools.join(", ")}`);

  const weakExts = Object.entries(analysis.per_ext_stats)
    .filter(([_, s]) => s.total >= 3 && s.success_rate < 0.5)
    .map(([ext, s]) => `${ext} ${(s.success_rate * 100).toFixed(0)}%`);
  if (weakExts.length > 0) hint("reflect", `Weak file types: ${weakExts.join(", ")}`);

  if (analysis.failure_patterns.length > 0) {
    const patternSummary = analysis.failure_patterns
      .map(p => `${p.pattern_type}(${p.count})`)
      .join(", ");
    hint("reflect", `Patterns: ${patternSummary}`);
  }

  if (seeded > 0) hint("reflect", `Evolved ${seeded} new skill(s)`);
  if (stagnation.shouldRollback) hint("reflect", `Rolled back ${stagnation.rolledBackCount} stagnant skills`);
  hint("reflect", `Trend: ${metrics.trend} (${metrics.score_history.length} sessions)`);

  // Report skill attribution (#6)
  if (metrics.skill_attribution) {
    const effective = Object.values(metrics.skill_attribution)
      .filter(a => a.sessions_active >= 2 && a.avg_score_with > a.avg_score_without + 0.02);
    const ineffective = Object.values(metrics.skill_attribution)
      .filter(a => a.sessions_active >= 2 && a.avg_score_with < a.avg_score_without - 0.02);

    if (effective.length > 0) {
      hint("reflect", `Effective skills: ${effective.map(s => `${s.skill_name}(+${((s.avg_score_with - s.avg_score_without) * 100).toFixed(0)}%)`).join(", ")}`);
    }
    if (ineffective.length > 0) {
      hint("reflect", `Ineffective skills: ${ineffective.map(s => s.skill_name).join(", ")} — consider /evolve rollback`);
    }
  }
});

// ── Legacy gate (format validation + cap) ───────────

export function gateSkills(): { kept: { name: string }[]; removed: { name: string }[] } {
  const result = { kept: [] as { name: string }[], removed: [] as { name: string }[] };
  if (!existsSync(EVOLVED_DIR)) return result;

  const skills = listDirs(EVOLVED_DIR);
  for (const name of skills) {
    const skillFile = join(EVOLVED_DIR, name, "SKILL.md");
    if (!existsSync(skillFile)) {
      result.removed.push({ name });
      rmDirSync(join(EVOLVED_DIR, name));
      continue;
    }
    const content = readFileSync(skillFile, "utf8");
    const body = content.replace(/---[\s\S]*?---/, "").trim();
    if (!content.startsWith("---") || body.length < 20) {
      result.removed.push({ name });
      rmDirSync(join(EVOLVED_DIR, name));
    } else {
      result.kept.push({ name });
    }
  }

  // Enforce cap — remove oldest first
  const remaining = listDirs(EVOLVED_DIR).sort();
  if (remaining.length > MAX_EVOLVED_SKILLS) {
    const excess = remaining.slice(0, remaining.length - MAX_EVOLVED_SKILLS);
    for (const name of excess) {
      result.removed.push({ name });
      rmDirSync(join(EVOLVED_DIR, name));
    }
  }
  return result;
}
