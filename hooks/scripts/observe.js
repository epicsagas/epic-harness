#!/usr/bin/env node
/**
 * Ring 3: observe — Pre/PostToolUse(*)
 *
 * Multi-dimensional tool usage recording.
 * Adapted from A-Evolve's Observation → BatchAnalysis pipeline.
 *
 * Scoring: 0.5 * tool_success + 0.3 * output_quality + 0.2 * execution_cost
 * Per-tool-type scoring criteria (Bash ≠ Edit ≠ Read).
 * Failure classification: 10 categories instead of binary error/success.
 */
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { execFile } from "node:child_process";
import { runHook, harnessExists, ensureDir, appendJsonl, classifyTool, classifyFailure, extractFileExt, OBS_DIR, DISPATCH_DIR, today, now, SCORE_WEIGHTS, getSessionId, } from "./common.js";
// ── Sequence counter (O(1) via byte counting) ──────
function getNextSequenceId(sessionFile) {
    if (!existsSync(sessionFile))
        return 1;
    // Count newlines without reading entire file into memory as string split
    const buf = readFileSync(sessionFile);
    let count = 0;
    for (let i = 0; i < buf.length; i++) {
        if (buf[i] === 0x0a)
            count++; // '\n'
    }
    return count + 1;
}
// ── Per-tool scoring ────────────────────────────────
// Commands that normally produce no output on success
const SILENT_OK_CMDS = /^\s*(mkdir|cp|mv|rm|chmod|chown|ln|touch|git\s+(add|checkout|switch|branch|stash|tag|remote)|cd|export|source|tsc\s+--noEmit)\b/;
function scoreBash(output, _stderr, command) {
    const failure = classifyFailure(output);
    const toolSuccess = failure === null ? 1 : 0;
    // output_quality: context-aware empty-output handling
    let quality = 1.0;
    const isEmpty = !output || output.trim().length === 0;
    if (isEmpty && command && SILENT_OK_CMDS.test(command)) {
        quality = 1.0; // silent success is normal for these commands
    }
    else if (isEmpty) {
        quality = 0.7; // mild penalty — might be unexpected silence
    }
    if (/warning|Warning|WARN/i.test(output) && !/\bWARN(ING)?\b.*deprecat/i.test(output)) {
        quality = Math.max(quality - 0.3, 0);
    }
    // execution_cost: huge output = likely a dump, not efficient
    const len = output.length;
    const cost = len > 50000 ? 0.3 : len > 20000 ? 0.6 : 1.0;
    return { tool_success: toolSuccess, output_quality: quality, execution_cost: cost };
}
function scoreEdit(output, prevAction, currentAction) {
    const failure = classifyFailure(output);
    const toolSuccess = failure === null ? 1 : 0;
    let quality = 1.0;
    if (/no changes|file not found/i.test(output))
        quality = 0.3;
    // Re-editing same file = correction signal
    if (prevAction && currentAction && prevAction === currentAction)
        quality = Math.min(quality, 0.7);
    return { tool_success: toolSuccess, output_quality: quality, execution_cost: 1.0 };
}
function scoreWrite(output) {
    const failure = classifyFailure(output);
    return {
        tool_success: failure === null ? 1 : 0,
        output_quality: failure === null ? 1.0 : 0.0,
        execution_cost: 1.0,
    };
}
function scoreReadSearch(output) {
    const hasResults = output && output.trim().length > 0 && !/no matches|0 results/i.test(output);
    return {
        tool_success: hasResults ? 1 : 0,
        output_quality: hasResults ? 1.0 : 0.5,
        execution_cost: 1.0,
    };
}
function computeScore(dims) {
    return Math.round((SCORE_WEIGHTS.success * dims.tool_success + SCORE_WEIGHTS.quality * dims.output_quality + SCORE_WEIGHTS.cost * dims.execution_cost) * 1000) / 1000;
}
// ── Previous action tracker (read last line only) ───
function getLastAction(sessionFile) {
    if (!existsSync(sessionFile))
        return null;
    // Read last ~1KB to find the last line — avoids reading entire file
    const fd = readFileSync(sessionFile);
    const tail = fd.subarray(Math.max(0, fd.length - 1024)).toString("utf8");
    const lines = tail.split("\n").filter(Boolean);
    if (lines.length === 0)
        return null;
    try {
        const last = JSON.parse(lines[lines.length - 1]);
        return last.action ?? null;
    }
    catch {
        return null;
    }
}
// ── Memory auto-record ──────────────────────────────
const MEM_KEYWORDS = ['decision', 'architecture', 'pattern', 'chose', 'decided', 'approach'];
/** Extract a short title from arbitrary text (first sentence or first 80 chars). */
function extractTitle(text) {
    const first = text.trim().split(/[\n.!?]/)[0].trim();
    return first.slice(0, 80) || 'Untitled decision';
}
/** Derive a project slug from cwd (last path segment, lowercase, hyphens). */
function detectProject() {
    return (process.env.PWD ?? process.cwd())
        .split('/')
        .filter(Boolean)
        .pop()
        ?.toLowerCase()
        .replace(/[^a-z0-9]+/g, '-') ?? 'unknown';
}
/** Mask common secret patterns in arbitrary text before storing. */
function maskBodySecrets(text) {
    return text
        .replace(/sk-[A-Za-z0-9\-_]{20,}/g, '<REDACTED>')
        .replace(/(?:api[_-]?key|token|secret|password|passwd|pwd)\s*[:=]\s*\S+/gi, (m) => m.replace(/[:=]\s*\S+/, '=<REDACTED>'))
        .replace(/Bearer\s+\S+/gi, 'Bearer <REDACTED>');
}
/**
 * Fire-and-forget: if the tool output or assistant message contains a decision
 * keyword, record it to the unified memory store via `epic-harness mem add`.
 * Failures are silently ignored — this must never block the main hook flow.
 */
function maybeRecordMemory(input, resolvedOutputText) {
    const toolName = (input.tool_name ?? '').toLowerCase();
    if (toolName !== 'write' && toolName !== 'edit')
        return;
    // Skip sensitive file paths — never record credentials or key files
    const filePath = input.tool_input?.file_path || '';
    const SENSITIVE_PATHS = /\.env|secrets?|credentials?|\.pem|\.key|id_rsa|\.pfx|\.p12/i;
    if (SENSITIVE_PATHS.test(filePath))
        return;
    const text = resolvedOutputText + (input.assistant_message ?? '');
    if (!MEM_KEYWORDS.some(k => text.toLowerCase().includes(k)))
        return;
    const title = extractTitle(text);
    const body = maskBodySecrets(text.slice(0, 500));
    const project = detectProject();
    execFile('epic-harness', [
        'mem', 'add',
        '--title', title,
        '--type', 'decision',
        '--project', project,
        '--agent', 'claude-code',
        '--body', body,
    ], { timeout: 2000 }, () => { /* fire-and-forget */ });
}
// ── Main hook ───────────────────────────────────────
/** Log a dispatch event (called externally or via skill triggers) */
export function logDispatch(signal, skills, contextHint) {
    ensureDir(DISPATCH_DIR);
    const record = {
        timestamp: now(),
        trigger_signal: signal,
        selected_skills: skills,
        context_hint: contextHint,
    };
    appendJsonl(join(DISPATCH_DIR, `dispatch_${today()}.jsonl`), record);
}
runHook((input) => {
    if (!harnessExists())
        return;
    ensureDir(OBS_DIR);
    // Session-ID based file separation for concurrent session safety (#3)
    const sessionFile = join(OBS_DIR, `session_${getSessionId()}.jsonl`);
    const toolCategory = classifyTool(input.tool_name ?? "");
    const record = {
        timestamp: now(),
        tool: input.tool_name ?? "unknown",
        tool_category: toolCategory,
        action: null,
        result: null,
        score: null,
        dimensions: null,
        failure_category: null,
        file_ext: extractFileExt(input.tool_input),
        sequence_id: getNextSequenceId(sessionFile),
    };
    // Pre-tool: capture action
    if (input.tool_input) {
        record.action = input.tool_input.command
            ?? input.tool_input.file_path
            ?? JSON.stringify(input.tool_input).slice(0, 200);
    }
    // Resolve tool output: prefer tool_output (structured) then tool_result (Claude Code actual field)
    const resolvedOutput = input.tool_output
        ? { output: input.tool_output.output ?? "", stderr: input.tool_output.stderr ?? "" }
        : input.tool_result != null
            ? typeof input.tool_result === "string"
                ? { output: input.tool_result, stderr: "" }
                : typeof input.tool_result === "object" && input.tool_result !== null
                    ? {
                        output: input.tool_result.output ?? "",
                        stderr: input.tool_result.stderr ?? "",
                    }
                    : { output: String(input.tool_result), stderr: "" }
            : null;
    // Post-tool: multi-dimensional scoring
    if (resolvedOutput) {
        const output = resolvedOutput.output;
        const stderr = resolvedOutput.stderr;
        const combined = output + "\n" + stderr;
        // Classify failure
        record.failure_category = classifyFailure(combined);
        record.result = record.failure_category === null ? "success" : "error";
        // Per-tool-type scoring
        let dims;
        switch (toolCategory) {
            case "bash":
                dims = scoreBash(combined, stderr, input.tool_input?.command ?? "");
                break;
            case "edit":
                dims = scoreEdit(combined, getLastAction(sessionFile), record.action);
                break;
            case "write":
                dims = scoreWrite(combined);
                break;
            case "read":
            case "glob":
            case "grep":
                dims = scoreReadSearch(combined);
                break;
            default:
                dims = { tool_success: record.failure_category === null ? 1 : 0, output_quality: 1.0, execution_cost: 1.0 };
        }
        record.dimensions = dims;
        record.score = computeScore(dims);
        // Error snippet for analysis
        if (record.failure_category !== null) {
            record.error_snippet = combined.slice(0, 500);
        }
    }
    appendJsonl(sessionFile, record);
    // Auto-record architectural decisions/patterns to unified memory (fire-and-forget)
    if (resolvedOutput) {
        maybeRecordMemory(input, resolvedOutput.output);
    }
});
