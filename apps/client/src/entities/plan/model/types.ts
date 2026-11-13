// Planning Workbench schema v0.3.

/** Whether the plan creates a module, changes one that exists, or only refers to an existing one. */
export type ModuleKind = 'new' | 'modify' | 'existing';

export const MODULE_KINDS: readonly ModuleKind[] = ['new', 'modify', 'existing'];

export const MODULE_KIND_LABELS: Record<ModuleKind, string> = { new: '신규', modify: '수정', existing: '기존' };

/**
 * What an interface is. The first six are offered to other modules; `server` is a request the
 * module sends to the server.
 */
export type InterfaceKind = 'component' | 'hook' | 'function' | 'store' | 'constant' | 'type' | 'server';

export const INTERFACE_KINDS: readonly InterfaceKind[] = [
  'component',
  'hook',
  'function',
  'store',
  'constant',
  'type',
  'server',
];

export const INTERFACE_KIND_LABELS: Record<InterfaceKind, string> = {
  component: '컴포넌트',
  hook: '훅',
  function: '함수',
  store: '스토어',
  constant: '상수',
  type: '타입',
  server: '서버 요청',
};

/** One point where a module meets the outside. A server request is named "METHOD /path". */
export interface ModuleInterface {
  name: string;
  kind: InterfaceKind;
  /** Behavior as a condition and a result. `condition` is empty for something that always holds. */
  condition: string;
  result: string;
  /** Props, arguments or request data, each written `name: Type`. Empty when there are none. */
  inputs: string[];
  /** Return value or response, each a type or `name: Type`. Empty when there is none. */
  outputs: string[];
}

/**
 * A module in the plan. `path` is where it is (or will be) placed: layer/slice/segment, or
 * layer/segment in app and shared. Several modules may share one placement.
 */
export interface PlanNode {
  id: string;
  /** Module name. Older plans have none; the placement is shown instead. */
  name?: string;
  kind?: ModuleKind;
  path: string;
  /** What the module is responsible for, in one sentence. */
  description: string;
  /** Files the module consists of, relative to its placement. */
  files?: string[];
  /** What the module offers to other modules, and the requests it sends to the server. */
  interfaces: ModuleInterface[];
}

export interface PlanEdge {
  id: string;
  from: string;
  to: string;
  type: string;
  /** Names of the interfaces of `to` that `from` uses. */
  uses?: string[];
}

export interface PlanRequest {
  id: string;
  nodeId: string | null;
  text: string;
  status: string;
}

export interface Plan {
  version: number;
  /** What the plan builds, as stated by whoever proposed it. */
  summary?: string;
  nodes: PlanNode[];
  edges: PlanEdge[];
  requests: PlanRequest[];
}

export const SCHEMA_VERSION = '0.3';

export const LAYERS = ['app', 'pages', 'widgets', 'features', 'entities', 'shared'] as const;
export type Layer = (typeof LAYERS)[number];

export const EDGE_TYPES = ['uses', 'imports', 'reads', 'emits', 'navigates'] as const;

export const LAYER_LABELS: Record<Layer, string> = {
  app: 'APP',
  pages: 'PAGES',
  widgets: 'WIDGETS',
  features: 'FEATURES',
  entities: 'ENTITIES',
  shared: 'SHARED',
};

export const LAYER_DESCRIPTIONS: Record<Layer, string> = {
  app: 'Routing · providers · bootstrap',
  pages: 'Routes and page composition',
  widgets: 'Complex reusable UI blocks',
  features: 'User interactions and use cases',
  entities: 'Business domain models',
  shared: 'Generic UI · API · utilities',
};
