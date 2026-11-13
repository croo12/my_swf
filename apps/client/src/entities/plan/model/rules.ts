// Validation of plans and of the relations inside them.
import { isDirectLayer, isLayer, layerIndex, layerOf, sliceOf } from './path';
import {
  INTERFACE_KINDS,
  MODULE_KINDS,
  type InterfaceKind,
  type ModuleInterface,
  type ModuleKind,
  type Plan,
  type PlanEdge,
  type PlanNode,
  type PlanRequest,
} from './types';

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

const interfaceKind = (value: unknown): InterfaceKind =>
  INTERFACE_KINDS.includes(value as InterfaceKind) ? (value as InterfaceKind) : 'function';

function interfacesOf(n: Loose, fallbackName: string): ModuleInterface[] {
  const interfaces: ModuleInterface[] = Array.isArray(n.interfaces)
    ? (n.interfaces as Loose[])
        .filter((item) => typeof item?.name === 'string' && item.name.trim())
        .map((item) => ({
          name: String(item.name).trim(),
          kind: interfaceKind(item.kind),
          condition: String(item.condition ?? ''),
          result: String(item.result ?? ''),
          inputs: stringArray(item.inputs),
          outputs: stringArray(item.outputs),
        }))
    : [];
  // Plans made before interfaces existed kept one list of inputs and outputs on the module.
  const [inputs, outputs] = [stringArray(n.inputs), stringArray(n.outputs)];
  if (!Array.isArray(n.interfaces) && inputs.length + outputs.length > 0) {
    interfaces.push({ name: fallbackName, kind: 'function', condition: '', result: '', inputs, outputs });
  }
  // An earlier shape listed server requests separately; they are interfaces like the rest.
  for (const call of Array.isArray(n.serverCalls) ? (n.serverCalls as Loose[]) : []) {
    if (typeof call?.path !== 'string' || !call.path.trim()) continue;
    const name = `${String(call.method ?? 'GET').toUpperCase()} ${call.path.trim()}`;
    if (!interfaces.some((item) => item.name === name))
      interfaces.push({ name, kind: 'server', condition: '', result: '', inputs: [], outputs: [] });
  }
  return interfaces;
}

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
      name: typeof n.name === 'string' && n.name.trim() ? n.name.trim() : undefined,
      // Nodes observed on disk by the repository scan are existing modules.
      kind: MODULE_KINDS.includes(n.kind as ModuleKind)
        ? (n.kind as ModuleKind)
        : n.source === 'filesystem'
          ? 'existing'
          : 'new',
      description: String(n.description ?? ''),
      files: stringArray(n.files),
      interfaces: interfacesOf(
        n,
        typeof n.name === 'string' && n.name.trim() ? n.name.trim() : (parts.at(-1) ?? 'module'),
      ),
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
      uses: stringArray(e.uses),
    }));
  return {
    version: Number(doc.version || 1),
    summary: typeof doc.summary === 'string' ? doc.summary : undefined,
    nodes,
    edges,
    requests: Array.isArray(doc.requests) ? (doc.requests as PlanRequest[]) : [],
  };
}
