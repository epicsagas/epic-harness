<div class="screen-header">
  <h2>Commands <span class="subtitle-tag">Ring 1</span></h2>
  <p>10 user-invoked slash commands (7 manual + 1 autonomous orbit + 2 utility)</p>
</div>

<div class="grid-2">
  <div class="cmd-card">
    <div class="cmd-name">/discover</div>
    <div class="cmd-desc">Problem discovery. Probes with 5 Whys / JTBD / Fishbone / Socratic techniques (max 3 rounds), then frames a structured problem statement.</div>
    <div class="cmd-tags"><span class="pill info">analysis</span><span class="pill teal">pre-spec</span></div>
  </div>
  <div class="cmd-card">
    <div class="cmd-name">/spec</div>
    <div class="cmd-desc">Converts requirements into numbered Requirements + Acceptance Criteria document (SPEC-{ts}.md). Suggests /team when 3+ requirements.</div>
    <div class="cmd-tags"><span class="pill info">planning</span><span class="pill purple">REQ+AC</span></div>
  </div>
  <div class="cmd-card">
    <div class="cmd-name">/go</div>
    <div class="cmd-desc">Reads approved SPEC, maps Requirements to tasks, executes via TDD subagents (parallel where safe, worktree-isolated when files overlap).</div>
    <div class="cmd-tags"><span class="pill success">build</span><span class="pill info">TDD</span></div>
  </div>
  <div class="cmd-card">
    <div class="cmd-name">/check</div>
    <div class="cmd-desc">Parallel agents (reviewer + auditor + test runner) + scope-based checks (API contract, accessibility, migration safety). PASS/WARN/FAIL per dimension.</div>
    <div class="cmd-tags"><span class="pill warning">review</span><span class="pill danger">audit</span></div>
  </div>
  <div class="cmd-card">
    <div class="cmd-name">/ship</div>
    <div class="cmd-desc">Isolated integration test in fresh worktree, creates PR with spec + check report, watches CI, auto-fixes failures. Suggests /evolve on completion.</div>
    <div class="cmd-tags"><span class="pill success">deploy</span><span class="pill info">PR+CI</span></div>
  </div>
  <div class="cmd-card">
    <div class="cmd-name">/evolve</div>
    <div class="cmd-desc">Analyzes session observation logs to detect failure patterns and weak tools, auto-generates targeted skills. Sub-commands: status, history, cross-project, rollback, reset.</div>
    <div class="cmd-tags"><span class="pill orange">evolution</span><span class="pill purple">Ring 3</span></div>
  </div>
  <div class="cmd-card">
    <div class="cmd-name">/team</div>
    <div class="cmd-desc">Agent team manager. Browse org libraries, hire existing teams, or design new team (3-6 agents). Patterns: Pipeline, Fan-out, Expert-Pool, Producer-Reviewer, Supervisor.</div>
    <div class="cmd-tags"><span class="pill info">agents</span><span class="pill teal">team-design</span></div>
  </div>
  <div class="cmd-card" style="border:2px solid var(--accent);background:var(--surface-raised);">
    <div class="cmd-name">/orbit</div>
    <div class="cmd-desc">Autonomous pipeline: spec &#8594; go &#8594; check &#8594; ship &#8594; evolve. Auto-selects mode (Interactive/Council/Direct). Auto-retries check up to 3 times. Runs /evolve on success.</div>
    <div class="cmd-tags"><span class="pill info">autonomous</span><span class="pill success">end-to-end</span><span class="pill warning">30min deadline</span></div>
  </div>
  <div class="cmd-card">
    <div class="cmd-name">/status</div>
    <div class="cmd-desc">Real-time orchestration dashboard. Active agent count, per-agent progress stage, dependency graph, elapsed time, recent event summary.</div>
    <div class="cmd-tags"><span class="pill info">monitoring</span></div>
  </div>
  <div class="cmd-card">
    <div class="cmd-name">/intervene</div>
    <div class="cmd-desc">Intervene in active agent orchestration. Pause, cancel, or redirect running agents mid-execution.</div>
    <div class="cmd-tags"><span class="pill danger">control</span></div>
  </div>
</div>
