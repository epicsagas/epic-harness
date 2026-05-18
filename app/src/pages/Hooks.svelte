<div class="screen-header">
  <h2>Hooks <span class="subtitle-tag">Ring 0</span></h2>
  <p>6 Claude Code hooks providing autopilot automation &middot; Rust single binary</p>
</div>

<div class="panel">
  <div class="panel-header"><h3>Hook Registry</h3></div>
  <div class="panel-body">
    <div class="hook-row">
      <div class="hook-event" style="color:var(--success);">Session start</div>
      <div class="hook-cmd"><code>epic-harness resume</code></div>
      <div class="hook-desc">Restore session + load evolved skills</div>
      <span class="pill success">active</span>
    </div>
    <div class="hook-row">
      <div class="hook-event" style="color:var(--accent);">Pre tool use</div>
      <div class="hook-cmd"><code>epic-harness guard</code></div>
      <div class="hook-desc">Block dangerous shell patterns</div>
      <span class="pill success">active</span>
    </div>
    <div class="hook-row">
      <div class="hook-event" style="color:var(--purple);">Post tool use</div>
      <div class="hook-cmd"><code>epic-harness observe</code></div>
      <div class="hook-desc">Record tool scores (3-axis eval, async)</div>
      <span class="pill success">active</span>
    </div>
    <div class="hook-row">
      <div class="hook-event" style="color:var(--teal);">Post edit</div>
      <div class="hook-cmd"><code>epic-harness polish</code></div>
      <div class="hook-desc">Auto-format + typecheck (feeds into observe)</div>
      <span class="pill success">active</span>
    </div>
    <div class="hook-row">
      <div class="hook-event" style="color:var(--warning);">Pre compact</div>
      <div class="hook-cmd"><code>epic-harness snapshot</code></div>
      <div class="hook-desc">Save session state before compaction</div>
      <span class="pill success">active</span>
    </div>
    <div class="hook-row">
      <div class="hook-event" style="color:var(--danger);">Session end</div>
      <div class="hook-cmd"><code>epic-harness reflect</code></div>
      <div class="hook-desc">Evolve skills + save metrics + merge session obs</div>
      <span class="pill success">active</span>
    </div>
  </div>
</div>

<!-- Guard Rules -->
<div class="panel" style="margin-top:16px;">
  <div class="panel-header"><h3>Guard Rule Extension</h3></div>
  <div class="panel-body">
    <div style="background:var(--bg);border-radius:var(--radius);padding:14px;font-family:var(--font-mono);font-size:11px;line-height:1.8;color:var(--fg-secondary);">
      <div><span style="color:var(--muted)"># .harness/guard-rules.yaml</span></div>
      <div><span style="color:var(--danger)">blocked</span>:</div>
      <div>  <span style="color:var(--accent)">- pattern</span>: <span style="color:var(--teal)">kubectl\s+delete</span></div>
      <div>    <span style="color:var(--accent)">msg</span>: <span style="color:var(--teal)">kubectl delete blocked</span></div>
      <div><span style="color:var(--warning)">warned</span>:</div>
      <div>  <span style="color:var(--accent)">- pattern</span>: <span style="color:var(--teal)">docker\s+system\s+prune</span></div>
      <div>    <span style="color:var(--accent)">msg</span>: <span style="color:var(--teal)">Docker prune &mdash; check first</span></div>
    </div>
  </div>
</div>
