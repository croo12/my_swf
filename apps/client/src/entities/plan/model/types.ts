// Planning Workbench schema v0.3.

export interface PlanNode {
  id: string;
  path: string;
  description: string;
  inputs: string[];
  outputs: string[];
  x: number;
  y: number;
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
