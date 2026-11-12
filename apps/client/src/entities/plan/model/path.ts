// How an FSD path (layer/slice/segment or layer/segment) maps onto a node.
import { LAYERS, type Layer, type PlanNode } from './types';

export const isLayer = (value: string): value is Layer => (LAYERS as readonly string[]).includes(value);
/** app and shared hold segments directly; the other layers group segments into slices. */
export const isDirectLayer = (layer: string) => layer === 'app' || layer === 'shared';
export const layerIndex = (layer: string) => (LAYERS as readonly string[]).indexOf(layer);

export const layerOf = (n: PlanNode) => n.path.split('/')[0];
export const partsOf = (n: PlanNode) => n.path.split('/').filter(Boolean);
export const titleOf = (n: PlanNode) => n.path.split('/').slice(1).join('/') || n.path;
export const segmentOf = (n: PlanNode) => (isDirectLayer(layerOf(n)) ? partsOf(n)[1] : partsOf(n)[2]) || 'segment';
export const sliceOf = (n: PlanNode) => (isDirectLayer(layerOf(n)) ? null : partsOf(n)[1] || 'slice');

export const cleanPath = (raw: string) => raw.trim().replace(/^\/+|\/+$/g, '');

/** pages/widgets/features/entities need layer/slice/segment; app/shared need layer/segment. */
export function isValidPath(path: string): boolean {
  const parts = path.split('/');
  if (!isLayer(parts[0])) return false;
  return isDirectLayer(parts[0]) ? parts.length === 2 : parts.length >= 3;
}
