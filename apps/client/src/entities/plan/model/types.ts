// Planning Workbench schema v0.3.

/** Whether the plan creates a module, changes one that exists, or only refers to an existing one. */
export type ModuleKind = 'new' | 'modify' | 'existing';

export const MODULE_KINDS: readonly ModuleKind[] = ['new', 'modify', 'existing'];

export const MODULE_KIND_LABELS: Record<ModuleKind, string> = { new: '신규', modify: '수정', existing: '기존' };

/** Something a module exposes to other modules: a component, hook, function, store, type or constant. */
export interface ModuleInterface {
  name: string;
  description: string;
  /** Props or arguments. Empty when it takes none. */
  inputs: string[];
  /** What it returns, renders or emits. Empty when it gives nothing back. */
  outputs: string[];
}

/** A request a module sends to the server. The server side is outside the plan. */
export interface ServerCall {
  method: string;
  path: string;
  description: string;
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
  /** What the module is responsible for. */
  description: string;
  /** What the module does, one behavior per entry. */
  behaviors?: string[];
  /** Files the module consists of, relative to its placement. */
  files?: string[];
  /** What the module exposes to other modules. */
  interfaces: ModuleInterface[];
  /** Requests to the server that originate in this module. */
  serverCalls: ServerCall[];
}

export interface PlanEdge {
  id: string;
  from: string;
  to: string;
  type: string;
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
