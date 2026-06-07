// Tests for helper logic used in Memory, Integrations, Settings, Hooks pages.

import { describe, it, expect } from 'vitest';
import type { HarnessMetrics, GraphData, IntegrationStatus } from '../lib/harness.js';

// ── Memory page helpers ────────────────────────────────────────────────────────

const typeColor: Record<string, string> = {
  project: '#6366f1', concept: '#22c55e', pattern: '#f59e0b',
  decision: '#ec4899', error: '#ef4444', session: '#64748b',
  resolution: '#06b6d4', default: '#94a3b8',
};

function nodeColor(type: string): string {
  return typeColor[type] ?? typeColor.default;
}

function nodeLabelTruncate(title: string): string {
  return title.length > 18 ? title.slice(0, 16) + '…' : title;
}

function nodeRadius(importance: number): number {
  return 8 + importance * 12;
}

const mockGraph: GraphData = {
  nodes: [
    { id: '1', title: 'epic-harness', type: 'project', tags: ['harness'], importance: 0.9, projects: ['epic-harness'], accessed_at: '' },
    { id: '2', title: 'harness-mem', type: 'concept', tags: ['memory'], importance: 0.8, projects: [], accessed_at: '' },
    { id: '3', title: 'a very long node title indeed', type: 'pattern', tags: [], importance: 0.5, projects: [], accessed_at: '' },
  ],
  edges: [
    { source: '1', target: '2', relation: 'contains', weight: 0.9 },
    { source: '1', target: '3', relation: 'uses', weight: 0.7 },
  ],
};

describe('Memory — nodeColor', () => {
  it('returns correct color for known types', () => {
    expect(nodeColor('project')).toBe('#6366f1');
    expect(nodeColor('concept')).toBe('#22c55e');
    expect(nodeColor('decision')).toBe('#ec4899');
  });
  it('falls back to default color for unknown types', () => {
    expect(nodeColor('unknown')).toBe('#94a3b8');
    expect(nodeColor('')).toBe('#94a3b8');
  });
});

describe('Memory — nodeLabelTruncate', () => {
  it('returns title as-is when <= 18 chars', () => {
    expect(nodeLabelTruncate('epic-harness')).toBe('epic-harness');
    expect(nodeLabelTruncate('exactly18charstr!!')).toBe('exactly18charstr!!');
  });
  it('truncates long titles to 16 chars + ellipsis', () => {
    const result = nodeLabelTruncate('a very long node title indeed');
    expect(result).toBe('a very long node…');
    expect(result.length).toBe(17); // 16 + ellipsis char
  });
});

describe('Memory — nodeRadius', () => {
  it('scales radius based on importance', () => {
    expect(nodeRadius(0)).toBe(8);
    expect(nodeRadius(1)).toBe(20);
    expect(nodeRadius(0.5)).toBe(14);
  });
});

describe('Memory — graph stats', () => {
  it('counts nodes and edges correctly', () => {
    expect(mockGraph.nodes.length).toBe(3);
    expect(mockGraph.edges.length).toBe(2);
  });
});

// ── Integrations page helpers ──────────────────────────────────────────────────

const INTEGRATIONS = [
  { name: 'Claude Code', id: 'claude-code', description: 'Official Anthropic CLI', setup: 'make install → hooks/bin/', resources: ['8 commands', '12 skills', '4 agents', '6 hooks'] },
  { name: 'Codex', id: 'codex', description: 'OpenAI Codex CLI', setup: 'copy integrations/codex/', resources: ['hooks.json', 'config.toml', '8 prompts', '7 skills'] },
  { name: 'Gemini CLI', id: 'gemini', description: 'Google Gemini CLI', setup: 'copy integrations/gemini/', resources: ['settings.json', 'GEMINI.md', '8 commands', '7 skills'] },
  { name: 'Cursor', id: 'cursor', description: 'Cursor AI editor', setup: 'copy integrations/cursor/', resources: ['hooks.json', '8 commands', '4 agents'] },
  { name: 'Cline', id: 'cline', description: 'VS Code AI assistant', setup: 'copy integrations/cline/', resources: ['5 hook scripts', 'rules/epic-harness.md'] },
  { name: 'Aider', id: 'aider', description: 'AI pair programming CLI', setup: 'copy integrations/aider/', resources: ['.aider.conf.yml', 'CONVENTIONS.md'] },
];

function mergeIntegrationStatus(meta: typeof INTEGRATIONS, statuses: IntegrationStatus[]) {
  return meta.map(m => {
    const s = statuses.find(s => s.name === m.name);
    return { ...m, installed: s?.installed ?? false, config_path: s?.config_path ?? null };
  });
}

const mockStatuses: IntegrationStatus[] = [
  { name: 'Claude Code', installed: true, config_path: '~/.claude/settings.json', version: null },
  { name: 'Codex', installed: false, config_path: null, version: null },
];

describe('Integrations — meta count', () => {
  it('has 6 integrations defined', () => {
    expect(INTEGRATIONS).toHaveLength(6);
  });
  it('all integrations have required fields', () => {
    for (const intg of INTEGRATIONS) {
      expect(intg.name).toBeTruthy();
      expect(intg.id).toBeTruthy();
      expect(intg.setup).toBeTruthy();
      expect(intg.resources.length).toBeGreaterThan(0);
    }
  });
});

describe('Integrations — mergeIntegrationStatus', () => {
  it('marks Claude Code as installed', () => {
    const merged = mergeIntegrationStatus(INTEGRATIONS, mockStatuses);
    const cc = merged.find(m => m.id === 'claude-code');
    expect(cc?.installed).toBe(true);
    expect(cc?.config_path).toBe('~/.claude/settings.json');
  });
  it('marks Codex as not installed', () => {
    const merged = mergeIntegrationStatus(INTEGRATIONS, mockStatuses);
    const codex = merged.find(m => m.id === 'codex');
    expect(codex?.installed).toBe(false);
  });
  it('defaults to not installed for missing status entries', () => {
    const merged = mergeIntegrationStatus(INTEGRATIONS, []);
    for (const m of merged) {
      expect(m.installed).toBe(false);
    }
  });
});

// ── Settings page helpers ──────────────────────────────────────────────────────

function scoreWeightTotal(weights: Record<string, unknown>): number {
  const vals = ['success', 'quality', 'cost'].map(k => Number(weights[k] ?? 0));
  return Math.round(vals.reduce((a, b) => a + b, 0) * 100) / 100;
}

function progressBarWidth(value: number): string {
  return `${Math.round(value * 100)}%`;
}

const mockMetrics: HarnessMetrics = {
  total_sessions: 42,
  avg_success_rate: 0.91,
  total_evolved_skills: 2,
  last_session: '2026-05-18T09:48:57Z',
  score_history: [
    { timestamp: '2026-05-12T04:00:00Z', avg_score: 0.72, success_rate: 0.9, observations: 11 },
    { timestamp: '2026-05-13T04:00:00Z', avg_score: 0.75, success_rate: 0.92, observations: 14 },
    { timestamp: '2026-05-14T04:00:00Z', avg_score: 0.82, success_rate: 0.95, observations: 18 },
  ],
  trend: 'improving',
  stagnation_count: 0,
  session_count: 42,
  avg_score: 0.763,
  skill_attribution: {
    'test-skill': { skill_name: 'test-skill', sessions_active: 5, avg_score_with: 0.85, avg_score_without: 0.78, first_seen: '2026-05-10' },
  },
};

describe('Settings — scoreWeightTotal', () => {
  it('sums default weights to 1.0', () => {
    expect(scoreWeightTotal({ success: 0.5, quality: 0.3, cost: 0.2 })).toBe(1.0);
  });
  it('returns 0 for empty weights', () => {
    expect(scoreWeightTotal({})).toBe(0);
  });
});

describe('Settings — progressBarWidth', () => {
  it('converts 0.5 to 50%', () => {
    expect(progressBarWidth(0.5)).toBe('50%');
  });
  it('converts 0.3 to 30%', () => {
    expect(progressBarWidth(0.3)).toBe('30%');
  });
  it('converts 1.0 to 100%', () => {
    expect(progressBarWidth(1.0)).toBe('100%');
  });
});

// ── Hooks page helpers ─────────────────────────────────────────────────────────

const HOOKS = [
  { event: 'Session Start', command: 'epic-harness resume', trigger: '세션 시작 시', effect: 'Restore session + load evolved skills' },
  { event: 'Pre Tool Use', command: 'epic-harness guard', trigger: '모든 툴 호출 전', effect: 'Block dangerous shell patterns' },
  { event: 'Post Tool Use', command: 'epic-harness observe', trigger: '툴 호출 후 (async)', effect: 'Record 3-axis scores to obs JSONL' },
  { event: 'Post Edit', command: 'epic-harness polish', trigger: '파일 편집 후', effect: 'Auto-format + typecheck' },
  { event: 'Pre Compact', command: 'epic-harness snapshot', trigger: '컨텍스트 압축 전', effect: 'Save session state to sessions/' },
  { event: 'Session End', command: 'epic-harness reflect', trigger: '세션 종료 시', effect: 'Evolve skills + save metrics' },
];

describe('Hooks — registry completeness', () => {
  it('has exactly 6 hooks', () => {
    expect(HOOKS).toHaveLength(6);
  });
  it('all hooks have command starting with epic-harness', () => {
    for (const h of HOOKS) {
      expect(h.command).toMatch(/^epic-harness /);
    }
  });
  it('commands are all unique', () => {
    const cmds = HOOKS.map(h => h.command);
    expect(new Set(cmds).size).toBe(cmds.length);
  });
});
