import type { EdgeRoute, Point } from '../model/layout';
import { isInvalidEdge } from '../model/rules';
import type { PlanEdge, PlanNode } from '../model/types';

const CORNER = 8;

/** A path along the points with rounded corners. The routes are orthogonal, so corners are right angles. */
function roundedPath(points: Point[]): string {
  if (points.length < 2) return '';
  let d = `M ${points[0].x} ${points[0].y}`;
  for (let i = 1; i < points.length - 1; i++) {
    const [prev, corner, next] = [points[i - 1], points[i], points[i + 1]];
    const before = Math.min(CORNER, Math.hypot(corner.x - prev.x, corner.y - prev.y) / 2);
    const after = Math.min(CORNER, Math.hypot(next.x - corner.x, next.y - corner.y) / 2);
    const enter = {
      x: corner.x - Math.sign(corner.x - prev.x) * before,
      y: corner.y - Math.sign(corner.y - prev.y) * before,
    };
    const leave = {
      x: corner.x + Math.sign(next.x - corner.x) * after,
      y: corner.y + Math.sign(next.y - corner.y) * after,
    };
    d += ` L ${enter.x} ${enter.y} Q ${corner.x} ${corner.y} ${leave.x} ${leave.y}`;
  }
  const last = points.at(-1)!;
  return `${d} L ${last.x} ${last.y}`;
}

interface EdgesProps {
  routes: EdgeRoute[];
  edges: PlanEdge[];
  nodes: PlanNode[];
  width: number;
  height: number;
  /** Relations of this module are emphasized; the others fade. */
  focusId: string | null;
}

/** The relations between modules, drawn along the routes the layout computed. */
export function Edges({ routes, edges, nodes, width, height, focusId }: EdgesProps) {
  const edgeById = new Map(edges.map((e) => [e.id, e]));
  return (
    <svg className="edges" viewBox={`0 0 ${width} ${height}`} style={{ width, height }} aria-label="모듈 관계 그래프">
      <defs>
        {['arrow', 'arrowActive', 'arrowDanger', 'arrowServer'].map((id) => (
          <marker key={id} id={id} viewBox="0 0 10 10" refX="9" refY="5" markerWidth="6" markerHeight="6" orient="auto">
            <path d="M0 0 L10 5 L0 10 z" />
          </marker>
        ))}
      </defs>
      {routes.map((route) => {
        const edge = edgeById.get(route.id);
        // Routes without a plan edge are the links from shared/api to the server boundary.
        const toServer = !edge;
        const invalid = edge ? isInvalidEdge(edge, nodes) : false;
        const active = focusId !== null && (route.from === focusId || route.to === focusId);
        const className = [
          'edge-path',
          toServer && 'server',
          invalid && 'invalid',
          active && 'active',
          focusId !== null && !active && 'muted',
        ]
          .filter(Boolean)
          .join(' ');
        const marker = invalid ? 'arrowDanger' : toServer ? 'arrowServer' : active ? 'arrowActive' : 'arrow';
        return (
          <path key={route.id} className={className} d={roundedPath(route.points)} markerEnd={`url(#${marker})`} />
        );
      })}
    </svg>
  );
}
