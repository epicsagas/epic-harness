<div class="screen-header">
  <h2>Internal Agents</h2>
  <p>4 built-in agents used by /go and /check phases &middot; extendable via /team</p>
</div>

<div class="grid-2">
  <div class="agent-card">
    <div class="agent-name">Builder</div>
    <div class="agent-role">epic:builder</div>
    <div class="agent-desc">Implements a single task using TDD. Writes test first, then code, then verifies. Used by /go to execute individual requirement tasks.</div>
    <div style="margin-top:10px;"><span class="pill success">TDD</span><span class="pill info">/go</span></div>
  </div>
  <div class="agent-card">
    <div class="agent-name">Reviewer</div>
    <div class="agent-role">epic:reviewer</div>
    <div class="agent-desc">Reviews code for quality, correctness, style, and test coverage. Launched by /check in parallel with auditor and test runner.</div>
    <div style="margin-top:10px;"><span class="pill warning">review</span><span class="pill info">/check</span></div>
  </div>
  <div class="agent-card">
    <div class="agent-name">Auditor</div>
    <div class="agent-role">epic:auditor</div>
    <div class="agent-desc">Audits code for security vulnerabilities and performance issues. Parallel execution with reviewer during /check phase.</div>
    <div style="margin-top:10px;"><span class="pill danger">security</span><span class="pill info">/check</span></div>
  </div>
  <div class="agent-card">
    <div class="agent-name">Planner</div>
    <div class="agent-role">epic:planner</div>
    <div class="agent-desc">Breaks down a goal into ordered, parallelizable tasks with dependencies. Used by /go to create the execution plan from approved spec.</div>
    <div style="margin-top:10px;"><span class="pill purple">planning</span><span class="pill info">/go</span></div>
  </div>
</div>

<!-- /team Patterns -->
<div class="panel" style="margin-top:16px;">
  <div class="panel-header"><h3>/team Orchestration Patterns</h3></div>
  <div class="panel-body" style="padding:0;">
    <table class="data-table">
      <thead>
        <tr><th>Pattern</th><th>Agents</th><th>Best For</th></tr>
      </thead>
      <tbody>
        <tr>
          <td style="color:var(--fg)">Pipeline</td>
          <td>3-6</td>
          <td style="color:var(--fg-secondary)">Sequential handoffs, build &#8594; test &#8594; review</td>
        </tr>
        <tr>
          <td style="color:var(--fg)">Fan-out</td>
          <td>3-6</td>
          <td style="color:var(--fg-secondary)">Parallel independent tasks, then merge</td>
        </tr>
        <tr>
          <td style="color:var(--fg)">Expert-Pool</td>
          <td>3-6</td>
          <td style="color:var(--fg-secondary)">Route to specialist per task type</td>
        </tr>
        <tr>
          <td style="color:var(--fg)">Producer-Reviewer</td>
          <td>2-3</td>
          <td style="color:var(--fg-secondary)">One builds, one reviews, iterate</td>
        </tr>
        <tr>
          <td style="color:var(--fg)">Supervisor</td>
          <td>3-6</td>
          <td style="color:var(--fg-secondary)">Central coordinator dispatches to workers</td>
        </tr>
      </tbody>
    </table>
  </div>
</div>
