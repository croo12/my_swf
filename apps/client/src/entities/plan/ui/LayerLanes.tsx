import { LANE_H, LANE_LEFT, NODE_W, SLICE_PAD, laneY } from '../model/layout';
import { isDirectLayer, layerOf, sliceOf } from '../model/path';
import { LAYERS, LAYER_DESCRIPTIONS, LAYER_LABELS, type PlanNode } from '../model/types';

/** The six horizontal FSD layer bands behind the nodes. */
export function LayerLanes({ nodes, width }: { nodes: PlanNode[]; width: number }) {
  return (
    <div className="layer-groups" aria-hidden="true">
      {LAYERS.map((layer, i) => (
        <section
          key={layer}
          className="layer-lane"
          data-layer={layer}
          style={{ left: LANE_LEFT, top: laneY(layer), width: width - LANE_LEFT * 2, height: LANE_H }}
        >
          <div className="lane-bar">
            <span className="lane-index">0{i + 1}</span>
            <span className="lane-name">{LAYER_LABELS[layer]}</span>
            <span className="lane-rule">{isDirectLayer(layer) ? 'DIRECT SEGMENTS' : 'SLICES → SEGMENTS'}</span>
            <span className="lane-count">{nodes.filter((n) => layerOf(n) === layer).length} modules</span>
            <div className="lane-desc">{LAYER_DESCRIPTIONS[layer]}</div>
          </div>
        </section>
      ))}
    </div>
  );
}

/** Dashed outlines that group the segments belonging to one slice. */
export function SliceGroups({ nodes }: { nodes: PlanNode[] }) {
  const groups = new Map<string, { layer: string; slice: string; members: PlanNode[] }>();
  for (const n of nodes) {
    const slice = sliceOf(n);
    if (!slice) continue;
    const key = layerOf(n) + '|' + slice;
    const group = groups.get(key) ?? { layer: layerOf(n), slice, members: [] };
    group.members.push(n);
    groups.set(key, group);
  }
  return (
    <div className="slice-groups" aria-hidden="true">
      {[...groups.entries()].map(([key, { layer, slice, members }]) => {
        const left = Math.min(...members.map((n) => n.x)) - SLICE_PAD;
        const right = Math.max(...members.map((n) => n.x + NODE_W)) + SLICE_PAD;
        return (
          <section
            key={key}
            className="slice-group"
            data-layer={layer}
            style={{ left, top: laneY(layer) + 46, width: right - left, height: LANE_H - 52 }}
          >
            <span className="slice-label">
              <span className="slice-symbol">▤</span> {slice}{' '}
              <span className="slice-module-count">
                {members.length} segment{members.length === 1 ? '' : 's'}
              </span>
            </span>
          </section>
        );
      })}
    </div>
  );
}
