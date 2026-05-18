<div class="screen-header">
  <h2>Auto Skills <span class="subtitle-tag">Ring 2</span></h2>
  <p>15 context-triggered skills + _dispatch core router &middot; static skills always take priority over evolved</p>
</div>

<!-- _dispatch highlight -->
<div class="panel" style="margin-bottom:16px;border-left:3px solid var(--accent);">
  <div class="panel-header"><h3>_dispatch &mdash; Core Router</h3></div>
  <div class="panel-body">
    <div class="cmd-desc" style="font-size:13px;">
      Always active. Auto-invokes matching skill before every response. Runs confusion protocol on high-risk ambiguity.
      Calls <code>mem_recall</code> with current task context before invoking any skill. Past decisions (importance=0.9) surface first.
    </div>
    <div style="margin-top:8px;font-size:11px;color:var(--muted);font-family:var(--font-mono);">
      dispatch log: <code>$HARNESS_DIR/dispatch/dispatch_YYYYMMDD.jsonl</code>
    </div>
  </div>
</div>

<div style="display:grid;grid-template-columns:repeat(auto-fill,minmax(280px,1fr));gap:12px;">
  <div class="skill-card">
    <div class="skill-name">tdd</div>
    <div class="skill-trigger">trigger: new feature or bug fix</div>
    <div class="skill-desc">Enforces Red &#8594; Green &#8594; Refactor. No prod code without failing test first.</div>
  </div>
  <div class="skill-card">
    <div class="skill-name">debug</div>
    <div class="skill-trigger">trigger: test failure, runtime error, unexpected behavior</div>
    <div class="skill-desc">Systematic root-cause isolation. Never retry without diagnosis.</div>
  </div>
  <div class="skill-card">
    <div class="skill-name">verify</div>
    <div class="skill-trigger">trigger: before marking done or /ship</div>
    <div class="skill-desc">Build + test + lint must all pass. No exceptions.</div>
  </div>
  <div class="skill-card">
    <div class="skill-name">secure</div>
    <div class="skill-trigger">trigger: auth, DB, API, infra, or secrets code touched</div>
    <div class="skill-desc">Security checklist for SQL, endpoints, env vars, infra config.</div>
  </div>
  <div class="skill-card">
    <div class="skill-name">perf</div>
    <div class="skill-trigger">trigger: loops, DB queries, rendering, batch ops</div>
    <div class="skill-desc">Catches N+1, missing indexes, unnecessary re-renders.</div>
  </div>
  <div class="skill-card">
    <div class="skill-name">simplify</div>
    <div class="skill-trigger">trigger: file &gt;200 lines, high complexity, duplication</div>
    <div class="skill-desc">Deep nesting, copy-paste, god functions. Extract and reduce.</div>
  </div>
  <div class="skill-card">
    <div class="skill-name">commit</div>
    <div class="skill-trigger">trigger: commit/save</div>
    <div class="skill-desc">CC 1.0.0 commit generator. Stages relevant files, infers type(scope): desc.</div>
  </div>
  <div class="skill-card">
    <div class="skill-name">context</div>
    <div class="skill-trigger">trigger: context &gt;70% or large new task</div>
    <div class="skill-desc">Snapshots state, compacts, resumes via hook.</div>
  </div>
  <div class="skill-card">
    <div class="skill-name">discover</div>
    <div class="skill-trigger">trigger: vague/unfocused request</div>
    <div class="skill-desc">Reframes goal before acting. Prevents solution-without-problem.</div>
  </div>
  <div class="skill-card">
    <div class="skill-name">council</div>
    <div class="skill-trigger">trigger: architecture/design decisions with trade-offs</div>
    <div class="skill-desc">Multi-voice analysis for decisions with no clear answer.</div>
  </div>
  <div class="skill-card">
    <div class="skill-name">document</div>
    <div class="skill-trigger">trigger: public API/module added or changed</div>
    <div class="skill-desc">Auto-generates documentation for new exports and signature changes.</div>
  </div>
  <div class="skill-card">
    <div class="skill-name">reflect</div>
    <div class="skill-trigger">trigger: /reflect or AI usage review</div>
    <div class="skill-desc">Scores 5 dims: amplification, improvement, metacognition, prompts, efficiency.</div>
  </div>
  <div class="skill-card">
    <div class="skill-name">orchestrate</div>
    <div class="skill-trigger">trigger: active multi-agent run</div>
    <div class="skill-desc">Handles inbox reading, dep resolution, message formatting, handoffs.</div>
  </div>
  <div class="skill-card">
    <div class="skill-name">agent-introspection</div>
    <div class="skill-trigger">trigger: 3+ consecutive failures, circular retries</div>
    <div class="skill-desc">Breaks loops. Detects overwhelming context and circular patterns.</div>
  </div>
</div>
