<script lang="ts">
  import { tStore } from '$lib/i18n.js';
</script>

<div class="page">
  <div class="page-header">
    <h1>{$tStore('pageHooks')} <span class="badge">Ring 0</span></h1>
    <p>{$tStore('pageHooksDesc')}</p>
  </div>

  <!-- Hook Registry table -->
  <div class="card" style="margin-bottom:1rem;">
    <h3>{$tStore('hookRegistryTitle')}</h3>
    <table class="data-table">
      <thead>
        <tr>
          <th>{$tStore('colHook')}</th>
          <th>{$tStore('colCommand')}</th>
          <th>{$tStore('colTrigger')}</th>
          <th>{$tStore('colEffect')}</th>
        </tr>
      </thead>
      <tbody>
        <tr>
          <td style="color:var(--success, #22c55e); white-space:nowrap;">Session Start</td>
          <td><code>epic-harness resume</code></td>
          <td style="color:var(--text-secondary); font-size:0.82rem;">{$tStore('onSessionStart')}</td>
          <td style="color:var(--text-secondary); font-size:0.82rem;">{$tStore('hookResumeEffect')}</td>
        </tr>
        <tr>
          <td style="color:var(--accent, #6366f1); white-space:nowrap;">Pre Tool Use</td>
          <td><code>epic-harness guard</code></td>
          <td style="color:var(--text-secondary); font-size:0.82rem;">{$tStore('onPreTool')}</td>
          <td style="color:var(--text-secondary); font-size:0.82rem;">{$tStore('hookGuardEffect')}</td>
        </tr>
        <tr>
          <td style="color:var(--purple, #a855f7); white-space:nowrap;">Post Tool Use</td>
          <td><code>epic-harness observe</code></td>
          <td style="color:var(--text-secondary); font-size:0.82rem;">{$tStore('onPostTool')}</td>
          <td style="color:var(--text-secondary); font-size:0.82rem;">{$tStore('hookObserveEffect')}</td>
        </tr>
        <tr>
          <td style="color:var(--teal, #06b6d4); white-space:nowrap;">Post Edit</td>
          <td><code>epic-harness polish</code></td>
          <td style="color:var(--text-secondary); font-size:0.82rem;">{$tStore('onPostEdit')}</td>
          <td style="color:var(--text-secondary); font-size:0.82rem;">{$tStore('hookPolishEffect')}</td>
        </tr>
        <tr>
          <td style="color:var(--warning, #f59e0b); white-space:nowrap;">Pre Compact</td>
          <td><code>epic-harness snapshot</code></td>
          <td style="color:var(--text-secondary); font-size:0.82rem;">{$tStore('onPreCompact')}</td>
          <td style="color:var(--text-secondary); font-size:0.82rem;">{$tStore('hookSnapshotEffect')}</td>
        </tr>
        <tr>
          <td style="color:var(--danger, #ef4444); white-space:nowrap;">Session End</td>
          <td><code>epic-harness reflect</code></td>
          <td style="color:var(--text-secondary); font-size:0.82rem;">{$tStore('onSessionEnd')}</td>
          <td style="color:var(--text-secondary); font-size:0.82rem;">{$tStore('hookReflectEffect')}</td>
        </tr>
      </tbody>
    </table>
  </div>

  <!-- Hook Flow -->
  <div class="card" style="margin-bottom:1rem;">
    <h3>{$tStore('hookFlowTitle')}</h3>
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
    <h3>{$tStore('guardRulesTitle')}</h3>
    <p style="font-size:0.82rem; color:var(--text-secondary); margin-bottom:0.75rem;">
      {$tStore('guardRulesDesc')}
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
    <h3>{$tStore('polishFeedbackTitle')}</h3>
    <p style="font-size:0.82rem; color:var(--text-secondary); margin-bottom:0.75rem;">
      {$tStore('polishFeedbackDesc')}
    </p>
    <table class="data-table">
      <thead>
        <tr>
          <th>{$tStore('colPolishResult')}</th>
          <th>{$tStore('colFailureType')}</th>
          <th>{$tStore('colPatternDetection')}</th>
        </tr>
      </thead>
      <tbody>
        <tr>
          <td>{$tStore('polishFormatFail')}</td>
          <td><code>lint_fail</code></td>
          <td style="color:var(--text-secondary); font-size:0.82rem;">{$tStore('polishFormatFeedDesc')}</td>
        </tr>
        <tr>
          <td>{$tStore('polishTypecheckFail')}</td>
          <td><code>build_fail</code></td>
          <td style="color:var(--text-secondary); font-size:0.82rem;">{$tStore('polishTypecheckFeedDesc')}</td>
        </tr>
      </tbody>
    </table>
  </div>
</div>
