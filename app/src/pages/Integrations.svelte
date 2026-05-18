<div class="screen-header">
  <h2>Integrations</h2>
  <p>6 AI coding tool integrations &middot; configs in <code>integrations/</code></p>
</div>

<div style="display:grid;grid-template-columns:repeat(auto-fill,minmax(360px,1fr));gap:16px;">
  <div class="integration-card">
    <div class="int-icon" style="background:var(--accent-soft);color:var(--accent);">&#9889;</div>
    <div class="int-info">
      <div class="int-name">Claude Code</div>
      <div class="int-desc">Primary integration. Hooks + skills + commands + MCP server.</div>
      <div class="int-assets">
        <span class="pill info">hooks</span>
        <span class="pill purple">skills (15)</span>
        <span class="pill teal">commands (10)</span>
        <span class="pill orange">agents (4)</span>
      </div>
    </div>
  </div>
  <div class="integration-card">
    <div class="int-icon" style="background:var(--success-soft);color:var(--success);">&#128187;</div>
    <div class="int-info">
      <div class="int-name">Codex (OpenAI)</div>
      <div class="int-desc">OpenAI Codex CLI integration.</div>
      <div class="int-assets">
        <span class="pill info">hooks.json</span>
        <span class="pill purple">config.toml</span>
        <span class="pill teal">prompts (8)</span>
        <span class="pill orange">skills (7)</span>
      </div>
    </div>
  </div>
  <div class="integration-card">
    <div class="int-icon" style="background:var(--purple-soft);color:var(--purple);">&#10148;</div>
    <div class="int-info">
      <div class="int-name">Cursor</div>
      <div class="int-desc">Cursor IDE agent integration.</div>
      <div class="int-assets">
        <span class="pill info">hooks.json</span>
        <span class="pill teal">commands (8)</span>
        <span class="pill orange">agents (4)</span>
      </div>
    </div>
  </div>
  <div class="integration-card">
    <div class="int-icon" style="background:var(--teal-soft);color:var(--teal);">&#10024;</div>
    <div class="int-info">
      <div class="int-name">Gemini CLI</div>
      <div class="int-desc">Google Gemini CLI integration.</div>
      <div class="int-assets">
        <span class="pill info">settings.json</span>
        <span class="pill purple">GEMINI.md</span>
        <span class="pill teal">commands (8)</span>
        <span class="pill orange">skills (7)</span>
      </div>
    </div>
  </div>
  <div class="integration-card">
    <div class="int-icon" style="background:var(--orange-soft);color:var(--orange);">&#9878;</div>
    <div class="int-info">
      <div class="int-name">Cline</div>
      <div class="int-desc">Cline autonomous coding agent.</div>
      <div class="int-assets">
        <span class="pill info">hooks (5 scripts)</span>
        <span class="pill purple">rules/epic-harness.md</span>
      </div>
    </div>
  </div>
  <div class="integration-card">
    <div class="int-icon" style="background:var(--pink-soft);color:var(--pink);">&#127760;</div>
    <div class="int-info">
      <div class="int-name">Aider</div>
      <div class="int-desc">Aider AI pair programming integration.</div>
      <div class="int-assets">
        <span class="pill info">.aider.conf.yml</span>
        <span class="pill purple">CONVENTIONS.md</span>
      </div>
    </div>
  </div>
</div>

<!-- Integration Details -->
<div class="panel" style="margin-top:16px;">
  <div class="panel-header"><h3>Shared Resources</h3></div>
  <div class="panel-body" style="padding:0;">
    <table class="data-table">
      <thead><tr><th>Resource</th><th>Count</th><th>Location</th></tr></thead>
      <tbody>
        <tr>
          <td style="color:var(--fg)">Commands</td>
          <td>10</td>
          <td style="color:var(--muted)">registry/commands/*.md</td>
        </tr>
        <tr>
          <td style="color:var(--fg)">Auto Skills</td>
          <td>15 + _dispatch</td>
          <td style="color:var(--muted)">registry/skills/*/SKILL.md</td>
        </tr>
        <tr>
          <td style="color:var(--fg)">Agents</td>
          <td>4</td>
          <td style="color:var(--muted)">registry/agents/*.md</td>
        </tr>
        <tr>
          <td style="color:var(--fg)">Presets</td>
          <td>4 stacks</td>
          <td style="color:var(--muted)">registry/presets/{go,node,python,rust}</td>
        </tr>
        <tr>
          <td style="color:var(--fg)">Hooks Binary</td>
          <td>1</td>
          <td style="color:var(--muted)">hooks/bin/epic-harness (Rust)</td>
        </tr>
      </tbody>
    </table>
  </div>
</div>
