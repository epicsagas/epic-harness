#!/usr/bin/env node
/**
 * Ring 0: polish — PostToolUse(Edit)
 *
 * Auto-format + typecheck after file edits.
 * Feeds failures back to observe pipeline (#9).
 */
import { execSync, execFileSync } from "node:child_process";
import { existsSync } from "node:fs";
import { extname, basename, join, isAbsolute } from "node:path";
import { runHook, hint, harnessExists, ensureDir, appendJsonl, CWD, OBS_DIR, now, getSessionId, SCORE_WEIGHTS, } from "./common.js";
function tryExec(cmd, opts) {
    try {
        return execSync(cmd, { cwd: CWD, timeout: 15_000, stdio: "pipe" }).toString();
    }
    catch (e) {
        if (!opts?.silent) {
            const err = e;
            return err.stdout?.toString() ?? null;
        }
        return null;
    }
}
function tryExecFile(bin, args, opts) {
    try {
        return execFileSync(bin, args, { cwd: CWD, timeout: 15_000, stdio: "pipe" }).toString();
    }
    catch (e) {
        if (!opts?.silent) {
            const err = e;
            return err.stdout?.toString() ?? null;
        }
        return null;
    }
}
function isValidFilePath(p) {
    return typeof p === "string" && p.length > 0 && !p.includes("\0") && isAbsolute(p);
}
/** Record polish result as observation for the eval loop */
function feedbackToObserve(filePath, formatter, success, errorSnippet) {
    if (!harnessExists())
        return;
    ensureDir(OBS_DIR);
    const dims = {
        tool_success: success ? 1 : 0,
        output_quality: success ? 1.0 : 0.3,
        execution_cost: 1.0,
    };
    const score = Math.round((SCORE_WEIGHTS.success * dims.tool_success +
        SCORE_WEIGHTS.quality * dims.output_quality +
        SCORE_WEIGHTS.cost * dims.execution_cost) * 1000) / 1000;
    const record = {
        timestamp: now(),
        tool: "polish",
        tool_category: "other",
        action: `${formatter}:${basename(filePath)}`,
        result: success ? "success" : "error",
        score,
        dimensions: dims,
        failure_category: success ? null : (formatter === "tsc" ? "build_fail" : "lint_fail"),
        error_snippet: errorSnippet?.slice(0, 500),
        file_ext: extname(filePath) || undefined,
    };
    const sessionFile = join(OBS_DIR, `session_${getSessionId()}.jsonl`);
    appendJsonl(sessionFile, record);
}
function formatJS(filePath) {
    if (!isValidFilePath(filePath))
        return;
    const hasBiome = existsSync(join(CWD, "biome.json")) || existsSync(join(CWD, "biome.jsonc"));
    const hasPrettier = existsSync(join(CWD, ".prettierrc")) || existsSync(join(CWD, ".prettierrc.json"));
    if (hasBiome) {
        const result = tryExecFile("npx", ["biome", "format", "--write", filePath], { silent: true });
        if (result) {
            hint("polish", `Biome: ${basename(filePath)}`);
            feedbackToObserve(filePath, "biome", true);
        }
    }
    else if (hasPrettier) {
        const result = tryExecFile("npx", ["prettier", "--write", filePath], { silent: true });
        if (result) {
            hint("polish", `Prettier: ${basename(filePath)}`);
            feedbackToObserve(filePath, "prettier", true);
        }
    }
}
function checkTS(filePath) {
    if (!existsSync(join(CWD, "tsconfig.json")))
        return;
    const out = tryExec(`npx tsc --noEmit --pretty false 2>&1 | head -10`);
    if (out && /error TS/.test(out)) {
        hint("polish", `TS errors:\n${out.slice(0, 500)}`);
        feedbackToObserve(filePath, "tsc", false, out.slice(0, 500));
    }
    else {
        feedbackToObserve(filePath, "tsc", true);
    }
}
function formatPython(filePath) {
    if (!isValidFilePath(filePath))
        return;
    if (tryExecFile("ruff", ["format", filePath], { silent: true })
        || tryExecFile("black", [filePath], { silent: true })) {
        hint("polish", `Formatted: ${basename(filePath)}`);
        feedbackToObserve(filePath, "ruff/black", true);
    }
}
function formatGo(filePath) {
    if (!isValidFilePath(filePath))
        return;
    if (tryExecFile("gofmt", ["-w", filePath], { silent: true })) {
        hint("polish", `gofmt: ${basename(filePath)}`);
        feedbackToObserve(filePath, "gofmt", true);
    }
}
runHook((input) => {
    const filePath = input.tool_input?.file_path || "";
    if (!filePath)
        return;
    const ext = extname(filePath);
    if ([".js", ".jsx", ".ts", ".tsx"].includes(ext)) {
        formatJS(filePath);
        if ([".ts", ".tsx"].includes(ext))
            checkTS(filePath);
    }
    else if (ext === ".py") {
        formatPython(filePath);
    }
    else if (ext === ".go") {
        formatGo(filePath);
    }
});
