<div class="screen-header" style="display:flex;justify-content:space-between;align-items:flex-start;">
  <div>
    <h2>Settings</h2>
    <p>Review and edit epic-harness configurations</p>
  </div>
</div>

<div class="grid-2-1">
  <div>
    <!-- Eval Weights -->
    <div class="panel">
      <div class="panel-header"><h3>Eval Score Weights</h3></div>
      <div class="panel-body">
        <div class="settings-group">
          <div class="setting-row">
            <div class="setting-info">
              <div class="setting-name">Success Weight</div>
              <div class="setting-desc">tool_success axis weight (0/1)</div>
            </div>
            <div class="setting-control">
              <input class="input-field" type="number" value="0.5" step="0.1" min="0" max="1" style="width:100px;" />
            </div>
          </div>
          <div class="setting-row">
            <div class="setting-info">
              <div class="setting-name">Quality Weight</div>
              <div class="setting-desc">output_quality axis weight (0.0-1.0)</div>
            </div>
            <div class="setting-control">
              <input class="input-field" type="number" value="0.3" step="0.1" min="0" max="1" style="width:100px;" />
            </div>
          </div>
          <div class="setting-row">
            <div class="setting-info">
              <div class="setting-name">Cost Weight</div>
              <div class="setting-desc">execution_cost axis weight (0.0-1.0)</div>
            </div>
            <div class="setting-control">
              <input class="input-field" type="number" value="0.2" step="0.1" min="0" max="1" style="width:100px;" />
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- Evolution -->
    <div class="panel" style="margin-top:16px;">
      <div class="panel-header"><h3>Evolution Tuning</h3></div>
      <div class="panel-body">
        <div class="settings-group">
          <div class="setting-row">
            <div class="setting-info">
              <div class="setting-name">Stagnation Limit</div>
              <div class="setting-desc">Sessions without improvement before rollback</div>
            </div>
            <div class="setting-control">
              <input class="input-field" type="number" value="3" style="width:100px;" />
            </div>
          </div>
          <div class="setting-row">
            <div class="setting-info">
              <div class="setting-name">Improvement Threshold</div>
              <div class="setting-desc">Minimum score improvement to count as progress</div>
            </div>
            <div class="setting-control">
              <input class="input-field" type="number" value="5" style="width:100px;" />
              <span style="font-size:11px;color:var(--muted);margin-left:4px;">%</span>
            </div>
          </div>
          <div class="setting-row">
            <div class="setting-info">
              <div class="setting-name">Max Evolved Skills</div>
              <div class="setting-desc">Upper limit for auto-generated skills</div>
            </div>
            <div class="setting-control">
              <input class="input-field" type="number" value="10" style="width:100px;" />
            </div>
          </div>
          <div class="setting-row">
            <div class="setting-info">
              <div class="setting-name">Cross-Project Learning</div>
              <div class="setting-desc">Share patterns across projects (opt-in)</div>
            </div>
            <div class="setting-control">
              <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
              <div class="toggle" role="switch" tabindex="0" onclick={(e) => e.currentTarget.classList.toggle('on')}></div>
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- Pattern Detection -->
    <div class="panel" style="margin-top:16px;">
      <div class="panel-header"><h3>Pattern Detection Thresholds</h3></div>
      <div class="panel-body">
        <div class="settings-group">
          <div class="setting-row">
            <div class="setting-info">
              <div class="setting-name">Repeated Error Min</div>
              <div class="setting-desc">Consecutive same error + same hash threshold</div>
            </div>
            <div class="setting-control">
              <input class="input-field" type="number" value="3" style="width:100px;" />
            </div>
          </div>
          <div class="setting-row">
            <div class="setting-info">
              <div class="setting-name">Debug Loop Min</div>
              <div class="setting-desc">Same file in consecutive operations threshold</div>
            </div>
            <div class="setting-control">
              <input class="input-field" type="number" value="5" style="width:100px;" />
            </div>
          </div>
          <div class="setting-row">
            <div class="setting-info">
              <div class="setting-name">Thrashing Min Edits</div>
              <div class="setting-desc">Edit&#8596;Error alternating minimum edit count</div>
            </div>
            <div class="setting-control">
              <input class="input-field" type="number" value="3" style="width:100px;" />
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>

  <!-- Right Column -->
  <div>
    <!-- System Info -->
    <div class="panel">
      <div class="panel-header"><h3>System Info</h3></div>
      <div class="panel-body" style="padding:0;">
        <table class="data-table">
          <tbody>
            <tr><td style="color:var(--muted)">Version</td><td style="color:var(--fg)">0.3.11</td></tr>
            <tr><td style="color:var(--muted)">Runtime</td><td style="color:var(--fg)">Rust (edition 2024)</td></tr>
            <tr><td style="color:var(--muted)">Binary</td><td style="color:var(--fg)">hooks/bin/epic-harness</td></tr>
            <tr><td style="color:var(--muted)">Memory DB</td><td style="color:var(--fg)">SQLite + FTS5</td></tr>
            <tr><td style="color:var(--muted)">Dashboard</td><td style="color:var(--fg)">Tauri (src-tauri/)</td></tr>
            <tr><td style="color:var(--muted)">License</td><td style="color:var(--fg)">Apache-2.0</td></tr>
            <tr><td style="color:var(--muted)">CI</td><td style="color:var(--fg)">check + clippy + fmt + test</td></tr>
            <tr><td style="color:var(--muted)">Release</td><td style="color:var(--fg)">cargo-dist</td></tr>
          </tbody>
        </table>
      </div>
    </div>

    <!-- Project Paths -->
    <div class="panel" style="margin-top:16px;">
      <div class="panel-header"><h3>Project Data Paths</h3></div>
      <div class="panel-body" style="padding:0;">
        <table class="data-table">
          <tbody>
            <tr><td style="color:var(--muted)">HARNESS_DIR</td><td style="color:var(--fg);font-size:10px;">~/.harness/projects/{slug}/</td></tr>
            <tr><td style="color:var(--muted)">Observations</td><td style="color:var(--fg);font-size:10px;">obs/ (JSONL, 3-axis scores)</td></tr>
            <tr><td style="color:var(--muted)">Evolved Skills</td><td style="color:var(--fg);font-size:10px;">evolved/ (auto-generated)</td></tr>
            <tr><td style="color:var(--muted)">Evolution Backup</td><td style="color:var(--fg);font-size:10px;">evolved_backup/ (best state)</td></tr>
            <tr><td style="color:var(--muted)">Sessions</td><td style="color:var(--fg);font-size:10px;">sessions/*.json</td></tr>
            <tr><td style="color:var(--muted)">Metrics</td><td style="color:var(--fg);font-size:10px;">metrics.json</td></tr>
            <tr><td style="color:var(--muted)">Orbit State</td><td style="color:var(--fg);font-size:10px;">orbit/PIPELINE-*.json</td></tr>
            <tr><td style="color:var(--muted)">Dispatch Log</td><td style="color:var(--fg);font-size:10px;">dispatch/dispatch_*.jsonl</td></tr>
            <tr><td style="color:var(--muted)">Memory</td><td style="color:var(--fg);font-size:10px;">memory/ (project patterns)</td></tr>
          </tbody>
        </table>
      </div>
    </div>

    <!-- Cold Start Presets -->
    <div class="panel" style="margin-top:16px;">
      <div class="panel-header"><h3>Cold-Start Presets</h3></div>
      <div class="panel-body">
        <div style="display:flex;gap:8px;flex-wrap:wrap;">
          <span class="pill info">Rust</span>
          <span class="pill success">Node.js</span>
          <span class="pill warning">Python</span>
          <span class="pill purple">Go</span>
        </div>
        <div style="margin-top:10px;font-size:11px;color:var(--muted);">
          Auto-apply on first session with no evolved skills for detected stacks.
        </div>
      </div>
    </div>
  </div>
</div>
