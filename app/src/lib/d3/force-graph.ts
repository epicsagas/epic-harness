import * as d3 from 'd3';
import type { GraphNode, GraphEdge } from '$lib/api/types';

const TYPE_COLORS: Record<string, string> = {
  decision: '#6750A4',
  resolution: '#4A9B7F',
  concept: '#D81B60',
  project: '#1976D2',
  error: '#B3261E',
  session: '#7D5260',
  pattern: '#0288D1',
  instinct: '#9C27B0',
  psychographic: '#FF6F00',
};

export function getColor(type: string): string {
  return TYPE_COLORS[type] ?? '#7D5260';
}

interface SimNode extends d3.SimulationNodeDatum, GraphNode {}

interface SimLink extends d3.SimulationLinkDatum<SimNode> {
  label: string;
}

export interface GraphRenderOptions {
  svgEl: SVGSVGElement;
  width: number;
  height: number;
  nodes: GraphNode[];
  edges: GraphEdge[];
  onNodeClick?: (node: GraphNode) => void;
  onNodeHover?: (node: GraphNode | null) => void;
}

function setupZoom(svg: d3.Selection<SVGSVGElement, unknown, null, undefined>, g: d3.Selection<SVGGElement, unknown, null, undefined>) {
  const zoom = d3.zoom<SVGSVGElement, unknown>()
    .scaleExtent([0.1, 4])
    .on('zoom', (event: d3.D3ZoomEvent<SVGSVGElement, unknown>) => g.attr('transform', event.transform.toString()));
  svg.call(zoom);
  return zoom;
}

function renderLinks(g: d3.Selection<SVGGElement, unknown, null, undefined>, simLinks: SimLink[]) {
  return g.append('g')
    .attr('class', 'links')
    .selectAll('line')
    .data(simLinks)
    .join('line')
    .attr('stroke', '#49454F')
    .attr('stroke-opacity', 0.4)
    .attr('stroke-width', 1);
}

function renderLinkLabels(g: d3.Selection<SVGGElement, unknown, null, undefined>, simLinks: SimLink[]) {
  return g.append('g')
    .attr('class', 'link-labels')
    .selectAll('text')
    .data(simLinks.filter((l) => l.label))
    .join('text')
    .attr('fill', '#938F99')
    .attr('font-size', 9)
    .attr('text-anchor', 'middle')
    .text((d) => d.label);
}

function setupDrag(simulation: d3.Simulation<SimNode, SimLink>) {
  return d3.drag<SVGGElement, SimNode, SimNode>()
    .on('start', (event: d3.D3DragEvent<SVGGElement, SimNode, SimNode>, d) => {
      if (!event.active) simulation.alphaTarget(0.3).restart();
      d.fx = d.x;
      d.fy = d.y;
    })
    .on('drag', (event: d3.D3DragEvent<SVGGElement, SimNode, SimNode>, d) => {
      d.fx = event.x;
      d.fy = event.y;
    })
    .on('end', (event: d3.D3DragEvent<SVGGElement, SimNode, SimNode>, d) => {
      if (!event.active) simulation.alphaTarget(0);
      d.fx = null;
      d.fy = null;
    });
}

function getNodePos(d: SimNode | string | number): { x: number; y: number } {
  return typeof d === 'object' ? { x: d.x ?? 0, y: d.y ?? 0 } : { x: 0, y: 0 };
}

function createSimulation(simNodes: SimNode[], simLinks: SimLink[], width: number, height: number) {
  return d3.forceSimulation<SimNode>(simNodes)
    .force('link', d3.forceLink<SimNode, SimLink>(simLinks).id((d) => d.id).distance(80))
    .force('charge', d3.forceManyBody().strength(-120))
    .force('center', d3.forceCenter(width / 2, height / 2))
    .force('collision', d3.forceCollide<SimNode>().radius(20));
}

/** Shared node rendering — used by both initial render and updateData enter selection. */
function renderNodeGroup(
  enter: d3.Selection<d3.EnterElement, SimNode, SVGGElement, SimNode>,
  drag: d3.DragBehavior<SVGGElement, SimNode, SimNode>,
  onNodeClick?: (node: GraphNode) => void,
  onNodeHover?: (node: GraphNode | null) => void,
) {
  const g = enter.append('g').attr('cursor', 'pointer').call(drag);
  g.append('circle')
    .attr('r', 8)
    .attr('fill', (d) => getColor(d.type))
    .attr('stroke', '#1C1B1F')
    .attr('stroke-width', 1.5)
    .attr('opacity', 0.9);
  g.append('text')
    .attr('dy', -14)
    .attr('text-anchor', 'middle')
    .attr('fill', '#CAC4D0')
    .attr('font-size', 10)
    .text((d) => {
      const title = d.title || '';
      return title.length > 20 ? title.slice(0, 20) + '...' : title;
    });
  g.on('click', (_event, d) => onNodeClick?.(d))
    .on('mouseenter', (_event, d) => onNodeHover?.(d))
    .on('mouseleave', () => onNodeHover?.(null));
  return g;
}

export function renderForceGraph(opts: GraphRenderOptions) {
  const { svgEl, width, height, nodes, edges, onNodeClick, onNodeHover } = opts;
  const svg = d3.select(svgEl);
  svg.selectAll('*').remove();
  const g = svg.append('g');
  const zoom = setupZoom(svg, g);

  const simNodes: SimNode[] = nodes.map((n) => ({ ...n }));
  const nodeMap = new Map(simNodes.map((n) => [n.id, n]));
  const simLinks: SimLink[] = edges
    .filter((e) => nodeMap.has(e.source) && nodeMap.has(e.target))
    .map((e) => ({ source: e.source, target: e.target, label: e.relation }));

  let link = renderLinks(g, simLinks);
  let linkLabel = renderLinkLabels(g, simLinks);

  const simulation = createSimulation(simNodes, simLinks, width, height);
  const drag = setupDrag(simulation);

  let node = g.append('g')
    .attr('class', 'nodes')
    .selectAll<SVGGElement, SimNode>('g')
    .data(simNodes)
    .join('g')
    .attr('cursor', 'pointer')
    .call(drag);

  node.append('circle')
    .attr('r', 8)
    .attr('fill', (d) => getColor(d.type))
    .attr('stroke', '#1C1B1F')
    .attr('stroke-width', 1.5)
    .attr('opacity', 0.9);

  node.append('text')
    .attr('dy', -14)
    .attr('text-anchor', 'middle')
    .attr('fill', '#CAC4D0')
    .attr('font-size', 10)
    .text((d) => {
      const title = d.title || '';
      return title.length > 20 ? title.slice(0, 20) + '...' : title;
    });

  node.on('click', (_event, d) => onNodeClick?.(d))
    .on('mouseenter', (_event, d) => onNodeHover?.(d))
    .on('mouseleave', () => onNodeHover?.(null));

  simulation.on('tick', () => {
    link
      .attr('x1', (d) => getNodePos(d.source).x)
      .attr('y1', (d) => getNodePos(d.source).y)
      .attr('x2', (d) => getNodePos(d.target).x)
      .attr('y2', (d) => getNodePos(d.target).y);

    linkLabel
      .attr('x', (d) => (getNodePos(d.source).x + getNodePos(d.target).x) / 2)
      .attr('y', (d) => (getNodePos(d.source).y + getNodePos(d.target).y) / 2);

    node.attr('transform', (d) => `translate(${d.x ?? 0},${d.y ?? 0})`);
  });

  return {
    destroy: () => {
      svg.selectAll('*').remove();
      svg.on('.zoom', null);
      simulation.stop();
    },
    updateData: (newNodes: GraphNode[], newEdges: GraphEdge[]) => {
      const newSimNodes: SimNode[] = newNodes.map((n) => {
        const existing = nodeMap.get(n.id);
        return { ...n, x: existing?.x, y: existing?.y, fx: existing?.fx, fy: existing?.fy, vx: existing?.vx, vy: existing?.vy };
      });
      const newNodeMap = new Map(newSimNodes.map((n) => [n.id, n]));
      const newSimLinks: SimLink[] = newEdges
        .filter((e) => newNodeMap.has(e.source) && newNodeMap.has(e.target))
        .map((e) => ({ source: e.source, target: e.target, label: e.relation }));

      // Update simulation data
      simulation.nodes(newSimNodes);
      const linkForce = simulation.force<d3.ForceLink<SimNode, SimLink>>('link');
      linkForce?.links(newSimLinks);

      // Update node map for position preservation
      nodeMap.clear();
      for (const n of newSimNodes) { nodeMap.set(n.id, n); }

      // Rebind data to DOM — reassign selections to avoid stale closures
      link = link.data(newSimLinks).join('line')
        .attr('stroke', '#49454F').attr('stroke-opacity', 0.4).attr('stroke-width', 1);
      linkLabel = linkLabel.data(newSimLinks.filter((l) => l.label)).join('text')
        .attr('fill', '#938F99').attr('font-size', 9).attr('text-anchor', 'middle')
        .text((d) => d.label);

      node = node.data(newSimNodes, (d) => d.id)
        .join(
          (enter) => renderNodeGroup(enter, drag, onNodeClick, onNodeHover),
          (update) => {
            update.select('circle').attr('fill', (d) => getColor(d.type));
            update.select('text').text((d) => { const t = d.title || ''; return t.length > 20 ? t.slice(0, 20) + '...' : t; });
            return update;
          },
          (exit) => exit.remove(),
        );

      // Gently reheat simulation
      simulation.alpha(0.3).restart();
    },
    zoomIn: () => svg.transition().duration(300).call(zoom.scaleBy, 1.3),
    zoomOut: () => svg.transition().duration(300).call(zoom.scaleBy, 0.7),
    resetZoom: () => svg.transition().duration(500).call(zoom.transform, d3.zoomIdentity),
  };
}
