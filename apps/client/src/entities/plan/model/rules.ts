// Validation of plans and of the relations inside them.
import { isDirectLayer, isLayer, layerIndex, layerOf, sliceOf } from './path';
import type { Plan, PlanEdge, PlanNode, PlanRequest } from './types';

/** An "imports" edge may only point to a lower layer, or stay within one slice (or within app/shared). */
export function isInvalidEdge(edge: PlanEdge, nodes: PlanNode[]): boolean {
  if (edge.type !== 'imports') return false;
  const a = nodes.find((n) => n.id === edge.from);
  const b = nodes.find((n) => n.id === edge.to);
  if (!a || !b) return true;
  const la = layerOf(a);
  const ai = layerIndex(la);
  const bi = layerIndex(layerOf(b));
  if (ai < 0 || bi < 0) return true;
  if (ai !== bi) return ai > bi;
  return !(isDirectLayer(la) || sliceOf(a) === sliceOf(b));
}

type Loose = Record<string, unknown>;
const stringArray = (value: unknown): string[] => (Array.isArray(value) ? value.map(String) : []);

/** Validates and normalizes a plan from a file, the server, or an AI proposal. Throws on unusable input. */
export function normalizeImported(data: unknown): Plan {
  const doc = data as Loose | null;
  if (!doc || !Array.isArray(doc.nodes) || !Array.isArray(doc.edges)) throw new Error('nodes와 edges가 필요합니다.');
  const nodes = (doc.nodes as Loose[]).map((n, i): PlanNode => {
    const parts = String(n.path ?? '')
      .split('/')
      .filter(Boolean);
    if (!isLayer(parts[0])) throw new Error('알 수 없는 FSD layer: ' + parts[0]);
    let path = String(n.path ?? '');
    if (!isDirectLayer(parts[0]) && parts.length === 2) {
      path = parts.join('/') + '/' + (parts[0] === 'features' || parts[0] === 'entities' ? 'model' : 'ui');
    }
    return {
      ...n,
      id: String(n.id || 'import-' + i),
      path,
      description: String(n.description ?? ''),
      inputs: stringArray(n.inputs),
      outputs: stringArray(n.outputs),
      x: Number(n.x) || 0,
      y: Number(n.y) || 0,
    };
  });
  const known = new Set(nodes.map((n) => n.id));
  const edges = (doc.edges as Loose[])
    .filter((e) => known.has(String(e.from)) && known.has(String(e.to)))
    .map((e, i): PlanEdge => ({
      ...e,
      id: String(e.id || 'edge-import-' + i),
      from: String(e.from),
      to: String(e.to),
      type: String(e.type ?? 'uses'),
    }));
  return {
    version: Number(doc.version || 1),
    nodes,
    edges,
    requests: Array.isArray(doc.requests) ? (doc.requests as PlanRequest[]) : [],
  };
}
