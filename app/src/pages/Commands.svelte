<script lang="ts">
  import { tStore } from '$lib/i18n.js';

  // /orbit decision tree — mirrors the README mermaid diagram exactly:
  //   /orbit → requirement? → {interactive | council | direct} → Load spec →
  //   Go → Check → (PASS/WARN → Ship | FAIL → retry<3? → Go | Pause) →
  //   Ship → Evolve → Orbit Complete
  // Purple = human checkpoint, green = autonomous, teal = decision diamond.
  const UTILITIES = [
    { cmd: 'evolve', descKey: 'cmdEvolveDesc' },
    { cmd: 'team',   descKey: 'cmdTeamDesc' },
  ] as const;

  let copiedCmd = $state<string | null>(null);
  async function copyCmd(cmd: string) {
    try {
      await navigator.clipboard.writeText('/' + cmd);
      copiedCmd = cmd;
      setTimeout(() => { copiedCmd = null; }, 1500);
    } catch { copiedCmd = null; }
  }
</script>

<div class="screen-header">
  <h2>{$tStore('pageCommands')} <span class="subtitle-tag">Ring 1 · Skills</span></h2>
  <p>{$tStore('pageCommandsDesc3')}</p>
</div>

<!-- /orbit decision tree (SVG, README-style flowchart) -->
<div class="diagram-wrap">
  <svg viewBox="0 0 620 760" class="diagram" role="img" aria-label="orbit decision flow">
    <defs>
      <marker id="ah" markerWidth="9" markerHeight="9" refX="7" refY="4.5" orient="auto">
        <path d="M0,0 L7,4.5 L0,9 Z" fill="var(--muted)" />
      </marker>
    </defs>

    <!-- Connectors (drawn first, under nodes) -->
    <!-- /orbit → requirement? -->
    <line x1="310" y1="56" x2="310" y2="92" class="edge" marker-end="url(#ah)" />
    <!-- requirement? → 3 modes -->
    <path d="M310 168 L310 196 L120 196 L120 222" class="edge" fill="none" marker-end="url(#ah)" />
    <line x1="310" y1="168" x2="310" y2="222" class="edge" marker-end="url(#ah)" />
    <path d="M310 168 L310 196 L500 196 L500 222" class="edge" fill="none" marker-end="url(#ah)" />
    <!-- edge labels -->
    <text x="210" y="190" class="elabel" text-anchor="middle">unclear</text>
    <text x="324" y="190" class="elabel">clear + complex</text>
    <text x="410" y="190" class="elabel" text-anchor="middle">simple</text>

    <!-- 3 modes → Load spec (merge) -->
    <path d="M120 300 L120 326 L310 326" class="edge" fill="none" marker-end="url(#ah)" />
    <line x1="310" y1="300" x2="310" y2="326" class="edge" marker-end="url(#ah)" />
    <path d="M500 300 L500 326 L310 326" class="edge" fill="none" marker-end="url(#ah)" />

    <!-- Load spec → Go -->
    <line x1="310" y1="372" x2="310" y2="408" class="edge" marker-end="url(#ah)" />
    <!-- Go → Check -->
    <line x1="310" y1="468" x2="310" y2="504" class="edge" marker-end="url(#ah)" />
    <!-- Check → Ship (PASS/WARN) -->
    <path d="M390 540 L430 540 L430 600" class="edge" fill="none" marker-end="url(#ah)" />
    <text x="438" y="572" class="elabel">PASS / WARN</text>
    <!-- Check → retry (FAIL) -->
    <path d="M230 540 L190 540 L190 600" class="edge" fill="none" marker-end="url(#ah)" />
    <text x="150" y="572" class="elabel">FAIL</text>

    <!-- retry → Go (yes, loop back) -->
    <path d="M120 600 L60 600 L60 438 L218 438" class="edge" fill="none" marker-end="url(#ah)" />
    <text x="68" y="520" class="elabel">yes</text>
    <!-- retry → Pause (no) -->
    <line x1="190" y1="648" x2="190" y2="684" class="edge" marker-end="url(#ah)" />
    <text x="200" y="672" class="elabel">no</text>

    <!-- Pause → Abort (abort) -->
    <path d="M120 710 L70 710" class="edge" fill="none" marker-end="url(#ah)" />
    <text x="78" y="702" class="elabel">abort</text>

    <!-- Ship → Evolve -->
    <line x1="430" y1="660" x2="430" y2="696" class="edge" marker-end="url(#ah)" />

    <!-- ── Nodes ─────────────────────────────────────────── -->

    <!-- /orbit (start, oval) -->
    <g class="node-start" role="button" tabindex="0"
       onclick={() => copyCmd('orbit')}
       onkeydown={(e) => e.key === 'Enter' && copyCmd('orbit')}>
      <rect x="250" y="18" width="120" height="38" rx="19" class="n-start" />
      <text x="310" y="42" text-anchor="middle" class="n-text">/orbit</text>
    </g>

    <!-- requirement? (decision diamond) -->
    <g>
      <polygon points="310,92 380,130 310,168 240,130" class="n-decide" />
      <text x="310" y="134" text-anchor="middle" class="n-text">requirement?</text>
    </g>

    <!-- Interactive (human, left) -->
    <g>
      <rect x="40" y="222" width="160" height="78" rx="6" class="n-human" />
      <text x="120" y="246" text-anchor="middle" class="n-title">Interactive</text>
      <text x="120" y="266" text-anchor="middle" class="n-sub">/discover → /spec</text>
      <text x="120" y="284" text-anchor="middle" class="n-sub">then 'orbit go'</text>
    </g>

    <!-- Council (auto, center) -->
    <g>
      <rect x="220" y="222" width="180" height="78" rx="6" class="n-auto" />
      <text x="310" y="246" text-anchor="middle" class="n-title">Council</text>
      <text x="310" y="266" text-anchor="middle" class="n-sub">4-voice auto-spec</text>
      <text x="310" y="284" text-anchor="middle" class="n-tag">clear + complex</text>
    </g>

    <!-- Direct (auto, right) -->
    <g>
      <rect x="420" y="222" width="160" height="78" rx="6" class="n-auto" />
      <text x="500" y="246" text-anchor="middle" class="n-title">Direct</text>
      <text x="500" y="266" text-anchor="middle" class="n-sub">auto-spec</text>
      <text x="500" y="284" text-anchor="middle" class="n-tag">clear + simple</text>
    </g>

    <!-- Load spec (merge) -->
    <g>
      <rect x="230" y="326" width="160" height="46" rx="6" class="n-merge" />
      <text x="310" y="354" text-anchor="middle" class="n-text">Load spec</text>
    </g>

    <!-- Go -->
    <g class="node-cmd" role="button" tabindex="0"
       onclick={() => copyCmd('go')}
       onkeydown={(e) => e.key === 'Enter' && copyCmd('go')}>
      <rect x="218" y="408" width="184" height="60" rx="6" class="n-auto" />
      <text x="310" y="432" text-anchor="middle" class="n-title">Go</text>
      <text x="310" y="452" text-anchor="middle" class="n-sub">plan → TDD → integrate</text>
    </g>

    <!-- Check -->
    <g class="node-cmd" role="button" tabindex="0"
       onclick={() => copyCmd('audit')}
       onkeydown={(e) => e.key === 'Enter' && copyCmd('audit')}>
      <rect x="230" y="504" width="160" height="72" rx="6" class="n-auto" />
      <text x="310" y="528" text-anchor="middle" class="n-title">Check</text>
      <text x="310" y="548" text-anchor="middle" class="n-sub">review + audit + test</text>
    </g>

    <!-- retry<3? (decision diamond) -->
    <g>
      <polygon points="190,600 250,624 190,648 130,624" class="n-decide-sm" />
      <text x="190" y="628" text-anchor="middle" class="n-text-sm">retry &lt; 3?</text>
    </g>

    <!-- Pause (human) -->
    <g>
      <rect x="120" y="684" width="140" height="52" rx="6" class="n-human" />
      <text x="190" y="706" text-anchor="middle" class="n-title">Pause</text>
      <text x="190" y="724" text-anchor="middle" class="n-sub">user decides</text>
    </g>

    <!-- Abort (terminal, oval) -->
    <g>
      <rect x="14" y="692" width="56" height="36" rx="18" class="n-terminal" />
      <text x="42" y="715" text-anchor="middle" class="n-text-sm">Abort</text>
    </g>

    <!-- Ship -->
    <g class="node-cmd" role="button" tabindex="0"
       onclick={() => copyCmd('ship')}
       onkeydown={(e) => e.key === 'Enter' && copyCmd('ship')}>
      <rect x="350" y="600" width="160" height="60" rx="6" class="n-auto" />
      <text x="430" y="624" text-anchor="middle" class="n-title">Ship</text>
      <text x="430" y="644" text-anchor="middle" class="n-sub">isolated test → PR → CI</text>
    </g>

    <!-- Evolve -->
    <g class="node-cmd" role="button" tabindex="0"
       onclick={() => copyCmd('evolve')}
       onkeydown={(e) => e.key === 'Enter' && copyCmd('evolve')}>
      <rect x="350" y="696" width="160" height="48" rx="6" class="n-auto" />
      <text x="430" y="718" text-anchor="middle" class="n-title">Evolve</text>
      <text x="430" y="735" text-anchor="middle" class="n-sub">auto-analyze session</text>
    </g>

    <!-- (Orbit Complete would be below Evolve; omitted for space — Evolve is the last clickable node) -->
  </svg>
  {#if copiedCmd}
    <div class="copied-float">copied /{copiedCmd}</div>
  {/if}
</div>

<div class="legend">
  <span><span class="sw human"></span> human checkpoint</span>
  <span><span class="sw auto"></span> autonomous</span>
  <span><span class="sw decide"></span> decision</span>
  <span class="hint">click a node to copy the command</span>
</div>

<!-- Cross-cutting orchestrators -->
<div class="util-section">
  <div class="util-label">Cross-cutting orchestrators</div>
  <div class="util-grid">
    {#each UTILITIES as { cmd, descKey }}
      <button type="button" class="util-card" onclick={() => copyCmd(cmd)}>
        <div class="util-head">
          <span class="util-name">/{cmd}</span>
          {#if copiedCmd === cmd}
            <span class="copied-inline">{$tStore('copied')}</span>
          {/if}
        </div>
        <div class="util-desc">{$tStore(descKey)}</div>
      </button>
    {/each}
  </div>
</div>

<style>
  .diagram-wrap {
    position: relative;
    display: flex;
    justify-content: center;
    overflow-x: auto;
    margin-bottom: 10px;
  }
  .diagram { width: 100%; max-width: 620px; height: auto; min-width: 560px; }

  .edge { stroke: var(--muted); stroke-width: 1.4; fill: none; }
  .elabel {
    fill: var(--fg-secondary);
    font-size: 10px;
    font-family: var(--font-mono);
  }

  /* Node fills — README classDef: human=#4a4a6a, auto=#1a5c3a */
  .n-human { fill: #4a4a6a; stroke: #9b9bcc; stroke-width: 1; }
  .n-auto  { fill: #1a5c3a; stroke: #4caf7d; stroke-width: 1; }
  .n-merge { fill: #6b4a2a; stroke: #c9954a; stroke-width: 1; }
  .n-decide { fill: #2a3a5c; stroke: #5a7acc; stroke-width: 1; }
  .n-decide-sm { fill: #3a2a1a; stroke: #c9954a; stroke-width: 1; }
  .n-start, .n-terminal { fill: #1a5c3a; stroke: #4caf7d; stroke-width: 1.2; }

  .node-cmd, .node-start { cursor: pointer; }
  .node-cmd:hover .n-auto,
  .node-start:hover .n-start { filter: brightness(1.25); }

  .n-text { fill: #fff; font-size: 13px; font-weight: 600; font-family: var(--font-mono); }
  .n-text-sm { fill: #fff; font-size: 11px; font-weight: 600; font-family: var(--font-mono); }
  .n-title { fill: #fff; font-size: 14px; font-weight: 700; font-family: var(--font-mono); }
  .n-sub { fill: rgba(255,255,255,0.82); font-size: 10.5px; font-family: var(--font-mono); }
  .n-tag { fill: #4caf7d; font-size: 9.5px; font-weight: 700; font-family: var(--font-mono); letter-spacing: 0.04em; }

  .copied-float {
    position: absolute; top: 10px; right: 14px;
    font-size: 11px; color: var(--success);
    font-family: var(--font-mono); font-weight: 600;
  }

  .legend {
    display: flex; flex-wrap: wrap; gap: 16px; justify-content: center;
    font-size: 11px; color: var(--muted);
    margin-bottom: 28px; font-family: var(--font-mono);
  }
  .legend .sw {
    display: inline-block; width: 11px; height: 11px;
    border-radius: 3px; margin-right: 5px; vertical-align: -1px;
    border: 1px solid;
  }
  .legend .sw.human { background: #4a4a6a; border-color: #9b9bcc; }
  .legend .sw.auto { background: #1a5c3a; border-color: #4caf7d; }
  .legend .sw.decide { background: #2a3a5c; border-color: #5a7acc; }
  .legend .hint { color: var(--teal); }

  .util-section { margin-top: 4px; }
  .util-label {
    font-size: 11px; text-transform: uppercase; letter-spacing: 0.07em;
    color: var(--muted); margin-bottom: 10px; font-family: var(--font-mono);
  }
  .util-grid {
    display: grid; grid-template-columns: repeat(auto-fill, minmax(280px, 1fr)); gap: 12px;
  }
  .util-card {
    display: block; text-align: left; padding: 14px;
    border: 1px solid var(--border); border-left: 3px solid var(--purple);
    border-radius: var(--radius); background: var(--surface);
    cursor: pointer; font-family: inherit; color: var(--fg);
    transition: border-color var(--transition), background var(--transition);
  }
  .util-card:hover { border-color: var(--purple); background: var(--surface-raised); }
  .util-head { display: flex; align-items: center; gap: 8px; margin-bottom: 4px; }
  .util-name { font-family: var(--font-mono); font-weight: 600; font-size: 14px; }
  .util-desc { font-size: 12px; color: var(--fg-secondary); }
  .copied-inline { font-size: 11px; color: var(--success); font-family: var(--font-mono); font-weight: 600; }
</style>
