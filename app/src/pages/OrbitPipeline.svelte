<script lang="ts">
  import { onMount } from 'svelte';
  import { getOrbitPipelines } from '../lib/harness.js';
  import type { OrbitPipeline } from '../lib/harness.js';

  let pipelines = $state<OrbitPipeline[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);

  const runningPipelines = $derived(pipelines.filter(p => p.status === 'running'));
  const historyPipelines = $derived(pipelines.filter(p => p.status !== 'running'));

  const PHASES = ['spec', 'go', 'check', 'ship', 'complete'] as const;
  type Phase = typeof PHASES[number];

  async function load() {
    try {
      error = null;
      pipelines = await getOrbitPipelines();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }

  onMount(() => {
    load();
    const id = setInterval(load, 30000);
    return () => clearInterval(id);
  });

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

  function phaseIndex(phase: string): number {
    return PHASES.indexOf(phase as Phase);
  }

  function durationMinutes(startedAt: string, updatedAt: string): number {
    return Math.round((new Date(updatedAt).getTime() - new Date(startedAt).getTime()) / 60000);
  }

  function fmtDatetime(iso: string): string {
    try {
      return new Date(iso).toLocaleString('ko-KR', { month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit' });
    } catch {
      return iso;
    }
  }

  function shortId(id: string): string {
    return id.slice(0, 8);
  }

  function deadlineRemaining(deadline: string | null): string {
    if (!deadline) return '--';
    const diff = Math.round((new Date(deadline).getTime() - Date.now()) / 60000);
    if (diff <= 0) return '만료';
    return `${diff}분 남음`;
  }

  function phaseStageClass(phase: string, currentPhase: string, phaseHistory: OrbitPipeline['phase_history']): string {
    const completedPhases = phaseHistory.map(h => h.phase);
    if (completedPhases.includes(phase)) return 'completed';
    if (phase === currentPhase) return 'running';
    return 'pending';
  }

  function phaseIcon(phase: string): string {
    const icons: Record<string, string> = {
      spec: '☰',
      go: '▶',
      check: '✓',
      ship: '🚀',
      complete: '↻',
    };
    return icons[phase] ?? phase;
  }

  function phaseColor(phase: string): string {
    const colors: Record<string, string> = {
      spec: 'var(--accent)',
      go: 'var(--accent)',
      check: 'var(--success)',
      ship: 'var(--purple)',
      complete: 'var(--teal)',
    };
    return colors[phase] ?? 'var(--muted)';
  }
</script>

<div class="screen-header">
  <h2>/orbit <span class="subtitle-tag">Autonomous Pipeline</span></h2>
  <p>spec &#8594; go &#8594; check &#8594; ship &#8594; evolve &mdash; single-command spec-to-PR execution</p>
</div>

{#if error}
  <div class="panel" style="margin-bottom:16px;">
    <div class="panel-body">
      <span style="color:var(--danger)">데이터 로드 오류: {error}</span>
    </div>
  </div>
{/if}

<!-- Entry Mode Selection (static) -->
<div class="panel" style="margin-bottom:16px;">
  <div class="panel-header"><h3>Entry Mode Selection</h3></div>
  <div class="panel-body">
    <div class="grid-3">
      <div class="cmd-card" style="border-left:3px solid var(--accent);">
        <div class="cmd-name" style="font-size:13px;">Interactive</div>
        <div class="cmd-desc">User runs <code>/discover</code> &#8594; <code>/spec</code> manually, then triggers orbit. Best for vague requirements.</div>
        <div class="cmd-tags" style="margin-top:8px;"><span class="pill info">unclear requirement</span></div>
      </div>
      <div class="cmd-card" style="border-left:3px solid var(--purple);">
        <div class="cmd-name" style="font-size:13px;">Council</div>
        <div class="cmd-desc">4-voice parallel auto-spec (Architect + Critic + Implementor + QA). Best for complex, multi-stakeholder requirements.</div>
        <div class="cmd-tags" style="margin-top:8px;"><span class="pill purple">complex requirement</span></div>
      </div>
      <div class="cmd-card" style="border-left:3px solid var(--teal);">
        <div class="cmd-name" style="font-size:13px;">Direct</div>
        <div class="cmd-desc">Auto-spec and immediately start building. Best for clear, well-defined requirements.</div>
        <div class="cmd-tags" style="margin-top:8px;"><span class="pill teal">clear requirement</span></div>
      </div>
    </div>
  </div>
</div>

<!-- Running Pipelines (최상단) -->
{#if loading}
  <div class="panel" style="margin-bottom:16px;">
    <div class="panel-header"><h3>실행 중인 파이프라인</h3></div>
    <div class="panel-body" style="color:var(--muted);font-size:13px;">로딩 중...</div>
  </div>
{:else if runningPipelines.length > 0}
  {#each runningPipelines as p}
    <div class="panel" style="margin-bottom:16px;border-left:3px solid var(--accent);">
      <div class="panel-header">
        <h3>
          <span class="pill info" style="margin-right:8px;">RUNNING</span>
          {p.goal_slug ?? '(goal 없음)'}
        </h3>
        <div class="panel-actions" style="font-size:11px;color:var(--muted);font-family:var(--font-mono);">
          ID: {shortId(p.id)} &middot; Mode: {p.mode ?? '--'} &middot; Check fails: {p.check_fail_count}
        </div>
      </div>
      <div class="panel-body">
        <!-- Phase progress bar -->
        <div class="pipeline-flow">
          {#each PHASES as phase, i}
            {#if i > 0}
              <div class="pipeline-arrow {phaseIndex(p.phase) > i - 1 ? 'done' : ''}"></div>
            {/if}
            {@const stageClass = phaseStageClass(phase, p.phase, p.phase_history)}
            <div class="pipeline-stage {stageClass}">
              <div class="stage-icon"
                style="{stageClass === 'completed' || stageClass === 'running'
                  ? `background:${phaseColor(phase)}22;border-color:${phaseColor(phase)};color:${phaseColor(phase)};`
                  : ''}">
                {phaseIcon(phase)}
              </div>
              <div class="stage-label">{phase}</div>
            </div>
          {/each}
        </div>
        <div style="margin-top:12px;font-size:11px;color:var(--muted);font-family:var(--font-mono);display:flex;gap:24px;flex-wrap:wrap;">
          <span>시작: {fmtDatetime(p.started_at)}</span>
          {#if p.deadline}
            <span>마감: {fmtDatetime(p.deadline)} ({deadlineRemaining(p.deadline)})</span>
          {/if}
          <span>체크 실패: {p.check_fail_count} / 3</span>
        </div>
      </div>
    </div>
  {/each}
{/if}

<!-- Pipeline History Table -->
<div class="panel">
  <div class="panel-header"><h3>Pipeline History</h3></div>
  <div class="panel-body" style="padding:0;">
    {#if loading}
      <div style="padding:16px;color:var(--muted);font-size:13px;">로딩 중...</div>
    {:else if pipelines.length === 0}
      <div style="padding:32px;text-align:center;color:var(--muted);">
        <div style="font-size:32px;margin-bottom:8px;">📋</div>
        <div>실행된 파이프라인 없음</div>
        <div style="font-size:11px;margin-top:4px;">/orbit 명령으로 첫 파이프라인을 시작하세요</div>
      </div>
    {:else}
      <table class="data-table">
        <thead>
          <tr>
            <th>ID</th>
            <th>Goal</th>
            <th>Mode</th>
            <th>Phase</th>
            <th>Status</th>
            <th>Started</th>
            <th>Duration</th>
          </tr>
        </thead>
        <tbody>
          {#each historyPipelines as p}
            <tr>
              <td style="font-family:var(--font-mono);color:var(--muted)">{shortId(p.id)}</td>
              <td style="color:var(--fg)">{p.goal_slug ?? '--'}</td>
              <td style="color:var(--fg-secondary)">{p.mode ?? '--'}</td>
              <td style="font-family:var(--font-mono)">{p.phase}</td>
              <td><span class="pill {statusBadgeClass(p.status)}">{p.status}</span></td>
              <td style="font-family:var(--font-mono);font-size:11px;">{fmtDatetime(p.started_at)}</td>
              <td style="font-family:var(--font-mono)">{durationMinutes(p.started_at, p.updated_at)}분</td>
            </tr>
          {:else}
            <tr>
              <td colspan="7" style="text-align:center;color:var(--muted);padding:24px;">히스토리 없음</td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
  </div>
</div>

<!-- Safety Mechanisms (static) -->
<div class="grid-2" style="margin-top:16px;">
  <div class="panel">
    <div class="panel-header"><h3>Pipeline State Schema</h3></div>
    <div class="panel-body" style="padding:0;">
      <table class="data-table">
        <thead><tr><th>Field</th><th>Value</th></tr></thead>
        <tbody>
          <tr><td style="color:var(--muted)">id</td><td style="color:var(--fg)">{'{timestamp}'}</td></tr>
          <tr><td style="color:var(--muted)">mode</td><td style="color:var(--fg)">interactive | council | direct</td></tr>
          <tr><td style="color:var(--muted)">phase</td><td style="color:var(--accent)">mode_select &#8594; spec &#8594; go &#8594; check &#8594; ship</td></tr>
          <tr><td style="color:var(--muted)">status</td><td style="color:var(--fg)">running | completed | failed | timeout | aborted</td></tr>
          <tr><td style="color:var(--muted)">check_fail_count</td><td style="color:var(--fg)">0..3</td></tr>
          <tr><td style="color:var(--muted)">deadline</td><td style="color:var(--fg)">now + 30min</td></tr>
          <tr><td style="color:var(--muted)">worktree_name</td><td style="color:var(--fg)">isolated build env</td></tr>
        </tbody>
      </table>
    </div>
  </div>
  <div class="panel">
    <div class="panel-header"><h3>Safety Mechanisms</h3></div>
    <div class="panel-body">
      <ul class="activity-list">
        <li class="activity-item">
          <span class="activity-dot" style="background:var(--success)"></span>
          <div class="activity-content">
            <div class="activity-title">Concurrent orbit guard</div>
            <div class="activity-time">Only 1 running pipeline at a time</div>
          </div>
        </li>
        <li class="activity-item">
          <span class="activity-dot" style="background:var(--accent)"></span>
          <div class="activity-content">
            <div class="activity-title">Deadline enforcement</div>
            <div class="activity-time">30-min hard timeout per orbit</div>
          </div>
        </li>
        <li class="activity-item">
          <span class="activity-dot" style="background:var(--warning)"></span>
          <div class="activity-content">
            <div class="activity-title">Crash recovery</div>
            <div class="activity-time">45-min staleness threshold, phase_history wins</div>
          </div>
        </li>
        <li class="activity-item">
          <span class="activity-dot" style="background:var(--purple)"></span>
          <div class="activity-content">
            <div class="activity-title">Worktree safety</div>
            <div class="activity-time">Isolated build, state survives worktree loss</div>
          </div>
        </li>
      </ul>
    </div>
  </div>
</div>
