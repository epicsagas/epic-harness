// Tests for Dashboard, OrbitPipeline, Agents page data-binding logic.
// These tests validate the pure helper functions used in the components.

import { describe, it, expect } from 'vitest';
import type { HarnessMetrics, ObsSummary, OrbitPipeline } from '../lib/harness.js';

// ── helpers extracted from components ─────────────────────────────────────────

function totalFailures(obs: ObsSummary): number {
  return obs.recent_sessions.reduce((sum, s) => sum + s.failures, 0);
}

function evalRingOffset(avgScore: number): number {
  // circumference = 2 * pi * 42 ≈ 264
  return Math.round(264 * (1 - avgScore));
}

function topToolStats(obs: ObsSummary, n: number) {
  return [...obs.tool_stats].sort((a, b) => b.calls - a.calls).slice(0, n);
}

function durationMinutes(startedAt: string, updatedAt: string): number {
  return Math.round((new Date(updatedAt).getTime() - new Date(startedAt).getTime()) / 60000);
}

function truncate(s: string, len: number): string {
  return s.length > len ? s.slice(0, len) + '…' : s;
}

function relativeTime(ts: string): string {
  const diff = Math.floor((Date.now() - new Date(ts).getTime()) / 1000);
  if (diff < 60) return `${diff}s ago`;
  if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
  return `${Math.floor(diff / 3600)}h ago`;
}

function statusBadgeClass(status: OrbitPipeline['status']): string {
  const map: Record<string, string> = {
    running: 'info',
    complete: 'success',
    failed: 'danger',
    paused: 'warning',
    aborted: 'muted',
    timeout: 'danger',
  };
  return map[status] ?? 'muted';
}

const PIPELINE_PHASES = ['spec', 'go', 'check', 'ship', 'complete'] as const;
type PipelinePhase = typeof PIPELINE_PHASES[number];

function phaseIndex(phase: string): number {
  return PIPELINE_PHASES.indexOf(phase as PipelinePhase);
}

function lowSuccessTools(obs: ObsSummary, threshold = 0.85) {
  return obs.tool_stats.filter(t => t.success_rate < threshold);
}

// ── mock data ─────────────────────────────────────────────────────────────────

const mockMetrics: HarnessMetrics = {
  score_history: [0.72, 0.75, 0.82],
  trend: 'improving',
  stagnation_count: 0,
  session_count: 42,
  avg_score: 0.807,
  skill_attribution: {},
  score_weights: {},
};

const mockObs: ObsSummary = {
  recent_sessions: [
    { session_id: 'a', date: '20260518', tool_calls: 38, avg_score: 0.91, failures: 2 },
    { session_id: 'b', date: '20260518', tool_calls: 22, avg_score: 0.88, failures: 1 },
    { session_id: 'c', date: '20260517', tool_calls: 55, avg_score: 0.84, failures: 4 },
  ],
  tool_stats: [
    { tool: 'Read',  calls: 45, success_rate: 1.00, avg_score: 0.95 },
    { tool: 'Edit',  calls: 38, success_rate: 0.92, avg_score: 0.89 },
    { tool: 'Bash',  calls: 30, success_rate: 0.87, avg_score: 0.81 },
    { tool: 'Write', calls: 12, success_rate: 1.00, avg_score: 0.95 },
    { tool: 'Agent', calls:  8, success_rate: 0.75, avg_score: 0.78 },
  ],
  total_tool_calls: 133,
  avg_score: 0.891,
  active_agents: [
    { name: 'builder', last_tool: 'Edit', last_action: 'writing Dashboard.svelte', score: 0.91, timestamp: new Date().toISOString() },
  ],
};

const mockPipelines: OrbitPipeline[] = [
  {
    id: '20260518192211',
    mode: 'direct',
    phase: 'go',
    status: 'running',
    goal_slug: 'dashboard-live-metrics',
    branch: 'feat/live-metrics',
    check_fail_count: 0,
    started_at: new Date(Date.now() - 5 * 60000).toISOString(),
    updated_at: new Date().toISOString(),
    deadline: new Date(Date.now() + 25 * 60000).toISOString(),
    phase_history: [{ phase: 'spec', status: 'complete', completed_at: '' }],
  },
  {
    id: '20260517110000',
    mode: 'council',
    phase: 'complete',
    status: 'complete',
    goal_slug: 'epic-harness-dashboard',
    branch: 'feat/epic-harness-dashboard',
    check_fail_count: 0,
    started_at: new Date(Date.now() - 86400000).toISOString(),
    updated_at: new Date(Date.now() - 82800000).toISOString(),
    deadline: null,
    phase_history: [],
  },
];

// ── Dashboard helpers ──────────────────────────────────────────────────────────

describe('Dashboard — totalFailures', () => {
  it('sums failures across all recent sessions', () => {
    expect(totalFailures(mockObs)).toBe(7); // 2+1+4
  });
  it('returns 0 when no sessions', () => {
    expect(totalFailures({ ...mockObs, recent_sessions: [] })).toBe(0);
  });
});

describe('Dashboard — evalRingOffset', () => {
  it('returns 0 for perfect score', () => {
    expect(evalRingOffset(1.0)).toBe(0);
  });
  it('returns 264 for zero score', () => {
    expect(evalRingOffset(0)).toBe(264);
  });
  it('returns correct offset for 0.807', () => {
    // 264 * (1 - 0.807) = 264 * 0.193 ≈ 50.952 → 51
    expect(evalRingOffset(0.807)).toBe(51);
  });
});

describe('Dashboard — topToolStats', () => {
  it('returns top-N tools sorted by calls descending', () => {
    const top3 = topToolStats(mockObs, 3);
    expect(top3).toHaveLength(3);
    expect(top3[0].tool).toBe('Read');
    expect(top3[1].tool).toBe('Edit');
    expect(top3[2].tool).toBe('Bash');
  });
  it('does not mutate original tool_stats order', () => {
    const originalFirst = mockObs.tool_stats[0].tool;
    topToolStats(mockObs, 5);
    expect(mockObs.tool_stats[0].tool).toBe(originalFirst);
  });
});

// ── OrbitPipeline helpers ──────────────────────────────────────────────────────

describe('OrbitPipeline — durationMinutes', () => {
  it('calculates difference in minutes', () => {
    const start = new Date(Date.now() - 10 * 60000).toISOString();
    const end = new Date().toISOString();
    const dur = durationMinutes(start, end);
    expect(dur).toBeGreaterThanOrEqual(9);
    expect(dur).toBeLessThanOrEqual(11);
  });
  it('returns 0 for same timestamps', () => {
    const ts = new Date().toISOString();
    expect(durationMinutes(ts, ts)).toBe(0);
  });
});

describe('OrbitPipeline — statusBadgeClass', () => {
  it('maps each status to expected pill class', () => {
    expect(statusBadgeClass('running')).toBe('info');
    expect(statusBadgeClass('complete')).toBe('success');
    expect(statusBadgeClass('failed')).toBe('danger');
    expect(statusBadgeClass('paused')).toBe('warning');
    expect(statusBadgeClass('aborted')).toBe('muted');
  });
});

describe('OrbitPipeline — phaseIndex', () => {
  it('returns correct index for each phase', () => {
    expect(phaseIndex('spec')).toBe(0);
    expect(phaseIndex('go')).toBe(1);
    expect(phaseIndex('check')).toBe(2);
    expect(phaseIndex('ship')).toBe(3);
    expect(phaseIndex('complete')).toBe(4);
  });
  it('returns -1 for unknown phase', () => {
    expect(phaseIndex('unknown')).toBe(-1);
  });
});

describe('OrbitPipeline — running vs history split', () => {
  it('separates running pipelines from history', () => {
    const running = mockPipelines.filter(p => p.status === 'running');
    const history = mockPipelines.filter(p => p.status !== 'running');
    expect(running).toHaveLength(1);
    expect(running[0].goal_slug).toBe('dashboard-live-metrics');
    expect(history).toHaveLength(1);
  });
});

// ── Agents helpers ─────────────────────────────────────────────────────────────

describe('Agents — truncate', () => {
  it('does not truncate short strings', () => {
    expect(truncate('hello', 50)).toBe('hello');
  });
  it('truncates and appends ellipsis', () => {
    const long = 'a'.repeat(60);
    const result = truncate(long, 50);
    expect(result).toHaveLength(51); // 50 chars + ellipsis char
    expect(result.endsWith('…')).toBe(true);
  });
});

describe('Agents — lowSuccessTools', () => {
  it('returns tools below success_rate threshold', () => {
    const low = lowSuccessTools(mockObs, 0.85);
    expect(low.map(t => t.tool)).toContain('Agent'); // 0.75 < 0.85
    expect(low.map(t => t.tool)).not.toContain('Read'); // 1.0 >= 0.85
  });
  it('returns empty array when all tools pass threshold', () => {
    const allGood: ObsSummary = {
      ...mockObs,
      tool_stats: [{ tool: 'Read', calls: 10, success_rate: 1.0, avg_score: 0.95 }],
    };
    expect(lowSuccessTools(allGood, 0.85)).toHaveLength(0);
  });
});

describe('Agents — relativeTime', () => {
  it('shows seconds for very recent timestamps', () => {
    const ts = new Date(Date.now() - 30000).toISOString();
    expect(relativeTime(ts)).toMatch(/\ds ago/);
  });
  it('shows minutes for 2-minute-old timestamp', () => {
    const ts = new Date(Date.now() - 2 * 60000).toISOString();
    expect(relativeTime(ts)).toMatch(/\dm ago/);
  });
  it('shows hours for 2-hour-old timestamp', () => {
    const ts = new Date(Date.now() - 2 * 3600000).toISOString();
    expect(relativeTime(ts)).toMatch(/\dh ago/);
  });
});
