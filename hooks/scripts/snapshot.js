#!/usr/bin/env node
/**
 * Ring 0: snapshot — PreCompact
 *
 * Save working state before context compaction.
 * Includes observation summary so eval context survives compaction.
 */
import { existsSync } from "node:fs";
import { writeFileSync } from "node:fs";
import { join } from "node:path";
import { runHook, hint, harnessExists, ensureDir, listFiles, SESSIONS_DIR, OBS_DIR, today, readJsonlSafe, } from "./common.js";
export function getObsSummary(obsDir = OBS_DIR) {
    if (!existsSync(obsDir))
        return null;
    // Find all session files for today (supports session-ID based naming #3)
    const todayStr = today();
    const todayFiles = listFiles(obsDir, ".jsonl").filter(f => f.includes(todayStr));
    // Merge all records from today's sessions
    let records = [];
    for (const f of todayFiles) {
        records = records.concat(readJsonlSafe(join(obsDir, f)));
    }
    // Fallback: if no today files, try any session file
    if (records.length === 0) {
        const allFiles = listFiles(obsDir, ".jsonl").sort();
        if (allFiles.length > 0) {
            records = readJsonlSafe(join(obsDir, allFiles[allFiles.length - 1]));
        }
    }
    if (records.length === 0)
        return null;
    const scored = records.filter(r => r.score !== null && r.score !== undefined);
    const errors = scored.filter(r => r.result === "error");
    const total = scored.length;
    const successRate = total > 0 ? ((total - errors.length) / total * 100).toFixed(0) : "100";
    const avgScore = total > 0
        ? (scored.reduce((s, r) => s + r.score, 0) / total).toFixed(2)
        : "1.00";
    // Top error categories
    const errorCats = {};
    for (const e of errors) {
        const cat = e.failure_category ?? "unknown";
        errorCats[cat] = (errorCats[cat] ?? 0) + 1;
    }
    const topErrors = Object.entries(errorCats).sort((a, b) => b[1] - a[1]).slice(0, 3);
    const errorStr = topErrors.length > 0
        ? `, errors=[${topErrors.map(([c, n]) => `${c}:${n}`).join(",")}]`
        : "";
    return `${records.length} obs, ${successRate}% success, avg=${avgScore}${errorStr}`;
}
const isMain = import.meta.url === `file://${process.argv[1]}`;
if (isMain)
    runHook((input) => {
        if (!harnessExists())
            return;
        ensureDir(SESSIONS_DIR);
        const obsSummary = getObsSummary();
        const snapshot = {
            timestamp: new Date().toISOString(),
            type: "pre-compact",
            summary: obsSummary
                ? `${input.conversation_summary ?? "Context compaction"}. Eval: ${obsSummary}`
                : input.conversation_summary ?? "Context compaction triggered",
            pending_tasks: input.pending_tasks ?? [],
            context_usage: input.context_usage ?? null,
        };
        const filename = `snapshot_${Date.now()}.json`;
        writeFileSync(join(SESSIONS_DIR, filename), JSON.stringify(snapshot, null, 2));
        hint("snapshot", `Saved: ${filename}${obsSummary ? ` (${obsSummary})` : ""}`);
    });
