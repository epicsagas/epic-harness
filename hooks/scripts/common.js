/**
 * Shared utilities for all hook scripts.
 */
import { readFileSync, appendFileSync, existsSync, mkdirSync, readdirSync, statSync, rmSync, copyFileSync } from "node:fs";
import { join, extname } from "node:path";
// ── Paths ────────────────────────────────────────────
export const CWD = process.cwd();
export const HARNESS_DIR = join(CWD, ".harness");
export const OBS_DIR = join(HARNESS_DIR, "obs");
export const SESSIONS_DIR = join(HARNESS_DIR, "sessions");
export const MEMORY_DIR = join(HARNESS_DIR, "memory");
export const EVOLVED_DIR = join(HARNESS_DIR, "evolved");
export const TEAM_DIR = join(HARNESS_DIR, "team");
export const EVOLUTION_FILE = join(HARNESS_DIR, "evolution.jsonl");
export const METRICS_FILE = join(HARNESS_DIR, "metrics.json");
export const MAX_EVOLVED_SKILLS = 10;
export const STAGNATION_LIMIT = 3; // sessions before rollback
export const IMPROVEMENT_THRESHOLD = 0.05; // 5% improvement required
export const EVOLVED_BACKUP_DIR = join(HARNESS_DIR, "evolved_backup");
export const DISPATCH_DIR = join(HARNESS_DIR, "dispatch");
export const GUARD_RULES_FILE = join(HARNESS_DIR, "guard-rules.yaml");
export const PRESETS_DIR = join(HARNESS_DIR, "presets");
export const GLOBAL_HARNESS_DIR = join(process.env.HOME ?? "~", ".harness-global");
export const GLOBAL_PATTERNS_FILE = join(GLOBAL_HARNESS_DIR, "patterns.jsonl");
// ── Score Weights ───────────────────────────────────
export const SCORE_WEIGHTS = { success: 0.5, quality: 0.3, cost: 0.2 };
// ── Pattern Detection Thresholds ────────────────────
export const REPEATED_ERROR_MIN = 3; // consecutive same-error count
export const FTB_LOOKAHEAD = 3; // fix_then_break: steps to look ahead
export const FTB_MIN_CYCLES = 2; // fix_then_break: min occurrences per file
export const DEBUG_LOOP_MIN = 5; // long_debug_loop: consecutive ops on same file
export const THRASH_MIN_EDITS = 3; // thrashing: min edits on same file
export const THRASH_MIN_ERRORS = 3; // thrashing: min errors on same file
// ── Skill Seeding Thresholds ────────────────────────
export const WEAK_TOOL_RATE = 0.6; // seed if success rate below this
export const WEAK_TOOL_MIN_OBS = 5; // seed only with this many observations
export const WEAK_EXT_RATE = 0.5; // seed if ext success rate below this
export const WEAK_EXT_MIN_OBS = 3; // seed only with this many ext observations
export const HIGH_FREQ_ERROR_MIN = 5; // seed from error category with this many occurrences
// ── Session ID ─────────────────────────────────────
let _sessionId = null;
export function getSessionId() {
    if (!_sessionId) {
        _sessionId = `${today()}_${process.pid}_${Math.random().toString(36).slice(2, 8)}`;
    }
    return _sessionId;
}
// ── Cross-project learning ─────────────────────────
export const CROSS_PROJECT_OPT_IN_FILE = join(HARNESS_DIR, ".cross-project-enabled");
// ── Failure Classification ──────────────────────────
const FAILURE_RULES = [
    { pattern: /TypeError|type error/i, category: "type_error" },
    { pattern: /SyntaxError|Unexpected token|Parse error/i, category: "syntax_error" },
    { pattern: /FAIL(?:ED|ING)?[\s:]|test.*fail|AssertionError|assert\.\w+/i, category: "test_fail" },
    { pattern: /\blint\b.*(?:error|fail)|eslint.*error|biome.*error|oxlint.*error/i, category: "lint_fail" },
    { pattern: /build.*fail|tsc.*error|error TS\d+|compilation.*fail/i, category: "build_fail" },
    { pattern: /EACCES|permission denied/i, category: "permission_denied" },
    { pattern: /timeout|ETIMEDOUT|timed out/i, category: "timeout" },
    { pattern: /ENOENT|No such file or directory/i, category: "not_found" },
    // catch-all: require "Error:" or "error:" with colon, or stack-trace-like patterns
    // avoids matching words like "error-handler", "ErrorBoundary" in normal output
    { pattern: /(?:^|\n)\s*(?:Error|error|ERROR):|Traceback|at [\w.]+\s*\(|Unhandled|uncaught exception/m, category: "runtime_error" },
];
export function classifyFailure(output) {
    if (!output)
        return null;
    const sample = output.slice(0, 2000); // scan first 2KB only
    for (const rule of FAILURE_RULES) {
        if (rule.pattern.test(sample))
            return rule.category;
    }
    return null;
}
export function classifyTool(toolName) {
    const name = (toolName ?? "").toLowerCase();
    if (name === "bash")
        return "bash";
    if (name === "edit")
        return "edit";
    if (name === "write")
        return "write";
    if (name === "read")
        return "read";
    if (name === "glob")
        return "glob";
    if (name === "grep")
        return "grep";
    return "other";
}
export function extractFileExt(input) {
    if (!input)
        return undefined;
    const filePath = (input.file_path ?? input.path ?? "");
    if (filePath) {
        const ext = extname(filePath);
        return ext || undefined;
    }
    // Try to extract from bash command
    const cmd = (input.command ?? "");
    const match = cmd.match(/\.(ts|js|py|go|rs|java|c|cpp|rb|sh|json|yaml|yml|md|css|html|tsx|jsx)\b/);
    return match ? `.${match[1]}` : undefined;
}
// ── Helpers ──────────────────────────────────────────
export function harnessExists() {
    return existsSync(HARNESS_DIR);
}
export function ensureDir(dir) {
    mkdirSync(dir, { recursive: true });
}
export function readJsonSafe(filePath, fallback) {
    if (!existsSync(filePath))
        return fallback;
    try {
        return JSON.parse(readFileSync(filePath, "utf8"));
    }
    catch {
        return fallback;
    }
}
export function readJsonlSafe(filePath) {
    if (!existsSync(filePath))
        return [];
    return readFileSync(filePath, "utf8")
        .split("\n")
        .filter(Boolean)
        .map(line => { try {
        return JSON.parse(line);
    }
    catch {
        return null;
    } })
        .filter((x) => x !== null);
}
export function appendJsonl(filePath, record) {
    appendFileSync(filePath, JSON.stringify(record) + "\n");
}
export function listDirs(dir) {
    if (!existsSync(dir))
        return [];
    return readdirSync(dir).filter(d => statSync(join(dir, d)).isDirectory());
}
export function listFiles(dir, ext) {
    if (!existsSync(dir))
        return [];
    return readdirSync(dir).filter(f => f.endsWith(ext));
}
export function today() {
    return new Date().toISOString().slice(0, 10).replace(/-/g, "");
}
export function now() {
    return new Date().toISOString();
}
/** stderr message (visible to user as hint) */
export function hint(tag, msg) {
    process.stderr.write(`[${tag}] ${msg}\n`);
}
/** raw stderr line (no tag prefix) — for banners/dividers */
export function raw(line) {
    process.stderr.write(`${line}\n`);
}
/** Read stdin, run handler, write stdout (passthrough) */
export function runHook(handler) {
    let data = "";
    process.stdin.on("data", (c) => { data += c.toString(); });
    process.stdin.on("end", () => {
        try {
            const input = data ? JSON.parse(data) : {};
            handler(input);
        }
        catch { /* silent */ }
        process.stdout.write(data);
    });
}
/** Read stdin, run handler that can BLOCK (exit 2) */
export function runGuardHook(handler) {
    let data = "";
    process.stdin.on("data", (c) => { data += c.toString(); });
    process.stdin.on("end", () => {
        try {
            const input = data ? JSON.parse(data) : {};
            handler(input);
        }
        catch { /* silent */ }
        process.stdout.write(data);
    });
}
/** Recursively copy a directory (no external deps) */
export function copyDirSync(src, dest) {
    if (!existsSync(src))
        return;
    ensureDir(dest);
    for (const entry of readdirSync(src)) {
        const srcPath = join(src, entry);
        const destPath = join(dest, entry);
        if (statSync(srcPath).isDirectory()) {
            copyDirSync(srcPath, destPath);
        }
        else {
            copyFileSync(srcPath, destPath);
        }
    }
}
/** Remove directory recursively */
export function rmDirSync(dir) {
    if (existsSync(dir))
        rmSync(dir, { recursive: true, force: true });
}
/** Default Metrics object */
export function defaultMetrics() {
    return {
        total_sessions: 0,
        avg_success_rate: 0,
        total_evolved_skills: 0,
        score_history: [],
        best_score: 0,
        best_session: "",
        trend: "stable",
        stagnation_count: 0,
        skill_attribution: {},
    };
}
// ── Function name extraction ───────────────────────────
const FUNC_PATTERNS = [
    /(?:function|async function)\s+(\w+)/, // function handleSubmit()
    /(?:const|let|var)\s+(\w+)\s*=\s*(?:async\s*)?\(/, // const processData = async (
    /def\s+(\w+)\s*\(/, // def calculate_total(
    /func\s+(\w+)\s*\(/, // func main(
    /at\s+(\w+)\s*[(/]/, // at validateInput (
    /\.(\w+)\s+is not a function/, // .fetchUser is not a function
    /method\s+'(\w+)'/, // method 'doStuff'
];
const FUNC_KEYWORDS = new Set([
    "function", "async", "const", "let", "var", "def", "func", "at", "from",
    "import", "export", "return", "if", "else", "for", "while", "new", "class",
    "undefined", "null", "true", "false", "Error", "error", "TypeError",
]);
export function extractFunctionName(text) {
    if (!text)
        return null;
    for (const pattern of FUNC_PATTERNS) {
        const m = text.match(pattern);
        if (m && m[1] && m[1].length <= 80 && !FUNC_KEYWORDS.has(m[1])) {
            return m[1];
        }
    }
    return null;
}
// ── Frontmatter parsing ──────────────────────────────
export function parseFrontmatter(content) {
    if (!content.startsWith("---"))
        return null;
    const endIdx = content.indexOf("---", 3);
    if (endIdx < 0)
        return null;
    const block = content.slice(3, endIdx).trim();
    let name = "";
    let description = "";
    for (const line of block.split("\n")) {
        const nameMatch = line.match(/^name:\s*(.+)/);
        const descMatch = line.match(/^description:\s*"?(.+?)"?\s*$/);
        if (nameMatch)
            name = nameMatch[1].trim();
        if (descMatch)
            description = descMatch[1].trim();
    }
    if (!name || !description)
        return null;
    return { name, description };
}
// ── Evolved skill validation ────────────────────────
export function validateEvolvedSkill(content) {
    const errors = [];
    const fm = parseFrontmatter(content);
    if (!fm) {
        errors.push("invalid_frontmatter");
        return { valid: false, errors, frontmatter: { name: "", description: "" } };
    }
    if (fm.name.length < 2)
        errors.push("name_too_short");
    if (fm.description.length < 10)
        errors.push("description_too_short");
    const body = content.replace(/---[\s\S]*?---/, "").trim();
    if (body.length < 20)
        errors.push("body_too_short");
    if (!/#\s+/.test(body))
        errors.push("missing_heading");
    if (!/##\s+(Remediation|Process|Red Flags)/i.test(body))
        errors.push("missing_actionable_section");
    return { valid: errors.length === 0, errors, frontmatter: fm };
}
/** Simple YAML-like key:value parser for guard-rules.yaml (no external deps) */
export function parseSimpleYaml(content) {
    const result = { blocked: [], warned: [] };
    let section = null;
    for (const line of content.split("\n")) {
        const trimmed = line.trim();
        if (trimmed === "blocked:") {
            section = "blocked";
            continue;
        }
        if (trimmed === "warned:") {
            section = "warned";
            continue;
        }
        if (!section || !trimmed.startsWith("- "))
            continue;
        const entry = trimmed.slice(2).trim();
        // Format: "pattern: <regex> | msg: <message>"
        const match = entry.match(/^pattern:\s*(.+?)\s*\|\s*msg:\s*(.+)$/);
        if (match) {
            try {
                result[section].push({ pattern: new RegExp(match[1].trim()), msg: match[2].trim() });
            }
            catch { /* invalid regex, skip */ }
        }
    }
    return result;
}
/** Hash a string to short hex (for error dedup) */
export function hashString(str) {
    let hash = 0;
    for (let i = 0; i < str.length; i++) {
        const c = str.charCodeAt(i);
        hash = ((hash << 5) - hash) + c;
        hash |= 0;
    }
    return (hash >>> 0).toString(16).padStart(8, "0");
}
/** Normalize error message for dedup (strip line numbers, paths, timestamps) */
export function normalizeError(snippet) {
    return snippet
        .replace(/\d{4}-\d{2}-\d{2}[T ]\d{2}:\d{2}:\d{2}[.\dZ]*/g, "") // timestamps
        .replace(/:\d+:\d+/g, ":L:C") // line:col
        .replace(/\/[\w./-]+\//g, "/PATH/") // abs paths
        .replace(/\s+/g, " ")
        .trim()
        .slice(0, 200);
}
