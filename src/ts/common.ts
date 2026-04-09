/**
 * Shared utilities for all hook scripts.
 */
import { readFileSync, writeFileSync, appendFileSync, existsSync, mkdirSync, readdirSync, statSync, rmSync, copyFileSync } from "node:fs";
import { join, extname, basename } from "node:path";

// ── Failure / Tool Categories ───────────────────────

export type FailureCategory =
  | "type_error"
  | "syntax_error"
  | "test_fail"
  | "lint_fail"
  | "build_fail"
  | "permission_denied"
  | "timeout"
  | "not_found"
  | "runtime_error";

export type ToolCategory = "bash" | "edit" | "write" | "read" | "glob" | "grep" | "other";

// ── Types ────────────────────────────────────────────

export interface HookInput {
  tool_name?: string;
  tool_input?: Record<string, unknown>;
  tool_output?: { output?: string; stderr?: string };
  conversation_summary?: string;
  pending_tasks?: string[];
  context_usage?: number;
}

export interface ScoreDimensions {
  tool_success: number;     // 0 or 1 — did the tool itself succeed?
  output_quality: number;   // 0.0-1.0 — heuristic quality signals
  execution_cost: number;   // 0.0-1.0 — inverse proxy: 1.0=fast/cheap
}

export interface ObsRecord {
  timestamp: string;
  tool: string;
  tool_category: ToolCategory;
  action: string | null;
  result: "success" | "error" | null;
  score: number | null;
  dimensions: ScoreDimensions | null;
  failure_category: FailureCategory | null;
  error_snippet?: string;
  file_ext?: string;
  sequence_id?: number;
}

export interface SessionSnapshot {
  timestamp: string;
  type: string;
  summary: string;
  pending_tasks: string[];
  context_usage: number | null;
}

// ── Analysis Types (A-Evolve fusion) ────────────────

export interface ToolStats {
  tool_category: ToolCategory;
  total: number;
  successes: number;
  errors: number;
  avg_score: number;
  failure_categories: Record<string, number>;
}

export interface DetectedPattern {
  pattern_type: "repeated_same_error" | "fix_then_break" | "long_debug_loop" | "thrashing";
  description: string;
  count: number;
  involved_files: string[];
  suggested_remediation: string;
}

export interface SessionAnalysis {
  total_observations: number;
  success_rate: number;
  avg_score: number;
  score_distribution: Record<string, number>;
  per_tool_stats: Record<string, ToolStats>;
  per_error_stats: Record<string, number>;
  per_ext_stats: Record<string, { total: number; errors: number; success_rate: number }>;
  failure_patterns: DetectedPattern[];
  dimension_averages: ScoreDimensions;
}

export interface SessionScoreEntry {
  timestamp: string;
  success_rate: number;
  avg_score: number;
  observations: number;
  dimension_averages: ScoreDimensions;
}

export interface EvolutionRecord {
  timestamp: string;
  observations: number;
  success_rate: number;
  avg_score: number;
  error_patterns: Record<string, number>;
  failure_patterns: DetectedPattern[];
  skills_seeded: number;
  skills_rolled_back: number;
  total_evolved: number;
  analysis_summary: string;
}

export interface Metrics {
  total_sessions: number;
  avg_success_rate: number;
  total_evolved_skills: number;
  last_session?: string;
  score_history: SessionScoreEntry[];
  best_score: number;
  best_session: string;
  trend: "improving" | "stable" | "declining";
  stagnation_count: number;
  skill_attribution?: Record<string, SkillAttribution>;
  last_error_context?: string;
  last_pending_tasks?: string[];
}

export interface SkillAttribution {
  skill_name: string;
  sessions_active: number;
  avg_score_with: number;
  avg_score_without: number;
  first_seen: string;
}

export interface DispatchRecord {
  timestamp: string;
  trigger_signal: string;
  selected_skills: string[];
  context_hint: string;
}

export interface PolishFeedback {
  timestamp: string;
  file_path: string;
  formatter: string;
  typecheck_errors: number;
  failure_category: FailureCategory | null;
}

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
export const STAGNATION_LIMIT = 3;           // sessions before rollback
export const IMPROVEMENT_THRESHOLD = 0.05;   // 5% improvement required
export const EVOLVED_BACKUP_DIR = join(HARNESS_DIR, "evolved_backup");
export const DISPATCH_DIR = join(HARNESS_DIR, "dispatch");
export const GUARD_RULES_FILE = join(HARNESS_DIR, "guard-rules.yaml");
export const PRESETS_DIR = join(HARNESS_DIR, "presets");
export const GLOBAL_HARNESS_DIR = join(process.env.HOME ?? "~", ".harness-global");
export const GLOBAL_PATTERNS_FILE = join(GLOBAL_HARNESS_DIR, "patterns.jsonl");

// ── Score Weights ───────────────────────────────────
export const SCORE_WEIGHTS = { success: 0.5, quality: 0.3, cost: 0.2 };

// ── Pattern Detection Thresholds ────────────────────
export const REPEATED_ERROR_MIN = 3;          // consecutive same-error count
export const FTB_LOOKAHEAD = 3;               // fix_then_break: steps to look ahead
export const FTB_MIN_CYCLES = 2;              // fix_then_break: min occurrences per file
export const DEBUG_LOOP_MIN = 5;              // long_debug_loop: consecutive ops on same file
export const THRASH_MIN_EDITS = 3;            // thrashing: min edits on same file
export const THRASH_MIN_ERRORS = 3;           // thrashing: min errors on same file

// ── Skill Seeding Thresholds ────────────────────────
export const WEAK_TOOL_RATE = 0.6;            // seed if success rate below this
export const WEAK_TOOL_MIN_OBS = 5;           // seed only with this many observations
export const WEAK_EXT_RATE = 0.5;             // seed if ext success rate below this
export const WEAK_EXT_MIN_OBS = 3;            // seed only with this many ext observations
export const HIGH_FREQ_ERROR_MIN = 5;         // seed from error category with this many occurrences

// ── Session ID ─────────────────────────────────────
let _sessionId: string | null = null;
export function getSessionId(): string {
  if (!_sessionId) {
    _sessionId = `${today()}_${process.pid}_${Math.random().toString(36).slice(2, 8)}`;
  }
  return _sessionId;
}

// ── Cross-project learning ─────────────────────────
export const CROSS_PROJECT_OPT_IN_FILE = join(HARNESS_DIR, ".cross-project-enabled");

// ── Failure Classification ──────────────────────────

const FAILURE_RULES: { pattern: RegExp; category: FailureCategory }[] = [
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

export function classifyFailure(output: string): FailureCategory | null {
  if (!output) return null;
  const sample = output.slice(0, 2000); // scan first 2KB only
  for (const rule of FAILURE_RULES) {
    if (rule.pattern.test(sample)) return rule.category;
  }
  return null;
}

export function classifyTool(toolName: string): ToolCategory {
  const name = (toolName ?? "").toLowerCase();
  if (name === "bash") return "bash";
  if (name === "edit") return "edit";
  if (name === "write") return "write";
  if (name === "read") return "read";
  if (name === "glob") return "glob";
  if (name === "grep") return "grep";
  return "other";
}

export function extractFileExt(input: Record<string, unknown> | undefined): string | undefined {
  if (!input) return undefined;
  const filePath = (input.file_path ?? input.path ?? "") as string;
  if (filePath) {
    const ext = extname(filePath);
    return ext || undefined;
  }
  // Try to extract from bash command
  const cmd = (input.command ?? "") as string;
  const match = cmd.match(/\.(ts|js|py|go|rs|java|c|cpp|rb|sh|json|yaml|yml|md|css|html|tsx|jsx)\b/);
  return match ? `.${match[1]}` : undefined;
}

// ── Helpers ──────────────────────────────────────────

export function harnessExists(): boolean {
  return existsSync(HARNESS_DIR);
}

export function ensureDir(dir: string): void {
  mkdirSync(dir, { recursive: true });
}

export function readJsonSafe<T>(filePath: string, fallback: T): T {
  if (!existsSync(filePath)) return fallback;
  try {
    return JSON.parse(readFileSync(filePath, "utf8")) as T;
  } catch {
    return fallback;
  }
}

export function readJsonlSafe(filePath: string): Record<string, unknown>[] {
  if (!existsSync(filePath)) return [];
  return readFileSync(filePath, "utf8")
    .split("\n")
    .filter(Boolean)
    .map(line => { try { return JSON.parse(line); } catch { return null; } })
    .filter((x): x is Record<string, unknown> => x !== null);
}

export function appendJsonl(filePath: string, record: unknown): void {
  appendFileSync(filePath, JSON.stringify(record) + "\n");
}

export function listDirs(dir: string): string[] {
  if (!existsSync(dir)) return [];
  return readdirSync(dir).filter(d => statSync(join(dir, d)).isDirectory());
}

export function listFiles(dir: string, ext: string): string[] {
  if (!existsSync(dir)) return [];
  return readdirSync(dir).filter(f => f.endsWith(ext));
}

export function today(): string {
  return new Date().toISOString().slice(0, 10).replace(/-/g, "");
}

export function now(): string {
  return new Date().toISOString();
}

/** stderr message (visible to user as hint) */
export function hint(tag: string, msg: string): void {
  process.stderr.write(`[${tag}] ${msg}\n`);
}

/** raw stderr line (no tag prefix) — for banners/dividers */
export function raw(line: string): void {
  process.stderr.write(`${line}\n`);
}

/** Read stdin, run handler, write stdout (passthrough) */
export function runHook(handler: (input: HookInput) => void): void {
  let data = "";
  process.stdin.on("data", (c: Buffer) => { data += c.toString(); });
  process.stdin.on("end", () => {
    try {
      const input: HookInput = data ? JSON.parse(data) : {};
      handler(input);
    } catch { /* silent */ }
    process.stdout.write(data);
  });
}

/** Read stdin, run handler that can BLOCK (exit 2) */
export function runGuardHook(handler: (input: HookInput) => void): void {
  let data = "";
  process.stdin.on("data", (c: Buffer) => { data += c.toString(); });
  process.stdin.on("end", () => {
    try {
      const input: HookInput = data ? JSON.parse(data) : {};
      handler(input);
    } catch { /* silent */ }
    process.stdout.write(data);
  });
}

/** Recursively copy a directory (no external deps) */
export function copyDirSync(src: string, dest: string): void {
  if (!existsSync(src)) return;
  ensureDir(dest);
  for (const entry of readdirSync(src)) {
    const srcPath = join(src, entry);
    const destPath = join(dest, entry);
    if (statSync(srcPath).isDirectory()) {
      copyDirSync(srcPath, destPath);
    } else {
      copyFileSync(srcPath, destPath);
    }
  }
}

/** Remove directory recursively */
export function rmDirSync(dir: string): void {
  if (existsSync(dir)) rmSync(dir, { recursive: true, force: true });
}

/** Default Metrics object */
export function defaultMetrics(): Metrics {
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

const FUNC_PATTERNS: RegExp[] = [
  /(?:function|async function)\s+(\w+)/,           // function handleSubmit()
  /(?:const|let|var)\s+(\w+)\s*=\s*(?:async\s*)?\(/, // const processData = async (
  /def\s+(\w+)\s*\(/,                                // def calculate_total(
  /func\s+(\w+)\s*\(/,                               // func main(
  /at\s+(\w+)\s*[(/]/,                               // at validateInput (
  /\.(\w+)\s+is not a function/,                      // .fetchUser is not a function
  /method\s+'(\w+)'/,                                 // method 'doStuff'
];

const FUNC_KEYWORDS = new Set([
  "function", "async", "const", "let", "var", "def", "func", "at", "from",
  "import", "export", "return", "if", "else", "for", "while", "new", "class",
  "undefined", "null", "true", "false", "Error", "error", "TypeError",
]);

export function extractFunctionName(text: string): string | null {
  if (!text) return null;
  for (const pattern of FUNC_PATTERNS) {
    const m = text.match(pattern);
    if (m && m[1] && m[1].length <= 80 && !FUNC_KEYWORDS.has(m[1])) {
      return m[1];
    }
  }
  return null;
}

// ── Frontmatter parsing ──────────────────────────────

export function parseFrontmatter(content: string): { name: string; description: string } | null {
  if (!content.startsWith("---")) return null;
  const endIdx = content.indexOf("---", 3);
  if (endIdx < 0) return null;
  const block = content.slice(3, endIdx).trim();

  let name = "";
  let description = "";
  for (const line of block.split("\n")) {
    const nameMatch = line.match(/^name:\s*(.+)/);
    const descMatch = line.match(/^description:\s*"?(.+?)"?\s*$/);
    if (nameMatch) name = nameMatch[1].trim();
    if (descMatch) description = descMatch[1].trim();
  }

  if (!name || !description) return null;
  return { name, description };
}

// ── Evolved skill validation ────────────────────────

export function validateEvolvedSkill(content: string): { valid: boolean; errors: string[]; frontmatter: { name: string; description: string } } {
  const errors: string[] = [];
  const fm = parseFrontmatter(content);
  if (!fm) {
    errors.push("invalid_frontmatter");
    return { valid: false, errors, frontmatter: { name: "", description: "" } };
  }
  if (fm.name.length < 2) errors.push("name_too_short");
  if (fm.description.length < 10) errors.push("description_too_short");

  const body = content.replace(/---[\s\S]*?---/, "").trim();
  if (body.length < 20) errors.push("body_too_short");
  if (!/#\s+/.test(body)) errors.push("missing_heading");
  if (!/##\s+(Remediation|Process|Red Flags)/i.test(body)) errors.push("missing_actionable_section");

  return { valid: errors.length === 0, errors, frontmatter: fm };
}

// ── Guard pattern validation (ReDoS prevention) ─────

/**
 * Validates a user-defined regex pattern string for ReDoS safety before compilation.
 * Rejects patterns containing nested quantifiers that can cause catastrophic backtracking.
 * Returns true if the pattern is safe to compile and use, false otherwise.
 *
 * Rejected forms:
 *   - Group with quantifier followed by outer quantifier: (a+)+, (a*)*, (a+)*, (a*)+
 *   - Group with alternation containing quantified terms, followed by outer quantifier: (a|aa)+
 */
export function validateGuardPattern(pattern: string): boolean {
  // First check if the pattern is a valid regex at all
  try {
    new RegExp(pattern);
  } catch {
    return false;
  }

  // Reject patterns with nested quantifiers inside groups followed by an outer quantifier.
  // Matches: (...+...)+ / (...*...)+ / (...+...)* / (...*...)*
  // Note: alternation groups like (docker|podman)+ are NOT rejected — they are safe
  // when alternatives don't overlap. Only inner quantifiers trigger catastrophic backtracking.
  const nestedQuantifier = /\([^)]*[+*][^)]*\)[+*?]|\([^)]*[+*][^)]*\)\{[0-9,]+\}/;
  if (nestedQuantifier.test(pattern)) {
    return false;
  }

  return true;
}

export interface Rule {
  pattern: RegExp;
  msg: string;
}

/** Simple YAML-like key:value parser for guard-rules.yaml (no external deps) */
export function parseSimpleYaml(content: string): { blocked: Rule[]; warned: Rule[] } {
  const result: { blocked: Rule[]; warned: Rule[] } = { blocked: [], warned: [] };
  let section: "blocked" | "warned" | null = null;

  for (const line of content.split("\n")) {
    const trimmed = line.trim();
    if (trimmed === "blocked:") { section = "blocked"; continue; }
    if (trimmed === "warned:") { section = "warned"; continue; }
    if (!section || !trimmed.startsWith("- ")) continue;

    const entry = trimmed.slice(2).trim();
    // Format: "pattern: <regex> | msg: <message>"
    const match = entry.match(/^pattern:\s*(.+?)\s*\|\s*msg:\s*(.+)$/);
    if (match) {
      const rawPattern = match[1].trim();
      if (!validateGuardPattern(rawPattern)) continue; // skip ReDoS-risky patterns
      try {
        result[section].push({ pattern: new RegExp(rawPattern), msg: match[2].trim() });
      } catch { /* invalid regex, skip */ }
    }
  }
  return result;
}

/** Hash a string to short hex (for error dedup) */
export function hashString(str: string): string {
  let hash = 0;
  for (let i = 0; i < str.length; i++) {
    const c = str.charCodeAt(i);
    hash = ((hash << 5) - hash) + c;
    hash |= 0;
  }
  return (hash >>> 0).toString(16).padStart(8, "0");
}

/** Normalize error message for dedup (strip line numbers, paths, timestamps) */
export function normalizeError(snippet: string): string {
  return snippet
    .replace(/\d{4}-\d{2}-\d{2}[T ]\d{2}:\d{2}:\d{2}[.\dZ]*/g, "")  // timestamps
    .replace(/:\d+:\d+/g, ":L:C")                                       // line:col
    .replace(/\/[\w./-]+\//g, "/PATH/")                                  // abs paths
    .replace(/\s+/g, " ")
    .trim()
    .slice(0, 200);
}
