<script lang="ts">
  import { tStore } from '$lib/i18n.js';

  type CommandKey =
    | 'cmdDiscoverDesc' | 'cmdSpecDesc' | 'cmdGoDesc' | 'cmdAuditDesc'
    | 'cmdEvalDesc' | 'cmdShipDesc' | 'cmdOrbitDesc' | 'cmdEvolveDesc' | 'cmdTeamDesc';

  // The spec→ship flow orbit chains — 6 phases orbiting the orchestrator,
  // rendered as an SVG diagram (mirrors the epiccounty.com/ecosystem style:
  // central node + satellite nodes connected by labelled flow lines).
  const FLOW = [
    { cmd: 'discover', descKey: 'cmdDiscoverDesc', label: 'DISCOVER' },
    { cmd: 'spec',     descKey: 'cmdSpecDesc',     label: 'SPEC' },
    { cmd: 'go',       descKey: 'cmdGoDesc',       label: 'GO' },
    { cmd: 'audit',    descKey: 'cmdAuditDesc',    label: 'AUDIT' },
    { cmd: 'eval',     descKey: 'cmdEvalDesc',     label: 'EVAL' },
    { cmd: 'ship',     descKey: 'cmdShipDesc',     label: 'SHIP' },
  ] as const;

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
    } catch {
      copiedCmd = null;
    }
  }

  // Diagram geometry — 6 satellites evenly spaced around the centre.
  const CX = 360;
  const CY = 245;
  const R = 175; // orbit radius
  // Each satellite box dimensions
  const BW = 110;
  const BH = 52;
  const positions = FLOW.map((_, i) => {
    // start at top (-90deg), go clockwise
    const angle = (-90 + i * (360 / FLOW.length)) * (Math.PI / 180);
    return { x: CX + R * Math.cos(angle), y: CY + R * Math.sin(angle) };
  });
</script>

<div class="screen-header">
  <h2>{$tStore('pageCommands')} <span class="subtitle-tag">Ring 1 · Skills</span></h2>
  <p>{$tStore('pageCommandsDesc3')}</p>
</div>

<!-- SVG flow diagram: /orbit orchestrator at the centre, 6 phases around it -->
<div class="diagram-wrap">
  <svg viewBox="0 0 720 490" class="diagram" role="img" aria-label="orbit pipeline flow">
    <defs>
      <marker id="arrowhead" markerWidth="8" markerHeight="8" refX="6" refY="4" orient="auto">
        <path d="M0,0 L6,4 L0,8 Z" fill="var(--accent)" />
      </marker>
    </defs>

    <!-- Flow connectors centre → each phase -->
    {#each positions as p, i}
      <line
        x1={CX} y1={CY}
        x2={p.x} y2={p.y}
        class="flow-line"
        marker-end="url(#arrowhead)"
      />
      <!-- phase order badge on the line -->
      <text
        x={CX + (p.x - CX) * 0.42}
        y={CY + (p.y - CY) * 0.42 - 4}
        class="flow-num"
        text-anchor="middle"
      >{i + 1}</text>
    {/each}

    <!-- Central /orbit orchestrator node -->
    <g class="orbit-node" role="button" tabindex="0"
       onclick={() => copyCmd('orbit')}
       onkeydown={(e) => e.key === 'Enter' && copyCmd('orbit')}>
      <rect x={CX - 95} y={CY - 48} width="190" height="96" rx="12" class="orbit-box" />
      <rect x={CX - 87} y={CY - 40} width="174" height="80" rx="9" class="orbit-box-inner" />
      <text x={CX} y={CY - 14} text-anchor="middle" class="orbit-title">/orbit</text>
      <text x={CX} y={CY + 10} text-anchor="middle" class="orbit-sub">ORCHESTRATOR</text>
      <text x={CX} y={CY + 28} text-anchor="middle" class="orbit-hint">spec → ship · auto</text>
    </g>

    <!-- Satellite phase nodes -->
    {#each FLOW as phase, i}
      {@const p = positions[i]}
      <g
        class="phase-node"
        role="button" tabindex="0"
        onclick={() => copyCmd(phase.cmd)}
        onkeydown={(e) => e.key === 'Enter' && copyCmd(phase.cmd)}
      >
        <rect x={p.x - BW / 2} y={p.y - BH / 2} width={BW} height={BH} rx="8" class="phase-box" />
        <text x={p.x} y={p.y - 6} text-anchor="middle" class="phase-cmd">/{phase.cmd}</text>
        <text x={p.x} y={p.y + 12} text-anchor="middle" class="phase-label">{phase.label}</text>
      </g>
    {/each}
  </svg>
  {#if copiedCmd === 'orbit'}
    <div class="copied-float">{$tStore('copied')}</div>
  {/if}
</div>

<div class="diagram-legend">
  <span><span class="dot accent"></span> orbit chains phases 1→6 automatically</span>
  <span>click any node to copy <code>/command</code></span>
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
    margin-bottom: 12px;
  }
  .diagram {
    width: 100%;
    max-width: 720px;
    height: auto;
  }

  .flow-line {
    stroke: var(--border);
    stroke-width: 1.5;
    stroke-dasharray: 4 3;
  }

  .flow-num {
    fill: var(--accent);
    font-size: 12px;
    font-weight: 700;
    font-family: var(--font-mono);
  }

  /* Central orbit node */
  .orbit-box {
    fill: var(--accent-soft);
    stroke: var(--accent);
    stroke-width: 1.5;
  }
  .orbit-box-inner {
    fill: var(--surface-raised);
    stroke: var(--accent);
    stroke-width: 0.8;
    stroke-opacity: 0.4;
  }
  .orbit-node { cursor: pointer; }
  .orbit-node:hover .orbit-box { filter: drop-shadow(0 0 10px var(--accent)); }
  .orbit-title {
    fill: var(--accent);
    font-size: 22px;
    font-weight: 700;
    font-family: var(--font-mono);
  }
  .orbit-sub {
    fill: var(--fg-secondary);
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.1em;
    font-family: var(--font-mono);
  }
  .orbit-hint {
    fill: var(--muted);
    font-size: 10px;
    font-family: var(--font-mono);
  }

  /* Satellite phase nodes */
  .phase-node { cursor: pointer; }
  .phase-box {
    fill: var(--surface-raised);
    stroke: var(--teal);
    stroke-width: 1.2;
    transition: fill var(--transition), stroke-width var(--transition);
  }
  .phase-node:hover .phase-box {
    fill: var(--teal-soft);
    stroke-width: 2;
  }
  .phase-cmd {
    fill: var(--fg);
    font-size: 14px;
    font-weight: 600;
    font-family: var(--font-mono);
  }
  .phase-label {
    fill: var(--teal);
    font-size: 9px;
    font-weight: 700;
    letter-spacing: 0.08em;
    font-family: var(--font-mono);
  }

  .copied-float {
    position: absolute;
    top: 12px;
    right: 16px;
    font-size: 11px;
    color: var(--success);
    font-family: var(--font-mono);
    font-weight: 600;
  }

  .diagram-legend {
    display: flex;
    flex-wrap: wrap;
    gap: 18px;
    justify-content: center;
    font-size: 12px;
    color: var(--muted);
    margin-bottom: 28px;
    font-family: var(--font-mono);
  }
  .diagram-legend .dot {
    display: inline-block;
    width: 8px; height: 8px;
    border-radius: 50%;
    margin-right: 4px;
    vertical-align: middle;
  }
  .diagram-legend .dot.accent { background: var(--accent); }
  .diagram-legend code {
    font-family: var(--font-mono);
    color: var(--teal);
    background: var(--teal-soft);
    padding: 1px 5px;
    border-radius: 3px;
  }

  /* Cross-cutting */
  .util-section { margin-top: 4px; }
  .util-label {
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.07em;
    color: var(--muted);
    margin-bottom: 10px;
    font-family: var(--font-mono);
  }
  .util-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
    gap: 12px;
  }
  .util-card {
    display: block;
    text-align: left;
    padding: 14px;
    border: 1px solid var(--border);
    border-left: 3px solid var(--purple);
    border-radius: var(--radius);
    background: var(--surface);
    cursor: pointer;
    font-family: inherit;
    color: var(--fg);
    transition: border-color var(--transition), background var(--transition);
  }
  .util-card:hover { border-color: var(--purple); background: var(--surface-raised); }
  .util-head { display: flex; align-items: center; gap: 8px; margin-bottom: 4px; }
  .util-name { font-family: var(--font-mono); font-weight: 600; font-size: 14px; }
  .util-desc { font-size: 12px; color: var(--fg-secondary); }
  .copied-inline {
    font-size: 11px; color: var(--success);
    font-family: var(--font-mono); font-weight: 600;
  }

  @media (max-width: 640px) {
    .diagram-legend { flex-direction: column; gap: 6px; text-align: center; }
  }
</style>
