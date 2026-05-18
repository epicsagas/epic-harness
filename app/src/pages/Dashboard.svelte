<div class="screen-header">
  <h2>Dashboard</h2>
  <p>4-Ring architecture status &middot; eval scores &middot; system health</p>
</div>

<!-- 4-Ring Status -->
<div class="grid-4" style="margin-bottom:24px;">
  <div class="ring-badge">
    <div class="ring-num" style="background:var(--success-soft);color:var(--success);">0</div>
    <div class="ring-info">
      <div class="ring-title">Autopilot</div>
      <div class="ring-desc">6 hooks active</div>
    </div>
  </div>
  <div class="ring-badge">
    <div class="ring-num" style="background:var(--accent-soft);color:var(--accent);">1</div>
    <div class="ring-info">
      <div class="ring-title">Commands</div>
      <div class="ring-desc">10 user commands</div>
    </div>
  </div>
  <div class="ring-badge">
    <div class="ring-num" style="background:var(--purple-soft);color:var(--purple);">2</div>
    <div class="ring-info">
      <div class="ring-title">Auto Skills</div>
      <div class="ring-desc">15 context-triggered</div>
    </div>
  </div>
  <div class="ring-badge">
    <div class="ring-num" style="background:var(--orange-soft);color:var(--orange);">3</div>
    <div class="ring-info">
      <div class="ring-title">Evolution</div>
      <div class="ring-desc">observe &#8594; evolve loop</div>
    </div>
  </div>
</div>

<!-- Key Metrics -->
<div class="stats-grid">
  <div class="stat-card">
    <div class="stat-label"><span class="dot" style="background:var(--accent)"></span> Eval Score</div>
    <div class="stat-value">0.78</div>
    <div class="stat-sub">composite (success 50% + quality 30% + cost 20%)</div>
  </div>
  <div class="stat-card">
    <div class="stat-label"><span class="dot" style="background:var(--purple)"></span> Evolved Skills</div>
    <div class="stat-value">3</div>
    <div class="stat-sub">max 10 &middot; stagnation limit 3</div>
  </div>
  <div class="stat-card">
    <div class="stat-label"><span class="dot" style="background:var(--teal)"></span> Memory Nodes</div>
    <div class="stat-value">--</div>
    <div class="stat-sub">SQLite + FTS5 via harness-mem MCP</div>
  </div>
  <div class="stat-card">
    <div class="stat-label"><span class="dot" style="background:var(--success)"></span> Sessions</div>
    <div class="stat-value">--</div>
    <div class="stat-sub">this project</div>
  </div>
  <div class="stat-card">
    <div class="stat-label"><span class="dot" style="background:var(--warning)"></span> Orbit Runs</div>
    <div class="stat-value">--</div>
    <div class="stat-sub">spec &#8594; go &#8594; check &#8594; ship</div>
  </div>
  <div class="stat-card">
    <div class="stat-label"><span class="dot" style="background:var(--cyan)"></span> Integrations</div>
    <div class="stat-value">6</div>
    <div class="stat-sub">Claude Code &middot; Codex &middot; Cursor &middot; Gemini &middot; Cline &middot; Aider</div>
  </div>
</div>

<!-- Eval Axes + Pattern Detection -->
<div class="grid-2" style="margin-bottom:16px;">
  <div class="panel">
    <div class="panel-header">
      <h3>Eval Score Axes (3-Axis)</h3>
      <div class="panel-actions">
        <button class="chip active">Current</button>
        <button class="chip">Trend</button>
      </div>
    </div>
    <div class="panel-body">
      <div style="display:flex;gap:32px;align-items:center;justify-content:center;">
        <div class="usage-ring-container">
          <svg width="90" height="90" viewBox="0 0 100 100">
            <circle cx="50" cy="50" r="42" fill="none" stroke="var(--border)" stroke-width="6"/>
            <circle cx="50" cy="50" r="42" fill="none" stroke="var(--accent)" stroke-width="6"
                    stroke-dasharray="264" stroke-dashoffset="40" stroke-linecap="round"
                    transform="rotate(-90 50 50)"/>
          </svg>
          <div class="usage-ring-label">
            <div class="ring-value">0.85</div>
            <div class="ring-text">Success</div>
          </div>
        </div>
        <div class="usage-ring-container">
          <svg width="90" height="90" viewBox="0 0 100 100">
            <circle cx="50" cy="50" r="42" fill="none" stroke="var(--border)" stroke-width="6"/>
            <circle cx="50" cy="50" r="42" fill="none" stroke="var(--purple)" stroke-width="6"
                    stroke-dasharray="264" stroke-dashoffset="106" stroke-linecap="round"
                    transform="rotate(-90 50 50)"/>
          </svg>
          <div class="usage-ring-label">
            <div class="ring-value">0.60</div>
            <div class="ring-text">Quality</div>
          </div>
        </div>
        <div class="usage-ring-container">
          <svg width="90" height="90" viewBox="0 0 100 100">
            <circle cx="50" cy="50" r="42" fill="none" stroke="var(--border)" stroke-width="6"/>
            <circle cx="50" cy="50" r="42" fill="none" stroke="var(--teal)" stroke-width="6"
                    stroke-dasharray="264" stroke-dashoffset="145" stroke-linecap="round"
                    transform="rotate(-90 50 50)"/>
          </svg>
          <div class="usage-ring-label">
            <div class="ring-value">0.45</div>
            <div class="ring-text">Cost</div>
          </div>
        </div>
      </div>
      <div style="margin-top:16px;font-size:11px;color:var(--muted);text-align:center;font-family:var(--font-mono);">
        composite = 0.5 &times; success + 0.3 &times; quality + 0.2 &times; cost = <strong style="color:var(--fg)">0.78</strong>
      </div>
    </div>
  </div>

  <div class="panel">
    <div class="panel-header">
      <h3>Pattern Detection (4 Types)</h3>
    </div>
    <div class="panel-body" style="padding:0;">
      <table class="data-table">
        <thead>
          <tr><th>Pattern</th><th>Threshold</th><th>Status</th></tr>
        </thead>
        <tbody>
          <tr>
            <td style="color:var(--fg)">repeated_same_error</td>
            <td>min 3 consecutive</td>
            <td><span class="pill success">clear</span></td>
          </tr>
          <tr>
            <td style="color:var(--fg)">fix_then_break</td>
            <td>lookahead 3, min 2 cycles</td>
            <td><span class="pill success">clear</span></td>
          </tr>
          <tr>
            <td style="color:var(--fg)">long_debug_loop</td>
            <td>min 5 same file</td>
            <td><span class="pill success">clear</span></td>
          </tr>
          <tr>
            <td style="color:var(--fg)">thrashing</td>
            <td>min 3 edits + 3 errors</td>
            <td><span class="pill success">clear</span></td>
          </tr>
        </tbody>
      </table>
    </div>
  </div>
</div>

<!-- Recent Activity -->
<div class="panel">
  <div class="panel-header">
    <h3>Recent Activity</h3>
  </div>
  <div class="panel-body">
    <ul class="activity-list">
      <li class="activity-item">
        <span class="activity-dot" style="background:var(--success)"></span>
        <div class="activity-content">
          <div class="activity-title">Session restored via <code>epic-harness resume</code></div>
          <div class="activity-time">session start</div>
        </div>
      </li>
      <li class="activity-item">
        <span class="activity-dot" style="background:var(--accent)"></span>
        <div class="activity-content">
          <div class="activity-title">Evolved skills loaded (3 active)</div>
          <div class="activity-time">$HARNESS_DIR/evolved/</div>
        </div>
      </li>
      <li class="activity-item">
        <span class="activity-dot" style="background:var(--purple)"></span>
        <div class="activity-content">
          <div class="activity-title">harness-mem context loaded for project</div>
          <div class="activity-time">session start</div>
        </div>
      </li>
      <li class="activity-item">
        <span class="activity-dot" style="background:var(--warning)"></span>
        <div class="activity-content">
          <div class="activity-title">Polish hook ran: format + typecheck</div>
          <div class="activity-time">post-edit</div>
        </div>
      </li>
      <li class="activity-item">
        <span class="activity-dot" style="background:var(--teal)"></span>
        <div class="activity-content">
          <div class="activity-title">Observe hook recorded tool score</div>
          <div class="activity-time">post-tool-use</div>
        </div>
      </li>
    </ul>
  </div>
</div>
