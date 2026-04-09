#!/usr/bin/env node
/**
 * Ring 0: resume — SessionStart
 *
 * 1. Auto-init .harness/ if missing (+ cold-start presets #1)
 * 2. Restore previous session snapshot
 * 3. Report eval metrics (score, trend, weak areas) from metrics.json
 * 4. Session handoff: last error context + pending tasks (#10)
 * 5. Cross-project learning hints (#2)
 * 6. Load evolved skills, memory, team, stack detection
 */
import { existsSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { runHook, hint, raw, harnessExists, ensureDir, CWD, HARNESS_DIR, SESSIONS_DIR, MEMORY_DIR, EVOLVED_DIR, TEAM_DIR, OBS_DIR, METRICS_FILE, GLOBAL_PATTERNS_FILE, CROSS_PROJECT_OPT_IN_FILE, readJsonSafe, readJsonlSafe, listDirs, listFiles, defaultMetrics, } from "./common.js";
const BANNER = [
    "",
    "  ┌─┐┌─┐┬┌─┐  ┌─┐┌┬┐  ┬ ┬┌─┐┬─┐┌┐┌┌─┐┌─┐┌─┐",
    "  ├┤ ├─┘││    ├─┤  │   ├─┤├─┤├┬┘│││├┤ └─┐└─┐",
    "  └─┘┴  ┴└─┘  ┴ ┴  ┴   ┴ ┴┴ ┴┴└─┘└┘└─┘└─┘└─┘",
    "          6 commands · auto skills · self-evolving",
    "",
];
const STACK_FILES = {
    "package.json": "Node.js",
    "go.mod": "Go",
    "pyproject.toml": "Python",
    "Cargo.toml": "Rust",
    "build.gradle": "Java/Kotlin",
    "Gemfile": "Ruby",
    "pom.xml": "Java (Maven)",
    "composer.json": "PHP",
};
// ── Cold-start presets (#1) ───────────────────────────
const PRESETS = {
    "Node.js": {
        skills: {
            "evo-ts-care": [
                "---",
                "name: ts-care",
                'description: "Preset: TypeScript files need type-check after every edit."',
                "---",
                "",
                "# TypeScript file care (preset)",
                "",
                "## Process",
                "1. Run `tsc --noEmit` before and after editing .ts/.tsx files",
                "2. Check for strict null violations",
                "3. Validate import paths resolve correctly",
                "",
                "## Red Flags",
                "- Editing .ts files without verifying types afterward",
                "- Ignoring `any` type assertions",
            ].join("\n"),
            "evo-fix-build-fail": [
                "---",
                "name: fix-build-fail",
                'description: "Preset: Common Node.js build failures."',
                "---",
                "",
                "# Fix build failures (preset)",
                "",
                "## Remediation",
                "1. Check tsconfig.json paths and module resolution",
                "2. Verify all imports exist (`tsc --noEmit`)",
                "3. Check package.json type field (module vs commonjs)",
                "",
                "## Red Flags",
                "- Ignoring TypeScript strict mode errors",
                "- Missing type declarations for dependencies",
            ].join("\n"),
        },
    },
    "Go": {
        skills: {
            "evo-go-care": [
                "---",
                "name: go-care",
                'description: "Preset: Go files need vet + build check after edits."',
                "---",
                "",
                "# Go file care (preset)",
                "",
                "## Process",
                "1. Run `go vet ./...` after editing",
                "2. Run `go build ./...` to catch compile errors early",
                "3. Check for unused imports (will fail build)",
                "",
                "## Red Flags",
                "- Editing Go files without running vet",
                "- Leaving unused imports (Go compiler will reject)",
            ].join("\n"),
        },
    },
    "Python": {
        skills: {
            "evo-py-care": [
                "---",
                "name: py-care",
                'description: "Preset: Python files benefit from type-check + lint after edits."',
                "---",
                "",
                "# Python file care (preset)",
                "",
                "## Process",
                "1. Run `ruff check` or `flake8` after editing",
                "2. Run `mypy` if type hints are used",
                "3. Check import order and unused imports",
                "",
                "## Red Flags",
                "- Editing .py without lint check",
                "- Ignoring type-checker warnings",
            ].join("\n"),
        },
    },
    "Rust": {
        skills: {
            "evo-rs-care": [
                "---",
                "name: rs-care",
                'description: "Preset: Rust files need cargo check after edits."',
                "---",
                "",
                "# Rust file care (preset)",
                "",
                "## Process",
                "1. Run `cargo check` after editing .rs files",
                "2. Run `cargo clippy` for idiomatic patterns",
                "3. Check borrow checker errors carefully — read the full message",
                "",
                "## Red Flags",
                "- Editing Rust without cargo check",
                "- Using `.unwrap()` in non-test code",
            ].join("\n"),
        },
    },
};
function applyColdStartPresets(stacks) {
    let applied = 0;
    for (const stack of stacks) {
        const preset = PRESETS[stack];
        if (!preset)
            continue;
        for (const [skillName, content] of Object.entries(preset.skills)) {
            const skillDir = join(EVOLVED_DIR, skillName);
            if (existsSync(skillDir))
                continue; // don't overwrite existing
            ensureDir(skillDir);
            writeFileSync(join(skillDir, "SKILL.md"), content);
            applied++;
        }
    }
    return applied;
}
// ── Cross-project hints (#2) ────────────────────────
function getCrossProjectHints() {
    if (!existsSync(CROSS_PROJECT_OPT_IN_FILE))
        return [];
    if (!existsSync(GLOBAL_PATTERNS_FILE))
        return [];
    const projectName = CWD.split("/").pop() ?? "";
    const records = readJsonlSafe(GLOBAL_PATTERNS_FILE);
    // Get patterns from OTHER projects only
    const otherPatterns = records.filter(r => r.project !== projectName);
    if (otherPatterns.length === 0)
        return [];
    // Aggregate weak tools across projects
    const weakToolCounts = {};
    for (const r of otherPatterns.slice(-20)) {
        const tools = r.weak_tools ?? [];
        for (const t of tools) {
            weakToolCounts[t] = (weakToolCounts[t] ?? 0) + 1;
        }
    }
    const hints = [];
    const frequentWeak = Object.entries(weakToolCounts).filter(([_, c]) => c >= 2);
    if (frequentWeak.length > 0) {
        hints.push(`Cross-project: ${frequentWeak.map(([t, c]) => `${t} weak in ${c} projects`).join(", ")}`);
    }
    return hints;
}
// ── Main ─────────────────────────────────────────────
runHook(() => {
    // Auto-init .harness/ if missing — show banner on first run
    if (!harnessExists()) {
        BANNER.forEach(raw);
        ensureDir(HARNESS_DIR);
        ensureDir(OBS_DIR);
        ensureDir(SESSIONS_DIR);
        ensureDir(MEMORY_DIR);
        ensureDir(EVOLVED_DIR);
        hint("resume", "Initialized .harness/ — Ring 3 evolution loop active");
    }
    // 1. Latest session snapshot
    const snapshots = listFiles(SESSIONS_DIR, ".json").sort();
    if (snapshots.length > 0) {
        const latest = readJsonSafe(join(SESSIONS_DIR, snapshots[snapshots.length - 1]), { timestamp: "", type: "", summary: "", pending_tasks: [], context_usage: null });
        if (latest.summary)
            hint("resume", `Previous: ${latest.summary}`);
        if (latest.pending_tasks.length > 0) {
            hint("resume", `Pending: ${latest.pending_tasks.join(", ")}`);
        }
    }
    // 2. Eval metrics from previous sessions
    const metrics = readJsonSafe(METRICS_FILE, defaultMetrics());
    if (metrics.total_sessions > 0) {
        const lastEntry = metrics.score_history[metrics.score_history.length - 1];
        const scoreStr = lastEntry
            ? `${(lastEntry.success_rate * 100).toFixed(0)}% success, avg_score=${lastEntry.avg_score}`
            : `${(metrics.avg_success_rate * 100).toFixed(0)}% avg success`;
        hint("resume", `Last session: ${scoreStr} | trend=${metrics.trend} (${metrics.total_sessions} sessions)`);
        if (metrics.stagnation_count > 0) {
            hint("resume", `Stagnation: ${metrics.stagnation_count} session(s) without improvement`);
        }
        // Report dimension weaknesses
        if (lastEntry?.dimension_averages) {
            const dims = lastEntry.dimension_averages;
            const weak = [];
            if (dims.tool_success < 0.7)
                weak.push(`tool_success=${dims.tool_success}`);
            if (dims.output_quality < 0.7)
                weak.push(`output_quality=${dims.output_quality}`);
            if (weak.length > 0) {
                hint("resume", `Weak dimensions: ${weak.join(", ")}`);
            }
        }
        // Session handoff: last error context (#10)
        if (metrics.last_error_context) {
            hint("resume", `Last errors: ${metrics.last_error_context}`);
        }
        // Skill attribution highlights (#6)
        if (metrics.skill_attribution) {
            const effective = Object.values(metrics.skill_attribution)
                .filter(a => a.sessions_active >= 2 && a.avg_score_with > a.avg_score_without + 0.02);
            if (effective.length > 0) {
                hint("resume", `Top skills: ${effective.map(s => s.skill_name).join(", ")}`);
            }
        }
    }
    // 3. Evolved skills
    const evolved = listDirs(EVOLVED_DIR);
    if (evolved.length > 0)
        hint("resume", `Evolved skills: ${evolved.join(", ")}`);
    // 4. Cold-start presets (#1) — apply on first session when no evolved skills exist
    if (evolved.length === 0 && metrics.total_sessions === 0) {
        const stacks = Object.entries(STACK_FILES)
            .filter(([f]) => existsSync(join(CWD, f)))
            .map(([, s]) => s);
        if (stacks.length > 0) {
            const applied = applyColdStartPresets(stacks);
            if (applied > 0) {
                hint("resume", `Cold-start: applied ${applied} preset skill(s) for ${stacks.join(", ")}`);
            }
        }
    }
    // 5. Memory
    const memFiles = listFiles(MEMORY_DIR, ".md");
    if (memFiles.length > 0)
        hint("resume", `Memory: ${memFiles.length} file(s)`);
    // 6. Stack detection
    const stacks = Object.entries(STACK_FILES)
        .filter(([f]) => existsSync(join(CWD, f)))
        .map(([, s]) => s);
    if (stacks.length > 0)
        hint("resume", `Stack: ${stacks.join(", ")}`);
    // 7. Team
    const teamAgentsDir = join(TEAM_DIR, "agents");
    const teamAgents = listFiles(teamAgentsDir, ".md");
    if (teamAgents.length > 0) {
        hint("resume", `Team: ${teamAgents.map(a => a.replace(".md", "")).join(", ")}`);
    }
    // 8. Cross-project hints (#2)
    const crossHints = getCrossProjectHints();
    for (const h of crossHints) {
        hint("resume", h);
    }
});
