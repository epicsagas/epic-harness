<script lang="ts">
  import { onMount } from 'svelte';
  import { getIntegrationStatus, type IntegrationStatus } from '../lib/harness.js';

  let statuses = $state<IntegrationStatus[]>([]);
  let loading = $state(true);
  let error = $state('');

  const INTEGRATIONS = [
    { name: 'Claude Code', id: 'claude-code', description: 'Official Anthropic CLI', setup: 'make install → hooks/bin/', resources: ['8 commands', '12 skills', '4 agents', '6 hooks'] },
    { name: 'Codex', id: 'codex', description: 'OpenAI Codex CLI', setup: 'copy integrations/codex/', resources: ['hooks.json', 'config.toml', '8 prompts', '7 skills'] },
    { name: 'Gemini CLI', id: 'gemini', description: 'Google Gemini CLI', setup: 'copy integrations/gemini/', resources: ['settings.json', 'GEMINI.md', '8 commands', '7 skills'] },
    { name: 'Cursor', id: 'cursor', description: 'Cursor AI editor', setup: 'copy integrations/cursor/', resources: ['hooks.json', '8 commands', '4 agents'] },
    { name: 'Cline', id: 'cline', description: 'VS Code AI assistant', setup: 'copy integrations/cline/', resources: ['5 hook scripts', 'rules/epic-harness.md'] },
    { name: 'Aider', id: 'aider', description: 'AI pair programming CLI', setup: 'copy integrations/aider/', resources: ['.aider.conf.yml', 'CONVENTIONS.md'] },
  ];

  const merged = $derived(
    INTEGRATIONS.map(m => {
      const s = statuses.find(s => s.name === m.name);
      return { ...m, installed: s?.installed ?? false, config_path: s?.config_path ?? null };
    })
  );

  onMount(async () => {
    try {
      statuses = await getIntegrationStatus();
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  });
</script>

<div class="page">
  <div class="page-header">
    <h1>Integrations</h1>
    <p>6 AI coding tool integrations · configs in <code>integrations/</code></p>
  </div>

  {#if loading}
    <div class="loading">Loading integration status…</div>
  {:else if error}
    <div class="error-msg">{error}</div>
  {:else}
    <!-- Integration cards -->
    <div style="display:grid; grid-template-columns: repeat(auto-fill, minmax(340px, 1fr)); gap: 1rem; margin-bottom: 1.5rem;">
      {#each merged as intg}
        <div class="card">
          <div style="display:flex; align-items:flex-start; justify-content:space-between; margin-bottom:0.75rem;">
            <div>
              <h3 style="margin:0 0 0.25rem;">{intg.name}</h3>
              <p style="margin:0; font-size:0.85rem; color:var(--text-secondary);">{intg.description}</p>
            </div>
            <span class="badge" style="background:{intg.installed ? 'var(--success-soft, #dcfce7)' : 'var(--surface)'}; color:{intg.installed ? 'var(--success, #22c55e)' : 'var(--text-secondary)'}; white-space:nowrap; flex-shrink:0;">
              {intg.installed ? 'Installed' : 'Not installed'}
            </span>
          </div>

          {#if intg.config_path}
            <div style="font-size:0.75rem; color:var(--text-secondary); margin-bottom:0.5rem; font-family:monospace;">
              {intg.config_path}
            </div>
          {/if}

          <!-- Resources chips -->
          <div style="display:flex; flex-wrap:wrap; gap:0.3rem; margin-bottom:0.75rem;">
            {#each intg.resources as res}
              <span style="font-size:0.72rem; padding:0.15rem 0.5rem; border-radius:4px; background:var(--surface-2, var(--surface)); border:1px solid var(--border); color:var(--text-secondary);">{res}</span>
            {/each}
          </div>

          <!-- Setup command -->
          <div style="background:var(--bg); border-radius:4px; padding:0.4rem 0.6rem; font-family:monospace; font-size:0.78rem; color:var(--text-secondary);">
            {intg.setup}
          </div>
        </div>
      {/each}
    </div>

    <!-- Shared Resources table -->
    <div class="card">
      <h3>Shared Resources</h3>
      <table class="data-table">
        <thead>
          <tr>
            <th>Resource</th>
            <th>Claude Code</th>
            <th>Codex</th>
            <th>Gemini</th>
            <th>Cursor</th>
          </tr>
        </thead>
        <tbody>
          <tr><td>Commands</td><td>8</td><td>8</td><td>8</td><td>8</td></tr>
          <tr><td>Skills</td><td>12</td><td>7</td><td>7</td><td>—</td></tr>
          <tr><td>Agents</td><td>4</td><td>4</td><td>4</td><td>4</td></tr>
          <tr><td>Hooks</td><td>6</td><td>1</td><td>—</td><td>1</td></tr>
        </tbody>
      </table>
    </div>
  {/if}
</div>
