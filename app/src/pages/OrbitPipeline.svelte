<div class="screen-header">
  <h2>/orbit <span class="subtitle-tag">Autonomous Pipeline</span></h2>
  <p>spec &#8594; go &#8594; check &#8594; ship &#8594; evolve &mdash; single-command spec-to-PR execution</p>
</div>

<!-- Mode Selection -->
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

<!-- Pipeline Flow -->
<div class="panel">
  <div class="panel-header"><h3>Pipeline Flow</h3></div>
  <div class="panel-body">
    <div class="pipeline-flow">
      <div class="pipeline-stage completed">
        <div class="stage-icon" style="background:var(--accent-soft);border-color:var(--accent);color:var(--accent);">&#9776;</div>
        <div class="stage-label">spec</div>
        <div class="stage-sub">REQ + AC</div>
      </div>
      <div class="pipeline-arrow done"></div>
      <div class="pipeline-stage completed">
        <div class="stage-icon" style="background:var(--accent-soft);border-color:var(--accent);color:var(--accent);">&#9654;</div>
        <div class="stage-label">go</div>
        <div class="stage-sub">plan+TDD</div>
      </div>
      <div class="pipeline-arrow done"></div>
      <div class="pipeline-stage running">
        <div class="stage-icon" style="background:var(--success-soft);border-color:var(--success);color:var(--success);">&#10003;</div>
        <div class="stage-label">check</div>
        <div class="stage-sub">review+audit</div>
      </div>
      <div class="pipeline-arrow"></div>
      <div class="pipeline-stage pending">
        <div class="stage-icon">&#128640;</div>
        <div class="stage-label">ship</div>
        <div class="stage-sub">PR+CI</div>
      </div>
      <div class="pipeline-arrow"></div>
      <div class="pipeline-stage pending">
        <div class="stage-icon">&#8635;</div>
        <div class="stage-label">evolve</div>
        <div class="stage-sub">auto-analyze</div>
      </div>
    </div>
    <div style="margin-top:12px;font-size:11px;color:var(--muted);font-family:var(--font-mono);display:flex;gap:24px;">
      <span>check fail &#8594; auto-retry (max 3)</span>
      <span>3 fails &#8594; pause for human</span>
      <span>state: <code>$HARNESS_DIR/orbit/PIPELINE-*.json</code></span>
    </div>
  </div>
</div>

<!-- Orbit State Tracking -->
<div class="grid-2" style="margin-top:16px;">
  <div class="panel">
    <div class="panel-header"><h3>Pipeline State Schema</h3></div>
    <div class="panel-body" style="padding:0;">
      <table class="data-table">
        <thead><tr><th>Field</th><th>Value</th></tr></thead>
        <tbody>
          <tr><td style="color:var(--muted)">id</td><td style="color:var(--fg)">{timestamp}</td></tr>
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
