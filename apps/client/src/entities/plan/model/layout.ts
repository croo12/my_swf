// Automatic graph layout. ELK places the modules top-down by FSD layer and routes the relations
// as orthogonal lines around them; nothing is positioned by hand.
import type { ELK, ElkExtendedEdge, ElkNode } from 'elkjs/lib/elk-api';
import { createStore } from '@/shared/lib';
import { layerIndex, layerOf, sliceOf } from './path';
import { planStore } from './store';
import { LAYERS, type Layer, type Plan, type PlanNode } from './types';

export const NODE_W = 265;
export const NODE_H = 201;
/** Room on the left of every lane for its name. */
const LANE_LABEL_W = 170;
const LANE_PAD = 26;
const SERVER_ID = '@server';
const SERVER_ROW_H = 26;
/** Distances between cards and lines. Set on the root and on every slice box; ELK does not inherit them. */
const SPACING = {
  'elk.layered.spacing.nodeNodeBetweenLayers': '56',
  'elk.layered.spacing.edgeNodeBetweenLayers': '24',
  'elk.spacing.nodeNode': '36',
  'elk.spacing.edgeNode': '24',
  'elk.spacing.edgeEdge': '14',
};

export interface Point {
  x: number;
  y: number;
}
export interface Box extends Point {
  width: number;
  height: number;
}
export interface LaneBox extends Box {
  layer: Layer;
  count: number;
}
export interface SliceBox extends Box {
  key: string;
  layer: string;
  name: string;
  count: number;
}
export interface EdgeRoute {
  id: string;
  from: string;
  to: string;
  points: Point[];
}
/** One request to the server, with the modules it originates in. */
export interface ServerEndpoint {
  method: string;
  path: string;
  modules: string[];
}

export interface GraphLayout {
  /** Plan version the layout was computed for. */
  version: number;
  width: number;
  height: number;
  positions: Record<string, Point>;
  /** Only the layers that hold modules. */
  lanes: LaneBox[];
  slices: SliceBox[];
  edges: EdgeRoute[];
  /** The server boundary below the layers; present only when modules call the server. */
  server: (Box & { endpoints: ServerEndpoint[] }) | null;
}

const EMPTY: GraphLayout = {
  version: 0,
  width: 0,
  height: 0,
  positions: {},
  lanes: [],
  slices: [],
  edges: [],
  server: null,
};

export const layoutStore = createStore<GraphLayout>(EMPTY);

/** Every server request named by the plan, merged by method and path. */
export function serverEndpoints(nodes: PlanNode[]): ServerEndpoint[] {
  const endpoints = new Map<string, ServerEndpoint>();
  for (const node of nodes) {
    for (const item of node.interfaces.filter((i) => i.kind === 'server')) {
      const [method, ...rest] = item.name.split(' ');
      const endpoint = endpoints.get(item.name) ?? { method, path: rest.join(' '), modules: [] };
      endpoint.modules.push(node.id);
      endpoints.set(item.name, endpoint);
    }
  }
  return [...endpoints.values()];
}

/** Requests leave the client through the modules placed in shared/api. */
export const isApiModule = (node: PlanNode) => node.path === 'shared/api' || node.path.startsWith('shared/api/');

let elk: Promise<ELK> | undefined;
/** ELK is large, so it is loaded only when a graph is first laid out. */
const loadElk = () => (elk ??= import('elkjs/lib/elk.bundled.js').then((module) => new module.default()));

/** ELK keeps nodes with a lower partition number above the others: one partition per FSD layer. */
const partition = (layer: string) => ({ 'elk.partitioning.partition': String(layerIndex(layer)) });
const box = (n: ElkNode): Box => ({ x: n.x ?? 0, y: n.y ?? 0, width: n.width ?? 0, height: n.height ?? 0 });

async function computeLayout(plan: Plan): Promise<GraphLayout> {
  if (plan.nodes.length === 0) return { ...EMPTY, version: plan.version };
  const ids = new Set(plan.nodes.map((n) => n.id));

  // Modules of one slice are laid out inside a box for that slice; app and shared have no slices.
  const children: ElkNode[] = [];
  const groups = new Map<string, ElkNode>();
  for (const node of plan.nodes) {
    const layer = layerOf(node);
    const slice = sliceOf(node);
    const item: ElkNode = { id: node.id, width: NODE_W, height: NODE_H };
    if (!slice) {
      children.push({ ...item, layoutOptions: partition(layer) });
      continue;
    }
    const key = `${layer}/${slice}`;
    let group = groups.get(key);
    if (!group) {
      group = {
        id: 'slice:' + key,
        children: [],
        layoutOptions: { ...partition(layer), ...SPACING, 'elk.padding': '[top=34,left=16,bottom=16,right=16]' },
      };
      groups.set(key, group);
      children.push(group);
    }
    group.children?.push(item);
  }

  const edges: ElkExtendedEdge[] = plan.edges
    .filter((e) => ids.has(e.from) && ids.has(e.to) && e.from !== e.to)
    .map((e) => ({ id: e.id, sources: [e.from], targets: [e.to] }));

  // The server is shown as one box below all layers, reached from the shared/api modules.
  const endpoints = serverEndpoints(plan.nodes);
  if (endpoints.length > 0) {
    children.push({
      id: SERVER_ID,
      width: 420,
      height: 44 + endpoints.length * SERVER_ROW_H,
      layoutOptions: { 'elk.partitioning.partition': String(LAYERS.length) },
    });
    for (const node of plan.nodes.filter(isApiModule)) {
      edges.push({ id: `${SERVER_ID}:${node.id}`, sources: [node.id], targets: [SERVER_ID] });
    }
  }

  const laidOut: ElkNode = await (
    await loadElk()
  ).layout({
    id: 'root',
    children,
    edges,
    layoutOptions: {
      'elk.algorithm': 'layered',
      'elk.direction': 'DOWN',
      'elk.partitioning.activate': 'true',
      'elk.hierarchyHandling': 'INCLUDE_CHILDREN',
      'elk.edgeRouting': 'ORTHOGONAL',
      ...SPACING,
      'elk.padding': `[top=${LANE_PAD + 14},left=${LANE_LABEL_W + LANE_PAD},bottom=${LANE_PAD},right=${LANE_PAD}]`,
      'elk.json.shapeCoords': 'ROOT',
      'elk.json.edgeCoords': 'ROOT',
    },
  });

  const positions: Record<string, Point> = {};
  const slices: SliceBox[] = [];
  let server: GraphLayout['server'] = null;
  // Vertical extent of each layer, from the boxes placed in it.
  const extent = new Map<string, { top: number; bottom: number; count: number }>();
  const grow = (layer: string, b: Box, count: number) => {
    const e = extent.get(layer) ?? { top: Infinity, bottom: -Infinity, count: 0 };
    extent.set(layer, {
      top: Math.min(e.top, b.y),
      bottom: Math.max(e.bottom, b.y + b.height),
      count: e.count + count,
    });
  };
  const layerOfId = new Map(plan.nodes.map((n) => [n.id, layerOf(n)]));
  for (const child of laidOut.children ?? []) {
    if (child.id === SERVER_ID) {
      server = { ...box(child), endpoints };
    } else if (child.id.startsWith('slice:')) {
      const key = child.id.slice('slice:'.length);
      const layer = key.split('/')[0];
      const members = child.children ?? [];
      slices.push({ ...box(child), key, layer, name: key.slice(layer.length + 1), count: members.length });
      grow(layer, box(child), members.length);
      for (const member of members) positions[member.id] = { x: member.x ?? 0, y: member.y ?? 0 };
    } else {
      positions[child.id] = { x: child.x ?? 0, y: child.y ?? 0 };
      grow(layerOfId.get(child.id) ?? '', box(child), 1);
    }
  }

  const width = Math.max(laidOut.width ?? 0, 900);
  const lanes: LaneBox[] = LAYERS.filter((layer) => extent.has(layer)).map((layer) => {
    const e = extent.get(layer)!;
    return {
      layer,
      count: e.count,
      x: LANE_PAD - 8,
      y: e.top - LANE_PAD,
      width: width - (LANE_PAD - 8) * 2,
      height: e.bottom - e.top + LANE_PAD * 2,
    };
  });

  const routes: EdgeRoute[] = (laidOut.edges ?? []).map((edge) => {
    const section = edge.sections?.[0];
    const points = section ? [section.startPoint, ...(section.bendPoints ?? []), section.endPoint] : [];
    return { id: edge.id, from: edge.sources[0], to: edge.targets[0], points };
  });

  return {
    version: plan.version,
    width,
    height: (laidOut.height ?? 0) + LANE_PAD,
    positions,
    lanes,
    slices,
    edges: routes,
    server,
  };
}

/** The layer whose lane contains the given vertical position, among the lanes shown. */
export function layerAtY(y: number, lanes: LaneBox[]): Layer | null {
  return lanes.find((lane) => y >= lane.y && y <= lane.y + lane.height)?.layer ?? null;
}

// Re-run the layout whenever the plan changes. Results that arrive out of order are dropped.
let requested = 0;
function refreshLayout(): void {
  const { plan } = planStore.get();
  const ticket = ++requested;
  void computeLayout(plan)
    .then((layout) => {
      if (ticket === requested) layoutStore.set(layout);
    })
    .catch((error: unknown) => console.error('Graph layout failed', error));
}
planStore.subscribe(refreshLayout);
refreshLayout();
