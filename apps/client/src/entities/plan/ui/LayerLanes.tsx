import type { GraphLayout, LaneBox, SliceBox } from '../model/layout';
import { LAYER_DESCRIPTIONS, LAYER_LABELS, type PlanNode } from '../model/types';

/** The bands of the FSD layers that hold modules. Layers the plan does not touch are not drawn. */
export function LayerLanes({ lanes }: { lanes: LaneBox[] }) {
  return (
    <div className="layer-groups" aria-hidden="true">
      {lanes.map((lane) => (
        <section
          key={lane.layer}
          className="layer-lane"
          data-layer={lane.layer}
          style={{ left: lane.x, right: lane.x, top: lane.y, height: lane.height }}
        >
          <div className="lane-bar">
            <span className="lane-name">{LAYER_LABELS[lane.layer]}</span>
            <span className="lane-count">모듈 {lane.count}</span>
            <div className="lane-desc">{LAYER_DESCRIPTIONS[lane.layer]}</div>
          </div>
        </section>
      ))}
    </div>
  );
}

/** Dashed outlines that group the modules belonging to one slice. */
export function SliceGroups({ slices }: { slices: SliceBox[] }) {
  return (
    <div className="slice-groups" aria-hidden="true">
      {slices.map((slice) => (
        <section
          key={slice.key}
          className="slice-group"
          data-layer={slice.layer}
          style={{ left: slice.x, top: slice.y, width: slice.width, height: slice.height }}
        >
          <span className="slice-label">
            <span className="slice-symbol">▤</span> {slice.name}
          </span>
        </section>
      ))}
    </div>
  );
}

/**
 * The server, drawn below the layers as the boundary of the plan. It lists the requests the
 * planned modules send; the server side itself is not planned.
 */
export function ServerBoundary({ server, nodes }: { server: GraphLayout['server']; nodes: PlanNode[] }) {
  if (!server) return null;
  const nameOf = (id: string) => nodes.find((n) => n.id === id)?.name ?? id;
  return (
    <section
      className="server-boundary"
      style={{ left: server.x, top: server.y, width: server.width, height: server.height }}
    >
      <header>
        <strong>서버</strong>
        <span>계획 범위 밖 · 요청 {server.endpoints.length}개</span>
      </header>
      {server.endpoints.map((endpoint) => (
        <div
          key={endpoint.method + endpoint.path}
          className="server-endpoint"
          title={'호출하는 모듈: ' + endpoint.modules.map(nameOf).join(', ')}
        >
          <span className="server-method">{endpoint.method}</span>
          <span className="server-path">{endpoint.path}</span>
        </div>
      ))}
    </section>
  );
}
