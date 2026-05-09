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

export interface GraphRenderOptions {
  svgEl: SVGSVGElement;
  width: number;
  height: number;
  nodes: GraphNode[];
  edges: GraphEdge[];
  onNodeClick?: (node: GraphNode) => void;
  onNodeHover?: (node: GraphNode | null) => void;
}

export function renderForceGraph(opts: GraphRenderOptions) {
  const { svgEl, width, height, nodes, edges, onNodeClick, onNodeHover } = opts;

  const svg = d3.select(svgEl);
  svg.selectAll('*').remove();

  const g = svg.append('g');

  const zoom = d3.zoom<SVGSVGElement, unknown>()
    .scaleExtent([0.1, 4])
    .on('zoom', (event: d3.D3ZoomEvent<SVGSVGElement, unknown>) => g.attr('transform', event.transform.toString()));

  svg.call(zoom);

  const simNodes = nodes.map((n) => ({ ...n }));

  const nodeMap = new Map(simNodes.map((n) => [n.id, n]));

  const simLinks = edges
    .filter((e) => nodeMap.has(e.source) && nodeMap.has(e.target))
    .map((e) => ({
      source: e.source,
      target: e.target,
      label: e.relation,
    }));

  const link = g.append('g')
    .attr('class', 'links')
    .selectAll('line')
    .data(simLinks)
    .join('line')
    .attr('stroke', '#49454F')
    .attr('stroke-opacity', 0.4)
    .attr('stroke-width', 1);

  const linkLabel = g.append('g')
    .attr('class', 'link-labels')
    .selectAll('text')
    .data(simLinks.filter((l) => l.label))
    .join('text')
    .attr('fill', '#938F99')
    .attr('font-size', 9)
    .attr('text-anchor', 'middle')
    .text((d) => d.label);

  const node = g.append('g')
    .attr('class', 'nodes')
    .selectAll<SVGGElement, any>('g')
    .data(simNodes)
    .join('g')
    .attr('cursor', 'pointer')
    .call(d3.drag<SVGGElement, any>()
      .on('start', (event: d3.D3DragEvent<SVGGElement, any, any>, d: any) => {
        if (!event.active) simulation.alphaTarget(0.3).restart();
        d.fx = d.x;
        d.fy = d.y;
      })
      .on('drag', (event: d3.D3DragEvent<SVGGElement, any, any>, d: any) => {
        d.fx = event.x;
        d.fy = event.y;
      })
      .on('end', (event: d3.D3DragEvent<SVGGElement, any, any>, d: any) => {
        if (!event.active) simulation.alphaTarget(0);
        d.fx = null;
        d.fy = null;
      })
    );

  node.append('circle')
    .attr('r', 8)
    .attr('fill', (d: any) => getColor(d.type))
    .attr('stroke', '#1C1B1F')
    .attr('stroke-width', 1.5)
    .attr('opacity', 0.9);

  node.append('text')
    .attr('dy', -14)
    .attr('text-anchor', 'middle')
    .attr('fill', '#CAC4D0')
    .attr('font-size', 10)
    .text((d: any) => {
      const title = d.title || '';
      return title.length > 20 ? title.slice(0, 20) + '…' : title;
    });

  node.on('click', (_event: any, d: any) => onNodeClick?.(d))
    .on('mouseenter', (_event: any, d: any) => onNodeHover?.(d))
    .on('mouseleave', () => onNodeHover?.(null));

  const simulation = d3.forceSimulation(simNodes as any)
    .force('link', d3.forceLink(simLinks).id((d: any) => d.id).distance(80))
    .force('charge', d3.forceManyBody().strength(-120))
    .force('center', d3.forceCenter(width / 2, height / 2))
    .force('collision', d3.forceCollide().radius(20));

  simulation.on('tick', () => {
    link
      .attr('x1', (d: any) => d.source.x)
      .attr('y1', (d: any) => d.source.y)
      .attr('x2', (d: any) => d.target.x)
      .attr('y2', (d: any) => d.target.y);

    linkLabel
      .attr('x', (d: any) => (d.source.x + d.target.x) / 2)
      .attr('y', (d: any) => (d.source.y + d.target.y) / 2);

    node.attr('transform', (d: any) => `translate(${d.x},${d.y})`);
  });

  return {
    destroy: () => simulation.stop(),
    zoomIn: () => svg.transition().duration(300).call(zoom.scaleBy, 1.3),
    zoomOut: () => svg.transition().duration(300).call(zoom.scaleBy, 0.7),
    resetZoom: () => svg.transition().duration(500).call(zoom.transform, d3.zoomIdentity),
  };
}
