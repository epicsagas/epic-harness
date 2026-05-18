<div class="screen-header">
  <h2>Eval &amp; Evolve <span class="subtitle-tag">Ring 3</span></h2>
  <p>Observe &#8594; Analyze &#8594; Evolve &#8594; Gate &#8594; Reload self-improvement loop</p>
</div>

<!-- Evolution Pipeline -->
<div class="panel" style="margin-bottom:16px;">
  <div class="panel-header"><h3>Evolution Loop</h3></div>
  <div class="panel-body">
    <div class="pipeline-flow" style="justify-content:center;">
      <div class="pipeline-stage completed">
        <div class="stage-icon" style="background:var(--teal-soft);border-color:var(--teal);color:var(--teal);font-size:14px;">&#128065;</div>
        <div class="stage-label">Observe</div>
        <div class="stage-sub">post-tool-use</div>
      </div>
      <div class="pipeline-arrow done"></div>
      <div class="pipeline-stage completed">
        <div class="stage-icon" style="background:var(--accent-soft);border-color:var(--accent);color:var(--accent);font-size:14px;">&#128200;</div>
        <div class="stage-label">Analyze</div>
        <div class="stage-sub">session end</div>
      </div>
      <div class="pipeline-arrow done"></div>
      <div class="pipeline-stage running">
        <div class="stage-icon" style="background:var(--orange-soft);border-color:var(--orange);color:var(--orange);font-size:14px;">&#128736;</div>
        <div class="stage-label">Evolve</div>
        <div class="stage-sub">gen skills</div>
      </div>
      <div class="pipeline-arrow"></div>
      <div class="pipeline-stage pending">
        <div class="stage-icon" style="font-size:14px;">&#10003;</div>
        <div class="stage-label">Gate</div>
        <div class="stage-sub">validate</div>
      </div>
      <div class="pipeline-arrow"></div>
      <div class="pipeline-stage pending">
        <div class="stage-icon" style="font-size:14px;">&#8635;</div>
        <div class="stage-label">Reload</div>
        <div class="stage-sub">next session</div>
      </div>
    </div>
  </div>
</div>

<!-- Skill Seeding Thresholds -->
<div class="grid-2" style="margin-bottom:16px;">
  <div class="panel">
    <div class="panel-header"><h3>Skill Seeding Thresholds</h3></div>
    <div class="panel-body" style="padding:0;">
      <table class="data-table">
        <thead><tr><th>Type</th><th>Condition</th><th>Min Observations</th></tr></thead>
        <tbody>
          <tr>
            <td style="color:var(--fg)">Weak tool</td>
            <td>success rate &lt; 0.6</td>
            <td>5</td>
          </tr>
          <tr>
            <td style="color:var(--fg)">Weak file type</td>
            <td>success rate &lt; 0.5</td>
            <td>3</td>
          </tr>
          <tr>
            <td style="color:var(--fg)">High-frequency error</td>
            <td>5+ occurrences</td>
            <td>5</td>
          </tr>
        </tbody>
      </table>
    </div>
  </div>
  <div class="panel">
    <div class="panel-header"><h3>Stagnation &amp; Gating</h3></div>
    <div class="panel-body">
      <ul class="activity-list">
        <li class="activity-item">
          <span class="activity-dot" style="background:var(--warning)"></span>
          <div class="activity-content">
            <div class="activity-title">Stagnation limit: 3 sessions</div>
            <div class="activity-time">No improvement &#8594; auto-rollback to best checkpoint</div>
          </div>
        </li>
        <li class="activity-item">
          <span class="activity-dot" style="background:var(--accent)"></span>
          <div class="activity-content">
            <div class="activity-title">Improvement threshold: 5%</div>
            <div class="activity-time">Trend: improving / stable / declining</div>
          </div>
        </li>
        <li class="activity-item">
          <span class="activity-dot" style="background:var(--success)"></span>
          <div class="activity-content">
            <div class="activity-title">Max evolved skills: 10</div>
            <div class="activity-time">Static skills always take priority</div>
          </div>
        </li>
        <li class="activity-item">
          <span class="activity-dot" style="background:var(--purple)"></span>
          <div class="activity-content">
            <div class="activity-title">Validation: frontmatter + body &ge;20 chars</div>
            <div class="activity-time">Invalid skills silently removed</div>
          </div>
        </li>
      </ul>
    </div>
  </div>
</div>

<!-- Skill Attribution -->
<div class="panel">
  <div class="panel-header"><h3>Skill Attribution (A/B Tracking)</h3></div>
  <div class="panel-body">
    <div style="font-size:12px;color:var(--fg-secondary);margin-bottom:12px;font-family:var(--font-mono);">
      metrics.json tracks per-evolved-skill: avg_score_with vs avg_score_without &middot; positive delta = effective
    </div>
    <div style="display:flex;gap:16px;font-size:11px;color:var(--muted);font-family:var(--font-mono);">
      <span>data: <code>$HARNESS_DIR/metrics.json</code></span>
      <span>history: <code>$HARNESS_DIR/evolution.jsonl</code></span>
      <span>backup: <code>$HARNESS_DIR/evolved_backup/</code></span>
    </div>
  </div>
</div>
