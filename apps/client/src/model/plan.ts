// Planning Workbench schema v0.3 and the pure helpers that operate on it.

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

export const LAYERS = ['app', 'pages', 'widgets', 'features', 'entities', 'shared'] as const;
export type Layer = (typeof LAYERS)[number];

export const EDGE_TYPES = ['uses', 'imports', 'reads', 'emits', 'navigates'] as const;

export const LAYER_LABELS: Record<Layer, string> = {
  app: 'APP', pages: 'PAGES', widgets: 'WIDGETS', features: 'FEATURES', entities: 'ENTITIES', shared: 'SHARED',
};

export const LAYER_DESCRIPTIONS: Record<Layer, string> = {
  app: 'Routing · providers · bootstrap',
  pages: 'Routes and page composition',
  widgets: 'Complex reusable UI blocks',
  features: 'User interactions and use cases',
  entities: 'Business domain models',
  shared: 'Generic UI · API · utilities',
};

export const NODE_W = 265;
export const NODE_H = 201;
export const LANE_TOP = 102;
export const LANE_STEP = 290;
export const LANE_H = 273;
export const LANE_LEFT = 24;
export const CONTENT_LEFT = 194;
export const SLICE_PAD = 16;
export const MAX_HISTORY = 40;

const isLayer = (value: string): value is Layer => (LAYERS as readonly string[]).includes(value);
const isDirectLayer = (layer: string) => layer === 'app' || layer === 'shared';

export const layerOf = (n: PlanNode) => n.path.split('/')[0];
export const partsOf = (n: PlanNode) => n.path.split('/').filter(Boolean);
export const titleOf = (n: PlanNode) => n.path.split('/').slice(1).join('/') || n.path;
export const segmentOf = (n: PlanNode) => (isDirectLayer(layerOf(n)) ? partsOf(n)[1] : partsOf(n)[2]) || 'segment';
export const sliceOf = (n: PlanNode) => (isDirectLayer(layerOf(n)) ? null : partsOf(n)[1] || 'slice');
export const laneY = (layer: string) => LANE_TOP + Math.max(0, (LAYERS as readonly string[]).indexOf(layer)) * LANE_STEP;
export const canvasWidth = (nodes: PlanNode[]) => Math.max(1660, ...nodes.map((n) => n.x + NODE_W + 68));
export const canvasHeight = () => LANE_TOP + LANE_STEP * LAYERS.length + 18;

export const cleanPath = (raw: string) => raw.trim().replace(/^\/+|\/+$/g, '');

/** pages/widgets/features/entities need layer/slice/segment; app/shared need layer/segment. */
export function isValidPath(path: string): boolean {
  const parts = path.split('/');
  if (!isLayer(parts[0])) return false;
  return isDirectLayer(parts[0]) ? parts.length === 2 : parts.length >= 3;
}

/** Places every node inside its layer lane, grouping segments of one slice next to each other. */
export function layoutAll(plan: Plan): Plan {
  const positions = new Map<string, { x: number; y: number }>();
  for (const layer of LAYERS) {
    const groups = new Map<string, PlanNode[]>();
    for (const n of plan.nodes) {
      if (layerOf(n) !== layer) continue;
      const key = sliceOf(n) ?? '@' + segmentOf(n);
      groups.set(key, [...(groups.get(key) ?? []), n]);
    }
    let x = CONTENT_LEFT + 18;
    for (const modules of groups.values()) {
      modules.forEach((n, i) => positions.set(n.id, { x: x + i * (NODE_W + 18), y: laneY(layer) + 49 }));
      x += modules.length * (NODE_W + 18) + 55;
    }
  }
  return { ...plan, nodes: plan.nodes.map((n) => ({ ...n, ...positions.get(n.id) })) };
}

/** An `imports` edge may only point to a lower layer, or stay within one slice (or within app/shared). */
export function isInvalidEdge(edge: PlanEdge, nodes: PlanNode[]): boolean {
  if (edge.type !== 'imports') return false;
  const a = nodes.find((n) => n.id === edge.from);
  const b = nodes.find((n) => n.id === edge.to);
  if (!a || !b) return true;
  const la = layerOf(a);
  const ai = (LAYERS as readonly string[]).indexOf(la);
  const bi = (LAYERS as readonly string[]).indexOf(layerOf(b));
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
    const parts = String(n.path ?? '').split('/').filter(Boolean);
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
    .map((e, i): PlanEdge => ({ ...e, id: String(e.id || 'edge-import-' + i), from: String(e.from), to: String(e.to), type: String(e.type ?? 'uses') }));
  return {
    version: Number(doc.version || 1),
    nodes,
    edges,
    requests: Array.isArray(doc.requests) ? (doc.requests as PlanRequest[]) : [],
  };
}

export const SAMPLE: Plan = {
  version: 1,
  nodes: [
    { id: 'app-router', path: 'app/router', description: '라우트 선언과 페이지 연결', inputs: ['URL', 'routeConfig'], outputs: ['renderPage'], x: 212, y: 151 },
    { id: 'app-providers', path: 'app/providers', description: '전역 Provider와 초기 구성', inputs: ['AppConfig'], outputs: ['Context'], x: 565, y: 151 },
    { id: 'document-page', path: 'pages/document/ui', description: '문서 페이지의 UI 조립', inputs: ['documentId'], outputs: ['documentView'], x: 212, y: 441 },
    { id: 'books-page', path: 'pages/books/ui', description: '도서 탐색·선택 페이지', inputs: ['bookQuery'], outputs: ['bookClick'], x: 565, y: 441 },
    { id: 'purchase-page', path: 'pages/purchase/ui', description: '구매 화면 및 구매 요약', inputs: ['cartId'], outputs: ['purchaseView'], x: 918, y: 441 },
    { id: 'sidebar-ui', path: 'widgets/right-sidebar/ui', description: '공통 사이드바의 탭과 렌더링', inputs: ['activeTab', 'tabClick'], outputs: ['tabChange'], x: 212, y: 731 },
    { id: 'sidebar-model', path: 'widgets/right-sidebar/model', description: '사이드바 활성 탭 상태 관리', inputs: ['bookSelected'], outputs: ['activeTab'], x: 565, y: 731 },
    { id: 'sidebar-nav', path: 'features/sidebar-navigation/model', description: '탭 이동과 문서 포커스 요청', inputs: ['targetTab', 'focusTarget'], outputs: ['navigateIntent'], x: 212, y: 1021 },
    { id: 'book-selection', path: 'features/book-selection/model', description: '책 선택과 선택 상태 변경', inputs: ['bookId'], outputs: ['selectedBook'], x: 565, y: 1021 },
    { id: 'document-focus', path: 'features/document-focus/model', description: '포커스 대상과 이동 요청 관리', inputs: ['sectionId'], outputs: ['focusIntent'], x: 918, y: 1021 },
    { id: 'entity-book', path: 'entities/book/model', description: 'Book 도메인 모델과 식별자', inputs: ['BookId'], outputs: ['Book', 'BookQuery'], x: 212, y: 1311 },
    { id: 'entity-document', path: 'entities/document/model', description: 'Document 도메인 타입', inputs: ['DocumentId'], outputs: ['Document'], x: 565, y: 1311 },
    { id: 'shared-api', path: 'shared/api', description: '공통 HTTP 클라이언트', inputs: ['Request'], outputs: ['Response'], x: 212, y: 1601 },
    { id: 'shared-lib', path: 'shared/lib', description: '범용 유틸리티 함수', inputs: ['Primitive'], outputs: ['Normalized'], x: 565, y: 1601 },
  ],
  edges: [
    { id: 'e1', from: 'app-router', to: 'document-page', type: 'imports' },
    { id: 'e2', from: 'app-router', to: 'books-page', type: 'imports' },
    { id: 'e3', from: 'document-page', to: 'sidebar-ui', type: 'imports' },
    { id: 'e4', from: 'books-page', to: 'sidebar-ui', type: 'imports' },
    { id: 'e5', from: 'sidebar-ui', to: 'sidebar-model', type: 'imports' },
    { id: 'e6', from: 'sidebar-model', to: 'book-selection', type: 'imports' },
    { id: 'e7', from: 'sidebar-ui', to: 'sidebar-nav', type: 'imports' },
    { id: 'e8', from: 'book-selection', to: 'entity-book', type: 'imports' },
    { id: 'e9', from: 'sidebar-nav', to: 'document-page', type: 'navigates' },
    { id: 'e10', from: 'document-focus', to: 'entity-document', type: 'imports' },
  ],
  requests: [],
};

const STORAGE = 'planning-workbench-mvp-v3';

export function loadPlan(): Plan {
  try {
    const saved = JSON.parse(localStorage.getItem(STORAGE) ?? 'null');
    if (saved && Array.isArray(saved.nodes) && Array.isArray(saved.edges)) return saved as Plan;
  } catch {
    // Unreadable storage falls back to the sample graph.
  }
  return structuredClone(SAMPLE);
}

export function storePlan(plan: Plan): void {
  try {
    localStorage.setItem(STORAGE, JSON.stringify(plan));
  } catch {
    // Storage may be unavailable; the in-memory plan still works.
  }
}
