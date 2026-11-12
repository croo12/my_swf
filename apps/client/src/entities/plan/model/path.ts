// How an FSD path (layer/slice/segment or layer/segment) maps onto a node.
import { LAYERS, type Layer, type ModuleKind, type PlanNode } from './types';

export const isLayer = (value: string): value is Layer => (LAYERS as readonly string[]).includes(value);
/** app and shared hold segments directly; the other layers group segments into slices. */
export const isDirectLayer = (layer: string) => layer === 'app' || layer === 'shared';
export const layerIndex = (layer: string) => (LAYERS as readonly string[]).indexOf(layer);

export const layerOf = (n: PlanNode) => n.path.split('/')[0];
export const partsOf = (n: PlanNode) => n.path.split('/').filter(Boolean);
export const titleOf = (n: PlanNode) => n.path.split('/').slice(1).join('/') || n.path;
/** The segment is the last part; in sliced layers everything between the layer and it names the slice. */
export const segmentOf = (n: PlanNode) => (partsOf(n).length > 1 ? partsOf(n).at(-1) : undefined) ?? 'segment';
/** Slices may sit below grouping folders, so the slice name can have several parts ("home/assignment"). */
export const sliceOf = (n: PlanNode) =>
  isDirectLayer(layerOf(n)) ? null : partsOf(n).slice(1, -1).join('/') || partsOf(n)[1] || 'slice';

export const cleanPath = (raw: string) => raw.trim().replace(/^\/+|\/+$/g, '');

/** pages/widgets/features/entities need layer/slice/segment; app/shared need layer/segment. */
export function isValidPath(path: string): boolean {
  const parts = path.split('/');
  if (!isLayer(parts[0])) return false;
  return isDirectLayer(parts[0]) ? parts.length === 2 : parts.length >= 3;
}

/** Plans made before modules had kinds are treated as new work. */
export const kindOf = (n: PlanNode): ModuleKind => n.kind ?? 'new';
export const moduleNameOf = (n: PlanNode) => n.name?.trim() || [sliceOf(n), segmentOf(n)].filter(Boolean).join(' / ');

/** The same placement in another layer. Slices exist only in the sliced layers, so the shape may change. */
export function pathInLayer(path: string, layer: Layer): string {
  const parts = path.split('/').filter(Boolean);
  const from = parts[0];
  const segment = parts.at(-1) ?? 'model';
  if (isDirectLayer(layer)) return `${layer}/${segment}`;
  if (isDirectLayer(from))
    return `${layer}/${segment}/${layer === 'features' || layer === 'entities' ? 'model' : 'ui'}`;
  return [layer, ...parts.slice(1)].join('/');
}
