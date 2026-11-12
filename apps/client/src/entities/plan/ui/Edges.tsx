import { NODE_H, NODE_W, laneY } from '../model/layout';
import { layerOf } from '../model/path';
import { isInvalidEdge } from '../model/rules';
import type { PlanEdge, PlanNode } from '../model/types';

/** Same-layer edges arc above the nodes; cross-layer edges run between the facing node edges. */
function edgeGeometry(a: PlanNode, b: PlanNode) {
  if (layerOf(a) === layerOf(b)) {
    const forwards = b.x > a.x;
    const x1 = forwards ? a.x + NODE_W : a.x;
    const x2 = forwards ? b.x : b.x + NODE_W;
    const y1 = a.y + 100;
    const y2 = b.y + 100;
    const lift = (33 + (Math.abs(x2 - x1) < 150 ? 30 : 0)) * (forwards ? 1 : -1);
    return {
      d: `M ${x1} ${y1} C ${x1 + lift} ${y1 - 56}, ${x2 - lift} ${y2 - 56}, ${x2} ${y2}`,
      cx: (x1 + x2) / 2,
      cy: (y1 + y2) / 2 - 44,
    };
  }
  const downward = laneY(layerOf(b)) > laneY(layerOf(a));
  const x1 = a.x + NODE_W / 2;
  const x2 = b.x + NODE_W / 2;
  const y1 = downward ? a.y + NODE_H : a.y;
  const y2 = downward ? b.y : b.y + NODE_H;
  const bend = Math.max(45, Math.abs(y2 - y1) * 0.42) * (downward ? 1 : -1);
  return {
    d: `M ${x1} ${y1} C ${x1} ${y1 + bend}, ${x2} ${y2 - bend}, ${x2} ${y2}`,
    cx: (x1 + x2) / 2,
    cy: (y1 + y2) / 2,
  };
}

/** The relation arrows between nodes, with a label on each. Invalid imports are drawn in red. */
export function Edges({
  edges,
  nodes,
  width,
  height,
}: {
  edges: PlanEdge[];
  nodes: PlanNode[];
  width: number;
  height: number;
}) {
  return (
    <svg className="edges" viewBox={`0 0 ${width} ${height}`} style={{ width, height }} aria-label="모듈 관계 그래프">
      <defs>
        <marker id="arrow" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="5" markerHeight="5" orient="auto">
          <path d="M0 0 L10 5 L0 10 z" fill="#758eab" />
        </marker>
        <marker id="arrowDanger" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="5" markerHeight="5" orient="auto">
          <path d="M0 0 L10 5 L0 10 z" fill="#f18081" />
        </marker>
      </defs>
      {edges.map((e) => {
        const a = nodes.find((n) => n.id === e.from);
        const b = nodes.find((n) => n.id === e.to);
        if (!a || !b) return null;
        const { d, cx, cy } = edgeGeometry(a, b);
        const bad = isInvalidEdge(e, nodes);
        const pathClass = ['edge-path', e.type === 'navigates' && 'navigation', bad && 'invalid']
          .filter(Boolean)
          .join(' ');
        return (
          <g key={e.id}>
            <path className={pathClass} d={d} markerEnd={`url(#${bad ? 'arrowDanger' : 'arrow'})`} />
            <g className="edge-badge">
              <rect className="edge-label-bg" x={cx - 33} y={cy - 11} width="66" height="20" rx="7" />
              <text x={cx} y={cy + 2} textAnchor="middle">
                {e.type}
                {bad ? ' ⚠' : ''}
              </text>
            </g>
          </g>
        );
      })}
    </svg>
  );
}
