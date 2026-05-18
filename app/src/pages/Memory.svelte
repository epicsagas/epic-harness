<div class="screen-header">
  <h2>harness-mem <span class="subtitle-tag">WIP</span></h2>
  <p>Unified knowledge graph &middot; SQLite + FTS5 &middot; MCP server registered as <code>harness-mem</code></p>
</div>

<!-- Memory Schema -->
<div class="grid-2" style="margin-bottom:16px;">
  <div class="panel">
    <div class="panel-header"><h3>MCP Tools (6)</h3></div>
    <div class="panel-body" style="padding:0;">
      <table class="data-table">
        <thead><tr><th>Tool</th><th>Purpose</th></tr></thead>
        <tbody>
          <tr>
            <td style="color:var(--accent)">mem_recall</td>
            <td style="color:var(--fg-secondary)">Smart contextual recall &middot; hint + project + graph neighbors</td>
          </tr>
          <tr>
            <td style="color:var(--accent)">mem_add</td>
            <td style="color:var(--fg-secondary)">Add node with auto-importance by type</td>
          </tr>
          <tr>
            <td style="color:var(--accent)">mem_search</td>
            <td style="color:var(--fg-secondary)">FTS5 keyword search, ranked by importance</td>
          </tr>
          <tr>
            <td style="color:var(--accent)">mem_query</td>
            <td style="color:var(--fg-secondary)">Filter by tag/type/project</td>
          </tr>
          <tr>
            <td style="color:var(--accent)">mem_context</td>
            <td style="color:var(--fg-secondary)">Project-scoped smart recall (session start)</td>
          </tr>
          <tr>
            <td style="color:var(--accent)">mem_related</td>
            <td style="color:var(--fg-secondary)">BFS graph traversal from node ID</td>
          </tr>
        </tbody>
      </table>
    </div>
  </div>

  <div class="panel">
    <div class="panel-header"><h3>Scoring Formula</h3></div>
    <div class="panel-body">
      <div style="font-family:var(--font-mono);font-size:12px;line-height:2.2;">
        <div style="color:var(--fg-secondary);">recency(25%) + importance(35%) + access_freq(15%) + FTS_match(25%)</div>
        <div style="margin-top:12px;">
          <div style="display:flex;align-items:center;gap:8px;">
            <span style="color:var(--muted);width:120px;">recency:</span>
            <span style="color:var(--fg)">exponential decay, 30-day half-life</span>
          </div>
          <div style="display:flex;align-items:center;gap:8px;">
            <span style="color:var(--muted);width:120px;">importance:</span>
            <span style="color:var(--fg)">type-based (0.0-1.0)</span>
          </div>
          <div style="display:flex;align-items:center;gap:8px;">
            <span style="color:var(--muted);width:120px;">access_freq:</span>
            <span style="color:var(--fg)">saturates at 20 accesses</span>
          </div>
          <div style="display:flex;align-items:center;gap:8px;">
            <span style="color:var(--muted);width:120px;">FTS_match:</span>
            <span style="color:var(--fg)">1.0 bonus via FTS5</span>
          </div>
        </div>
      </div>
    </div>
  </div>
</div>

<!-- Node Types -->
<div class="panel" style="margin-bottom:16px;">
  <div class="panel-header"><h3>Node Types &amp; Default Importance</h3></div>
  <div class="panel-body">
    <div style="display:flex;gap:8px;flex-wrap:wrap;">
      <span class="pill" style="background:var(--danger-soft);color:var(--danger);font-size:12px;padding:4px 12px;">decision (0.9)</span>
      <span class="pill" style="background:var(--orange-soft);color:var(--orange);font-size:12px;padding:4px 12px;">resolution (0.8)</span>
      <span class="pill" style="background:var(--accent-soft);color:var(--accent);font-size:12px;padding:4px 12px;">concept (0.7)</span>
      <span class="pill" style="background:var(--teal-soft);color:var(--teal);font-size:12px;padding:4px 12px;">project (0.7)</span>
      <span class="pill" style="background:var(--purple-soft);color:var(--purple);font-size:12px;padding:4px 12px;">pattern (0.5)</span>
      <span class="pill" style="background:var(--warning-soft);color:var(--warning);font-size:12px;padding:4px 12px;">error (0.4)</span>
      <span class="pill" style="background:var(--fg);color:var(--bg);font-size:12px;padding:4px 12px;">session (0.2)</span>
    </div>
    <div style="margin-top:12px;font-size:11px;color:var(--muted);font-family:var(--font-mono);">
      lifecycle: access tracking &#8594; 30d decay (-10%/cycle) &#8594; 180d stale &#8594; <code>pinned</code> tag prevents decay
    </div>
  </div>
</div>

<!-- Storage -->
<div class="panel">
  <div class="panel-header"><h3>Storage</h3></div>
  <div class="panel-body" style="padding:0;">
    <table class="data-table">
      <tbody>
        <tr><td style="color:var(--muted)">Database</td><td style="color:var(--fg)">~/.harness/memory.db (SQLite + FTS5)</td></tr>
        <tr><td style="color:var(--muted)">MCP Server</td><td style="color:var(--fg)">harness-mem (registered in Claude Code)</td></tr>
        <tr><td style="color:var(--muted)">Integration</td><td style="color:var(--fg)">_dispatch calls mem_recall before invoking any skill</td></tr>
        <tr><td style="color:var(--muted)">Status</td><td style="color:var(--warning)">WIP &mdash; under active development</td></tr>
      </tbody>
    </table>
  </div>
</div>
