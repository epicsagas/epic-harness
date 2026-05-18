<div class="page">
  <div class="page-header">
    <h1>Hooks <span class="badge">Ring 0</span></h1>
    <p>6 Claude Code hooks providing autopilot automation · Rust single binary</p>
  </div>

  <!-- Hook Registry table -->
  <div class="card" style="margin-bottom:1rem;">
    <h3>Hook Registry</h3>
    <table class="data-table">
      <thead>
        <tr>
          <th>Hook</th>
          <th>Command</th>
          <th>Trigger</th>
          <th>Effect</th>
        </tr>
      </thead>
      <tbody>
        <tr>
          <td style="color:var(--success, #22c55e); white-space:nowrap;">Session Start</td>
          <td><code>epic-harness resume</code></td>
          <td style="color:var(--text-secondary); font-size:0.82rem;">세션 시작 시</td>
          <td style="color:var(--text-secondary); font-size:0.82rem;">Restore session + load evolved skills</td>
        </tr>
        <tr>
          <td style="color:var(--accent, #6366f1); white-space:nowrap;">Pre Tool Use</td>
          <td><code>epic-harness guard</code></td>
          <td style="color:var(--text-secondary); font-size:0.82rem;">모든 툴 호출 전</td>
          <td style="color:var(--text-secondary); font-size:0.82rem;">Block dangerous shell patterns</td>
        </tr>
        <tr>
          <td style="color:var(--purple, #a855f7); white-space:nowrap;">Post Tool Use</td>
          <td><code>epic-harness observe</code></td>
          <td style="color:var(--text-secondary); font-size:0.82rem;">툴 호출 후 (async)</td>
          <td style="color:var(--text-secondary); font-size:0.82rem;">Record 3-axis scores to obs JSONL</td>
        </tr>
        <tr>
          <td style="color:var(--teal, #06b6d4); white-space:nowrap;">Post Edit</td>
          <td><code>epic-harness polish</code></td>
          <td style="color:var(--text-secondary); font-size:0.82rem;">파일 편집 후</td>
          <td style="color:var(--text-secondary); font-size:0.82rem;">Auto-format + typecheck</td>
        </tr>
        <tr>
          <td style="color:var(--warning, #f59e0b); white-space:nowrap;">Pre Compact</td>
          <td><code>epic-harness snapshot</code></td>
          <td style="color:var(--text-secondary); font-size:0.82rem;">컨텍스트 압축 전</td>
          <td style="color:var(--text-secondary); font-size:0.82rem;">Save session state to sessions/</td>
        </tr>
        <tr>
          <td style="color:var(--danger, #ef4444); white-space:nowrap;">Session End</td>
          <td><code>epic-harness reflect</code></td>
          <td style="color:var(--text-secondary); font-size:0.82rem;">세션 종료 시</td>
          <td style="color:var(--text-secondary); font-size:0.82rem;">Evolve skills + save metrics</td>
        </tr>
      </tbody>
    </table>
  </div>

  <!-- Hook Flow -->
  <div class="card" style="margin-bottom:1rem;">
    <h3>Hook Flow</h3>
    <pre style="font-family:monospace; font-size:0.82rem; line-height:1.8; color:var(--text-secondary); margin:0; overflow-x:auto;">Session Start  →  [resume]    →  Load Skills
      |
 Tool Call     →  [guard]     →  Block/Allow  →  Execute  →  [observe]  →  Score
      |
 File Edit     →  [polish]    →  Format + Typecheck
      |
 Pre Compact   →  [snapshot]  →  Save State
      |
 Session End   →  [reflect]   →  Evolve + Metrics</pre>
  </div>

  <!-- Guard Rules -->
  <div class="card" style="margin-bottom:1rem;">
    <h3>Guard Rules Extension</h3>
    <p style="font-size:0.82rem; color:var(--text-secondary); margin-bottom:0.75rem;">
      Add custom block/warn rules via <code>.harness/guard-rules.yaml</code> in your project root.
    </p>
    <pre style="font-family:monospace; font-size:0.8rem; line-height:1.8; padding:0.75rem; background:var(--bg); border-radius:4px; overflow-x:auto; margin:0;"><span style="color:var(--text-secondary);"># .harness/guard-rules.yaml</span>
<span style="color:var(--danger, #ef4444);">blocked</span>:
  - <span style="color:var(--accent, #6366f1);">pattern</span>: <span style="color:var(--success, #22c55e);">kubectl\s+delete</span>
    <span style="color:var(--accent, #6366f1);">msg</span>: <span style="color:var(--success, #22c55e);">kubectl delete blocked — confirm with user first</span>
  - <span style="color:var(--accent, #6366f1);">pattern</span>: <span style="color:var(--success, #22c55e);">rm\s+-rf\s+/</span>
    <span style="color:var(--accent, #6366f1);">msg</span>: <span style="color:var(--success, #22c55e);">Dangerous rm blocked</span>
<span style="color:var(--warning, #f59e0b);">warned</span>:
  - <span style="color:var(--accent, #6366f1);">pattern</span>: <span style="color:var(--success, #22c55e);">docker\s+system\s+prune</span>
    <span style="color:var(--accent, #6366f1);">msg</span>: <span style="color:var(--success, #22c55e);">Docker prune — verify intent</span>
  - <span style="color:var(--accent, #6366f1);">pattern</span>: <span style="color:var(--success, #22c55e);">git\s+push\s+--force</span>
    <span style="color:var(--accent, #6366f1);">msg</span>: <span style="color:var(--success, #22c55e);">Force push — confirm with team</span></pre>
  </div>

  <!-- Polish → Observe feedback loop -->
  <div class="card">
    <h3>Polish → Observe Feedback</h3>
    <p style="font-size:0.82rem; color:var(--text-secondary); margin-bottom:0.75rem;">
      Polish hook results auto-record into the observe pipeline.
    </p>
    <table class="data-table">
      <thead>
        <tr><th>Polish result</th><th>Failure type recorded</th><th>Pattern detection</th></tr>
      </thead>
      <tbody>
        <tr>
          <td>Format failure</td>
          <td><code>lint_fail</code></td>
          <td style="color:var(--text-secondary); font-size:0.82rem;">Feeds repeated_same_error detector</td>
        </tr>
        <tr>
          <td>Typecheck failure</td>
          <td><code>build_fail</code></td>
          <td style="color:var(--text-secondary); font-size:0.82rem;">Feeds fix_then_break detector</td>
        </tr>
      </tbody>
    </table>
  </div>
</div>
