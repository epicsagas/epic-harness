<script lang="ts">
  import { onMount } from 'svelte';
  import * as d3 from 'd3';
  import { getGraph, type GraphData, type GraphNode } from '../lib/harness.js';

  let graphData = $state<GraphData>({ nodes: [], edges: [] });
  let selectedNode = $state<GraphNode | null>(null);
  let svgEl = $state<SVGSVGElement | undefined>(undefined);
  let loading = $state(true);
  let error = $state('');

  // node type → color
  const typeColor: Record<string, string> = {
    project: '#6366f1', concept: '#22c55e', pattern: '#f59e0b',
    decision: '#ec4899', error: '#ef4444', session: '#64748b',
    resolution: '#06b6d4', default: '#94a3b8',
  };

  onMount(async () => {
    try {
      graphData = await getGraph();
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
    if (graphData.nodes.length > 0) renderGraph();
  });

  function renderGraph() {
    const el = svgEl as SVGSVGElement | undefined;
    if (!el) return;
    d3.select(el).selectAll('*').remove();

    const width = el.clientWidth || 800;
    const height = el.clientHeight || 500;

    const svg = d3.select(el)
      .attr('viewBox', `0 0 ${width} ${height}`)
      .call(d3.zoom<SVGSVGElement, unknown>().scaleExtent([0.3, 3]).on('zoom', (e) => {
        g.attr('transform', e.transform);
      }));

    const g = svg.append('g');

    // Arrow marker
    svg.append('defs').append('marker')
      .attr('id', 'arrow')
      .attr('viewBox', '0 -4 8 8')
      .attr('refX', 18).attr('refY', 0)
      .attr('markerWidth', 6).attr('markerHeight', 6)
      .attr('orient', 'auto')
      .append('path').attr('d', 'M0,-4L8,0L0,4').attr('fill', '#475569');

    const nodes = graphData.nodes.map(n => ({ ...n })) as (GraphNode & d3.SimulationNodeDatum)[];
    const nodeById = new Map(nodes.map(n => [n.id, n]));

    type SimLink = { source: GraphNode & d3.SimulationNodeDatum; target: GraphNode & d3.SimulationNodeDatum; relation: string; weight: number };
    const links: SimLink[] = graphData.edges
      .map(e => ({ source: nodeById.get(e.source), target: nodeById.get(e.target), relation: e.relation, weight: e.weight }))
      .filter((l): l is SimLink => l.source != null && l.target != null);

    const sim = d3.forceSimulation(nodes)
      .force('link', d3.forceLink(links).id((d: any) => d.id).distance(100))
      .force('charge', d3.forceManyBody().strength(-200))
      .force('center', d3.forceCenter(width / 2, height / 2))
      .force('collision', d3.forceCollide(30));

    const link = g.append('g').selectAll('line').data(links).join('line')
      .attr('stroke', '#334155').attr('stroke-opacity', 0.6)
      .attr('stroke-width', (d: any) => Math.max(1, d.weight * 2))
      .attr('marker-end', 'url(#arrow)');

    const linkLabel = g.append('g').selectAll('text').data(links).join('text')
      .attr('fill', '#64748b').attr('font-size', 9).attr('text-anchor', 'middle')
      .text((d: any) => d.relation);

    const node = g.append('g').selectAll('g').data(nodes).join('g')
      .attr('cursor', 'pointer')
      .call(d3.drag<any, any>()
        .on('start', (e, d) => { if (!e.active) sim.alphaTarget(0.3).restart(); d.fx = d.x; d.fy = d.y; })
        .on('drag', (e, d) => { d.fx = e.x; d.fy = e.y; })
        .on('end', (e, d) => { if (!e.active) sim.alphaTarget(0); d.fx = null; d.fy = null; }))
      .on('click', (_, d: any) => { selectedNode = graphData.nodes.find(n => n.id === d.id) ?? null; });

    node.append('circle')
      .attr('r', (d: any) => 8 + d.importance * 12)
      .attr('fill', (d: any) => typeColor[d.type] ?? typeColor.default)
      .attr('stroke', '#1e293b').attr('stroke-width', 1.5)
      .attr('fill-opacity', 0.85);

    node.append('text')
      .attr('dy', (d: any) => -(10 + d.importance * 12))
      .attr('text-anchor', 'middle')
      .attr('fill', '#e2e8f0').attr('font-size', 11)
      .text((d: any) => d.title.length > 18 ? d.title.slice(0, 16) + '…' : d.title);

    sim.on('tick', () => {
      link.attr('x1', (d: any) => d.source.x).attr('y1', (d: any) => d.source.y)
          .attr('x2', (d: any) => d.target.x).attr('y2', (d: any) => d.target.y);
      linkLabel.attr('x', (d: any) => (d.source.x + d.target.x) / 2)
               .attr('y', (d: any) => (d.source.y + d.target.y) / 2);
      node.attr('transform', (d: any) => `translate(${d.x},${d.y})`);
    });
  }

  $effect(() => {
    if (!loading && graphData.nodes.length > 0 && svgEl) {
      renderGraph();
    }
  });
</script>

<div class="page">
  <div class="page-header">
    <h1>harness-mem</h1>
    <span class="badge badge-wip">WIP</span>
    <p>SQLite + FTS5 knowledge graph · {graphData.nodes.length} nodes · {graphData.edges.length} edges</p>
  </div>

  {#if loading}
    <div class="loading">Loading knowledge graph…</div>
  {:else if error}
    <div class="error-msg">{error}</div>
  {:else}
    <!-- D3 canvas -->
    <div class="graph-container" style="height: 500px; background: var(--surface); border-radius: 8px; position: relative; overflow: hidden; margin-bottom: 1.5rem;">
      <svg bind:this={svgEl as SVGSVGElement} style="width:100%; height:100%;"></svg>
      <div class="graph-legend" style="position:absolute; top:1rem; left:1rem; display:flex; flex-direction:column; gap:0.3rem;">
        {#each Object.entries(typeColor) as [type, color]}
          {#if type !== 'default'}
            <div style="display:flex; align-items:center; gap:0.4rem; font-size:0.75rem; color: var(--text-secondary);">
              <span style="width:10px;height:10px;border-radius:50%;background:{color};display:inline-block;"></span>
              {type}
            </div>
          {/if}
        {/each}
      </div>
    </div>

    <!-- Selected node detail -->
    {#if selectedNode}
      <div class="card" style="margin-bottom:1.5rem;">
        <h3>{selectedNode.title}</h3>
        <div style="display:flex; gap:0.5rem; flex-wrap:wrap; margin-top:0.5rem;">
          <span class="badge">{selectedNode.type}</span>
          {#each selectedNode.tags as tag}
            <span class="badge badge-outline">{tag}</span>
          {/each}
          <span class="badge">importance: {selectedNode.importance.toFixed(2)}</span>
        </div>
      </div>
    {/if}

    <!-- MCP Tools table -->
    <div class="card">
      <h3>MCP Tools</h3>
      <table class="data-table">
        <thead><tr><th>Tool</th><th>Purpose</th></tr></thead>
        <tbody>
          <tr><td>mem_recall</td><td>Smart contextual recall — hint + project + graph neighbors</td></tr>
          <tr><td>mem_add</td><td>Add node with auto-importance by type</td></tr>
          <tr><td>mem_search</td><td>FTS5 keyword search, results ranked by importance</td></tr>
          <tr><td>mem_query</td><td>SQL-level query for advanced filtering</td></tr>
          <tr><td>mem_context</td><td>Project-scoped smart recall (no hint)</td></tr>
          <tr><td>mem_related</td><td>BFS graph traversal from a node ID</td></tr>
        </tbody>
      </table>
    </div>

    <!-- Scoring formula -->
    <div class="card" style="margin-top:1rem;">
      <h3>Smart Recall Scoring</h3>
      <code style="display:block; padding:0.75rem; background:var(--bg); border-radius:4px; font-size:0.85rem;">
        score = recency(25%) + importance(35%) + access_freq(15%) + FTS_match(25%)
      </code>
    </div>
  {/if}
</div>
