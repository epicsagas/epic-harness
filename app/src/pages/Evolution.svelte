<script lang="ts">
  import { getEvolvedSkills, getHarnessMetrics, getSeesawRegistry, getVariantPool, getAdaptationLandscape } from '../lib/harness.js';
  import type { EvolutionData, HarnessMetrics, SolvedTaskRegistry, VariantPool, AdaptationLandscape } from '../lib/harness.js';
  import DateRangePicker from '$lib/components/DateRangePicker.svelte';
  import { tStore } from '$lib/i18n.js';
  import { selectedProject } from '$lib/stores/project.js';

  const SEEDING_THRESHOLDS = [
    { typeKey: 'seedTypeWeakTool',       threshKey: 'seedThreshWeakTool' },
    { typeKey: 'seedTypeWeakFileType',   threshKey: 'seedThreshWeakExt' },
    { typeKey: 'seedTypeHighFreqError',  threshKey: 'seedThreshHighFreq' },
    { typeKey: 'seedTypeStagnationRollback', threshKey: 'seedThreshStagnation' },
  ] as const;

  let data  = $state<EvolutionData | null>(null);
  let error = $state<string | null>(null);
  let loading = $state(true);
  // HarnessX evolution-engine state (4-PR stack)
  let metrics = $state<HarnessMetrics | null>(null);
  let seesaw = $state<SolvedTaskRegistry | null>(null);
  let variants = $state<VariantPool | null>(null);
  let landscape = $state<AdaptationLandscape | null>(null);

  // ── Update-flash: glow a section's border when its data changes.
  // Uses a canonical (sorted-key) JSON signature so backend HashMap
  // iteration-order non-determinism does not cause false-positive flashes.
  function canon(value: unknown): string {
    if (value === null || typeof value !== 'object') return JSON.stringify(value);
    if (Array.isArray(value)) return '[' + value.map(canon).join(',') + ']';
    const obj = value as Record<string, unknown>;
    return '{' + Object.keys(obj).sort().map(k => JSON.stringify(k)+':'+canon(obj[k])).join(',') + '}';
  }
  let flash = $state<Record<string, boolean>>({});
  const prevSnap: Record<string, string> = {};
  function flashIfChanged(key: string, payload: unknown): void {
    const sig = canon(payload);
    if (prevSnap[key] !== undefined && prevSnap[key] !== sig) {
      // Re-trigger the CSS animation by flipping the flag off→on.
      flash[key] = false;
      // microtask so Svelte re-renders the off state before re-adding
      queueMicrotask(() => { flash[key] = true; });
      window.setTimeout(() => { flash[key] = false; }, 1700);
    }
    prevSnap[key] = sig;
  }

  type RawRow = Record<string, unknown>;

  function deriveTrend(rows: RawRow[], idx: number): 'improving' | 'declining' | 'stable' {
    if (idx === 0) return 'stable';
    const prev = rows[idx - 1]['avg_score'] as number ?? 0;
    const cur  = rows[idx]['avg_score']  as number ?? 0;
    const delta = cur - prev;
    if (delta > 0.01) return 'improving';
    if (delta < -0.01) return 'declining';
    return 'stable';
  }

  // failure_patterns entries are DetectedPattern objects, not strings — join()
  // on them renders "[object Object]". Older rows may still hold plain strings,
  // and the scoring_bias variant carries no involved_files, so read defensively.
  function patternLabel(p: unknown): string {
    if (typeof p === 'string') return p;
    if (p && typeof p === 'object') {
      const o = p as Record<string, unknown>;
      const type = typeof o['pattern_type'] === 'string' ? o['pattern_type'] as string : '';
      const desc = typeof o['description'] === 'string' ? o['description'] as string : '';
      if (type && desc) return `${type}: ${desc}`;
      if (type || desc) return type || desc;
    }
    return '';
  }

  function patternSummary(row: RawRow): string {
    const fp = row['failure_patterns'];
    if (Array.isArray(fp) && fp.length > 0) {
      const labels = fp.map(patternLabel).filter(Boolean);
      if (labels.length > 0) return labels.join(', ');
    }
    const ep = row['error_patterns'];
    if (ep && typeof ep === 'object' && Object.keys(ep).length > 0)
      return Object.keys(ep).join(', ');
    const summary = row['analysis_summary'] as string ?? '';
    return summary || '—';
  }

  // Newest first, regardless of the order the backend happened to return.
  // Trend compares each row against the chronologically previous one, so the
  // comparison runs on the ascending copy before the list is reversed.
  const enriched = $derived((): RawRow[] => {
    if (!data) return [];
    const asc = (data.evolution_history as RawRow[])
      .slice()
      .sort((a, b) =>
        String(a['timestamp'] ?? '').localeCompare(String(b['timestamp'] ?? '')));
    return asc
      .map((r, i) => ({
        ...r,
        _trend: deriveTrend(asc, i),
        _patterns: patternSummary(r),
      }))
      .reverse();
  });

  // ── filters
  let filterDateFrom = $state('');
  let filterDateTo   = $state('');
  let filterTrend   = $state('');
  let filterPattern = $state('');

  const filteredHistory = $derived((): RawRow[] => {
    let list = enriched();
    if (filterDateFrom)
      list = list.filter(r => (r['timestamp'] as string ?? '').slice(0, 10) >= filterDateFrom);
    if (filterDateTo)
      list = list.filter(r => (r['timestamp'] as string ?? '').slice(0, 10) <= filterDateTo);
    if (filterTrend)
      list = list.filter(r => r['_trend'] === filterTrend);
    if (filterPattern.trim()) {
      const q = filterPattern.trim().toLowerCase();
      list = list.filter(r => (r['_patterns'] as string).toLowerCase().includes(q));
    }
    return list;
  });

  // ── pagination
  const PAGE_SIZE = 10;
  let histPage = $state(1);
  const histTotalPages = $derived(Math.max(1, Math.ceil(filteredHistory().length / PAGE_SIZE)));
  const histPageItems  = $derived(filteredHistory().slice((histPage - 1) * PAGE_SIZE, histPage * PAGE_SIZE));

  $effect(() => { filterDateFrom; filterDateTo; filterTrend; filterPattern; histPage = 1; });

  const hasFilter = $derived(!!(filterDateFrom || filterDateTo || filterTrend || filterPattern.trim()));
  function clearFilters() { filterDateFrom = ''; filterDateTo = ''; filterTrend = ''; filterPattern = ''; }

  const totalPatterns = $derived(
    (data?.evolution_history ?? []).reduce((acc, r) => {
      const fp = (r as RawRow)['failure_patterns'];
      return acc + (Array.isArray(fp) ? fp.length : 0);
    }, 0)
  );

  async function load() {
    try {
      data = await getEvolvedSkills();
      // Fetch HarnessX evolution-engine surfaces in parallel (best-effort;
      // these may be absent on a cold project — treat null as empty).
      const [m, sw, vp, al] = await Promise.allSettled([
        getHarnessMetrics(),
        getSeesawRegistry(),
        getVariantPool(),
        getAdaptationLandscape(),
      ]);
      metrics = m.status === 'fulfilled' ? m.value : null;
      seesaw = sw.status === 'fulfilled' ? sw.value : null;
      variants = vp.status === 'fulfilled' ? vp.value : null;
      landscape = al.status === 'fulfilled' ? al.value : null;
      // Trigger a border-glow flash on any section whose payload changed
      // since the previous poll (skips the very first load).
      flashIfChanged('metrics', metrics);
      flashIfChanged('seesaw', seesaw);
      flashIfChanged('variants', variants);
      flashIfChanged('landscape', landscape);
      error = null;
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }

  function dateOnly(ts: unknown): string {
    if (!ts || typeof ts !== 'string') return '—';
    return ts.slice(0, 10);
  }

  function extractSkillDesc(md: string): string {
    if (!md) return '—';
    const lines = md.split('\n');
    let inFrontmatter = false;
    let closedFrontmatter = false;
    for (const line of lines) {
      if (!closedFrontmatter) {
        if (line.trim() === '---' && !inFrontmatter) { inFrontmatter = true; continue; }
        if (line.trim() === '---' && inFrontmatter) { inFrontmatter = false; closedFrontmatter = true; continue; }
        continue;
      }
      if (line.startsWith('#')) return line.replace(/^#+\s*/, '').trim();
    }
    for (const line of lines) {
      if (line.startsWith('#')) return line.replace(/^#+\s*/, '').trim();
    }
    const descMatch = md.match(/^description:\s*"(.+)"/m);
    if (descMatch) return descMatch[1];
    return '—';
  }

  // Reload whenever the selected project changes (projectArgs() reads it at
  // call time), then poll on an interval. $effect re-runs on project switch.
  $effect(() => {
    const _project = $selectedProject; // reactive dependency
    load();
    const id = setInterval(() => { if (!document.hidden) load(); }, 30_000);
    return () => clearInterval(id);
  });
</script>

<div class="screen-header">
  <h2>{$tStore('pageEvolve')} <span class="subtitle-tag">Ring 3</span></h2>
  <p>{$tStore('pageEvolveDesc')}</p>
</div>

{#if metrics?.reward_hacking_suspected}
  <div class="rh-banner" class:hx-flash={flash.metrics} role="alert">
    ⚠️ <strong>Reward hacking suspected</strong> — execution_cost rising while output_quality falls.
    Skill seeding is blocked this round until the divergence resolves.
  </div>
{/if}

{#if !loading && (seesaw || variants || landscape)}
  <div class="harnessx-panels">
    {#if seesaw && seesaw.total_solved > 0}
      <div class="hx-panel" class:hx-flash={flash.seesaw}>
        <h3>Seesaw — solved tasks ({seesaw.total_solved})</h3>
        <p class="hx-muted">Per-task regression gate (HarnessX §4.1). Tasks below their best score − tolerance block seeding.</p>
        <ul class="hx-list">
          {#each Object.entries(seesaw.solved).slice(0, 8) as [task, best]}
            <li><code>{task}</code> → best {Number(best).toFixed(2)}</li>
          {/each}
        </ul>
      </div>
    {/if}

    {#if variants && variants.variants.length > 0}
      <div class="hx-panel" class:hx-flash={flash.variants}>
        <h3>Variants ({variants.variants.length})</h3>
        <p class="hx-muted">Variant isolation / ensemble routing (§4.5). Fork-on-regression prevents catastrophic forgetting.</p>
        <ul class="hx-list">
          {#each variants.variants as v}
            <li><code>{v.id}</code> · tags [{v.domain_tags.join(', ')}] · {v.skills.length} skill(s) · avg {v.avg_score.toFixed(2)}</li>
          {/each}
        </ul>
      </div>
    {/if}

    {#if landscape}
      <div class="hx-panel" class:hx-flash={flash.landscape}>
        <h3>Adaptation landscape</h3>
        <p class="hx-muted">Planner (§4.3). Under-exploration signal + persistent failures.</p>
        {#if landscape.persistent_failures.length > 0}
          <div class="hx-subhead">Persistent failures ({landscape.persistent_failures.length})</div>
          <ul class="hx-list">
            {#each landscape.persistent_failures.slice(0, 6) as pf}
              <li><code>{pf.failure_category}</code> · {pf.sessions_seen} session(s){pf.resolved ? ' · resolved' : ''}</li>
            {/each}
          </ul>
        {/if}
        {#if landscape.untried_edit_types.length > 0}
          <div class="hx-subhead">Untried edit types</div>
          <div class="hx-tags">{#each landscape.untried_edit_types as t}<span class="hx-tag">{t}</span>{/each}</div>
        {/if}
        {#if Object.keys(landscape.edit_type_coverage).length > 0}
          <div class="hx-subhead">Edit-type coverage</div>
          <ul class="hx-list">
            {#each Object.entries(landscape.edit_type_coverage) as [t, c]}
              <li><code>{t}</code> · {c} attempt(s)</li>
            {/each}
          </ul>
        {/if}
      </div>
    {/if}
  </div>
{/if}

{#if loading}
  <div style="color:var(--muted);padding:32px 0;text-align:center;font-family:var(--font-mono);font-size:13px;">
    {$tStore('loadingEvolution')}
  </div>
{:else if error}
  <div style="color:var(--danger);padding:16px;background:var(--danger-soft);border-radius:var(--radius);font-size:13px;">
    {$tStore('errorPrefix')}: {error}
  </div>
{:else if data}

  <!-- Stats -->
  <div style="display:grid;grid-template-columns:repeat(4,1fr);gap:12px;margin-bottom:20px;">
    <div class="panel">
      <div class="panel-body" style="text-align:center;">
        <div style="font-size:28px;font-weight:700;color:var(--accent);font-family:var(--font-mono);">{data.total_sessions_analyzed}</div>
        <div style="font-size:11px;color:var(--fg-secondary);margin-top:4px;">{$tStore('statSessionsAnalyzed')}</div>
      </div>
    </div>
    <div class="panel">
      <div class="panel-body" style="text-align:center;">
        <div style="font-size:28px;font-weight:700;color:var(--orange);font-family:var(--font-mono);">{totalPatterns}</div>
        <div style="font-size:11px;color:var(--fg-secondary);margin-top:4px;">{$tStore('statFailurePatterns')}</div>
      </div>
    </div>
    <div class="panel">
      <div class="panel-body" style="text-align:center;">
        <div style="font-size:28px;font-weight:700;color:var(--success);font-family:var(--font-mono);">{data.evolved_skills.length}</div>
        <div style="font-size:11px;color:var(--fg-secondary);margin-top:4px;">{$tStore('statEvolvedSkills')}</div>
      </div>
    </div>
    <div class="panel">
      <div class="panel-body" style="text-align:center;">
        <div style="font-size:28px;font-weight:700;color:var(--purple);font-family:var(--font-mono);">10</div>
        <div style="font-size:11px;color:var(--fg-secondary);margin-top:4px;">{$tStore('statMaxSkillsCap')}</div>
      </div>
    </div>
  </div>

  <!-- Evolved Skills -->
  <div class="panel" style="margin-bottom:16px;">
    <div class="panel-header"><h3>{$tStore('evolvedSkillsTitle')}</h3></div>
    <div class="panel-body">
      {#if data.evolved_skills.length === 0}
        <div style="color:var(--muted);font-size:13px;text-align:center;padding:16px 0;">
          {$tStore('noEvolvedSkills')}
        </div>
      {:else}
        <div style="display:grid;grid-template-columns:repeat(auto-fill,minmax(280px,1fr));gap:12px;">
          {#each data.evolved_skills as skill}
            <div class="skill-card">
              <div class="skill-name" style="color:var(--orange);">{skill.name}</div>
              <div class="skill-desc" style="margin-top:4px;">{extractSkillDesc(skill.skill_md)}</div>
              <div style="margin-top:8px;font-size:11px;color:var(--muted);font-family:var(--font-mono);">
                {dateOnly(skill.created_at)}
              </div>
            </div>
          {/each}
        </div>
      {/if}
    </div>
  </div>

  <!-- Evolution History -->
  <div class="panel" style="margin-bottom:16px;">
    <div class="panel-header">
      <h3>{$tStore('evolutionHistoryTitle')}
        <span style="font-size:11px;font-weight:400;color:var(--muted);margin-left:8px;">
          {$tStore('filterCountFmt', filteredHistory().length, data.evolution_history.length)}
        </span>
      </h3>
    </div>

    <!-- Filter bar -->
    <div style="padding:10px 16px;border-bottom:1px solid var(--border);display:flex;gap:8px;flex-wrap:wrap;align-items:center;">
      <DateRangePicker
        from={filterDateFrom}
        to={filterDateTo}
        onchange={(f, t) => { filterDateFrom = f; filterDateTo = t; }}
      />
      <select
        bind:value={filterTrend}
        style="background:var(--bg);border:1px solid var(--border);border-radius:var(--radius-sm);padding:4px 10px;color:var(--fg);font-size:12px;outline:none;"
      >
        <option value="">{$tStore('allTrend')}</option>
        <option value="improving">{$tStore('trendImproving')}</option>
        <option value="stable">{$tStore('trendStable')}</option>
        <option value="declining">{$tStore('trendDeclining')}</option>
      </select>
      <input
        type="text"
        placeholder={$tStore('patternSearch')}
        bind:value={filterPattern}
        style="background:var(--bg);border:1px solid var(--border);border-radius:var(--radius-sm);padding:4px 10px;color:var(--fg);font-size:12px;width:200px;outline:none;"
      />
      {#if hasFilter}
        <button
          onclick={clearFilters}
          style="background:transparent;border:1px solid var(--border);border-radius:var(--radius-sm);padding:4px 10px;color:var(--muted);font-size:12px;cursor:pointer;"
        >{$tStore('reset')}</button>
      {/if}
    </div>

    <div class="panel-body" style="padding:0;">
      {#if filteredHistory().length === 0}
        <div style="color:var(--muted);font-size:13px;text-align:center;padding:24px;">
          {hasFilter ? $tStore('noHistoryFilter') : $tStore('noHistoryEmpty')}
        </div>
      {:else}
        <table class="data-table">
          <thead>
            <tr>
              <th>{$tStore('colDate')}</th>
              <th>{$tStore('colSummaryPatterns')}</th>
              <th style="text-align:right;">{$tStore('colObs')}</th>
              <th style="text-align:right;">{$tStore('colSkills')}</th>
              <th>Edit</th>
              <th>{$tStore('colTrend')}</th>
              <th style="text-align:right;">{$tStore('colAvgScore')}</th>
            </tr>
          </thead>
          <tbody>
            {#each histPageItems as row}
              <tr>
                <td style="font-family:var(--font-mono);font-size:12px;white-space:nowrap;">
                  {dateOnly(row['timestamp'])}
                </td>
                <td style="font-size:12px;color:var(--fg-secondary);max-width:220px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;"
                    title={row['_patterns'] as string}>
                  {row['_patterns']}
                </td>
                <td style="font-family:var(--font-mono);font-size:12px;text-align:right;color:var(--muted);">
                  {row['observations'] ?? '—'}
                </td>
                <td style="font-family:var(--font-mono);text-align:right;">
                  {row['skills_seeded'] ?? 0}
                </td>
                <td style="font-family:var(--font-mono);font-size:11px;color:var(--muted);">
                  {row['edit_type'] ?? 'add_skill'}
                </td>
                <td>
                  {#if row['_trend'] === 'improving'}
                    <span class="pill success">{$tStore('trendImproving')}</span>
                  {:else if row['_trend'] === 'declining'}
                    <span class="pill danger">{$tStore('trendDeclining')}</span>
                  {:else}
                    <span class="pill info">{$tStore('trendStable')}</span>
                  {/if}
                </td>
                <td style="font-family:var(--font-mono);text-align:right;">
                  {typeof row['avg_score'] === 'number' ? (row['avg_score'] as number).toFixed(3) : '—'}
                </td>
              </tr>
            {/each}
          </tbody>
        </table>

        <!-- Pagination -->
        {#if histTotalPages > 1}
          <div style="display:flex;align-items:center;justify-content:space-between;padding:10px 16px;border-top:1px solid var(--border);">
            <span style="font-size:12px;color:var(--muted);">
              {(histPage-1)*PAGE_SIZE+1}–{Math.min(histPage*PAGE_SIZE, filteredHistory().length)} / {filteredHistory().length}
            </span>
            <div style="display:flex;gap:4px;">
              <button onclick={() => histPage = 1} disabled={histPage === 1}
                style="padding:3px 8px;border:1px solid var(--border);border-radius:var(--radius-sm);background:var(--bg);color:{histPage===1?'var(--muted)':'var(--fg)'};font-size:12px;cursor:{histPage===1?'default':'pointer'};">«</button>
              <button onclick={() => histPage--} disabled={histPage === 1}
                style="padding:3px 8px;border:1px solid var(--border);border-radius:var(--radius-sm);background:var(--bg);color:{histPage===1?'var(--muted)':'var(--fg)'};font-size:12px;cursor:{histPage===1?'default':'pointer'};">‹</button>
              {#each Array.from({length: histTotalPages}, (_, i) => i + 1).filter(n => Math.abs(n - histPage) <= 2) as n}
                <button onclick={() => histPage = n}
                  style="padding:3px 8px;border:1px solid {n===histPage?'var(--accent)':'var(--border)'};border-radius:var(--radius-sm);background:{n===histPage?'var(--accent-soft)':'var(--bg)'};color:{n===histPage?'var(--accent)':'var(--fg)'};font-size:12px;cursor:pointer;font-weight:{n===histPage?'600':'400'};">{n}</button>
              {/each}
              <button onclick={() => histPage++} disabled={histPage === histTotalPages}
                style="padding:3px 8px;border:1px solid var(--border);border-radius:var(--radius-sm);background:var(--bg);color:{histPage===histTotalPages?'var(--muted)':'var(--fg)'};font-size:12px;cursor:{histPage===histTotalPages?'default':'pointer'};">›</button>
              <button onclick={() => histPage = histTotalPages} disabled={histPage === histTotalPages}
                style="padding:3px 8px;border:1px solid var(--border);border-radius:var(--radius-sm);background:var(--bg);color:{histPage===histTotalPages?'var(--muted)':'var(--fg)'};font-size:12px;cursor:{histPage===histTotalPages?'default':'pointer'};">»</button>
            </div>
          </div>
        {/if}
      {/if}
    </div>
  </div>

  <!-- Seeding Thresholds -->
  <div class="panel">
    <div class="panel-header"><h3>{$tStore('seedingThresholdsTitle')}</h3></div>
    <div class="panel-body" style="padding:0;">
      <table class="data-table">
        <thead><tr><th>{$tStore('colType')}</th><th>{$tStore('colThreshold')}</th></tr></thead>
        <tbody>
          {#each SEEDING_THRESHOLDS as row}
            <tr>
              <td style="color:var(--fg);">{$tStore(row.typeKey)}</td>
              <td style="font-size:12px;color:var(--fg-secondary);font-family:var(--font-mono);">{$tStore(row.threshKey)}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  </div>
{/if}

<style>
  .rh-banner {
    background: var(--danger-soft);
    border: 1px solid var(--danger);
    color: var(--danger);
    padding: 10px 14px;
    border-radius: var(--radius, 8px);
    font-size: 13px;
    margin: 8px 0 16px;
  }
  .harnessx-panels {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(280px, 1fr));
    gap: 12px;
    margin: 8px 0 20px;
  }
  .hx-panel {
    background: var(--surface, rgba(255,255,255,0.03));
    border: 1px solid var(--border, rgba(255,255,255,0.08));
    border-radius: var(--radius, 8px);
    padding: 12px 14px;
    font-size: 12px;
  }
  .hx-panel h3 {
    margin: 0 0 4px;
    font-size: 13px;
  }
  .hx-muted {
    color: var(--muted, #8b949e);
    margin: 0 0 8px;
    font-size: 11px;
  }
  .hx-list {
    list-style: none;
    padding: 0;
    margin: 4px 0 0;
  }
  .hx-list li {
    padding: 2px 0;
    font-family: var(--font-mono, monospace);
    font-size: 11px;
    color: var(--fg-secondary, #b1bac4);
  }
  .hx-subhead {
    font-size: 11px;
    color: var(--muted, #8b949e);
    margin-top: 8px;
    margin-bottom: 2px;
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }
  .hx-tags {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }
  .hx-tag {
    background: rgba(255,255,255,0.06);
    border-radius: 4px;
    padding: 1px 6px;
    font-family: var(--font-mono, monospace);
    font-size: 10px;
  }
</style>
