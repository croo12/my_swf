// Canvas geometry: where lanes, slices, and nodes sit.
import { layerIndex, layerOf, segmentOf, sliceOf } from './path';
import { LAYERS, type Layer, type Plan, type PlanNode } from './types';

export const NODE_W = 265;
export const NODE_H = 201;
export const LANE_TOP = 24;
export const LANE_STEP = 290;
export const LANE_H = 273;
export const LANE_LEFT = 24;
export const CONTENT_LEFT = 194;
export const SLICE_PAD = 16;
const NODE_GAP = 18;
const GROUP_GAP = 55;

export const laneY = (layer: string) => LANE_TOP + Math.max(0, layerIndex(layer)) * LANE_STEP;
/** A node sits at a fixed height inside the lane of its layer; only x is free. */
export const nodeY = (layer: string) => laneY(layer) + 62;
export const canvasWidth = (nodes: PlanNode[]) => Math.max(1660, ...nodes.map((n) => n.x + NODE_W + 68));
export const canvasHeight = () => LANE_TOP + LANE_STEP * LAYERS.length + 18;

/** Places every node inside its layer lane, keeping the segments of one slice next to each other. */
export function layoutAll(plan: Plan): Plan {
  const positions = new Map<string, { x: number; y: number }>();
  for (const layer of LAYERS) {
    const groups = new Map<string, PlanNode[]>();
    for (const n of plan.nodes) {
      if (layerOf(n) !== layer) continue;
      const key = sliceOf(n) ?? '@' + segmentOf(n);
      groups.set(key, [...(groups.get(key) ?? []), n]);
    }
    let x = CONTENT_LEFT + NODE_GAP;
    for (const modules of groups.values()) {
      modules.forEach((n, i) => positions.set(n.id, { x: x + i * (NODE_W + NODE_GAP), y: nodeY(layer) }));
      x += modules.length * (NODE_W + NODE_GAP) + GROUP_GAP;
    }
  }
  return { ...plan, nodes: plan.nodes.map((n) => ({ ...n, ...positions.get(n.id) })) };
}

/** The layer whose lane contains the given vertical position (clamped to the first and last lane). */
export function layerAtY(y: number): Layer {
  const index = Math.floor((y - LANE_TOP) / LANE_STEP);
  return LAYERS[Math.max(0, Math.min(LAYERS.length - 1, index))];
}
