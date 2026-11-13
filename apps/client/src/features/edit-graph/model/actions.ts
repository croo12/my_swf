// User actions that change the plan: modules, relations, placement, undo.
import {
  EDGE_TYPES,
  clearSelection,
  emptyPlan,
  pathInLayer,
  type Layer,
  type ModuleInterface,
  type ModuleKind,
  closeEditor,
  layerOf,
  commitPlan,
  mutatePlan,
  openEditor,
  planStore,
  selectionStore,
  startConnecting,
  stopConnecting,
  undoPlan,
} from '@/entities/plan';
import { createStore } from '@/shared/lib';
import { notify } from '@/shared/ui/toast';

export interface NodeDraft {
  name: string;
  kind: ModuleKind;
  path: string;
  description: string;
  files: string[];
  interfaces: ModuleInterface[];
}

/** Relation type used for the next connection, chosen in the toolbar. */
export const edgeTypeStore = createStore<string>(EDGE_TYPES[0]);

/** A click on a node: finishes a pending connection, otherwise opens the editor. */
export function activateNode(id: string): void {
  const from = selectionStore.get().connectFromId;
  if (!from) {
    openEditor(id);
    return;
  }
  stopConnecting();
  if (from === id) return;
  const type = edgeTypeStore.get();
  if (planStore.get().plan.edges.some((e) => e.from === from && e.to === id && e.type === type)) {
    notify('이미 존재하는 연결입니다.');
    return;
  }
  const edge = { id: 'edge-' + Date.now(), from, to: id, type };
  mutatePlan(`${type} 관계를 추가했습니다.`, (p) => ({ ...p, edges: [...p.edges, edge] }));
}

/** Starts a connection from the given node, or cancels it if that node is already the source. */
export function toggleConnectFrom(id: string): void {
  if (selectionStore.get().connectFromId === id) stopConnecting();
  else startConnecting(id);
}

/** Toolbar variant: connects from the currently selected node. */
export function toggleConnectFromSelected(): void {
  const { connectFromId, selectedId } = selectionStore.get();
  if (connectFromId) {
    stopConnecting();
    return;
  }
  if (!selectedId) {
    notify('연결할 출발 모듈을 먼저 선택하세요.');
    return;
  }
  startConnecting(selectedId);
}

/** Adds a blank module to place and describe by hand. */
export function addNode(): void {
  const id = 'module-' + Date.now();
  const node = {
    id,
    name: '새 모듈',
    kind: 'new' as const,
    path: 'features/new-feature/model',
    description: '',
    interfaces: [],
  };
  mutatePlan('새 모듈을 추가했습니다. 이름과 배치 위치를 정하세요.', (p) => ({ ...p, nodes: [...p.nodes, node] }));
  openEditor(id);
}

/** Moves a module to another layer; its slice and segment are kept where the target layer has them. */
export function relocateNode(id: string, layer: Layer): void {
  const node = planStore.get().plan.nodes.find((n) => n.id === id);
  if (!node || layerOf(node) === layer) return;
  const path = pathInLayer(node.path, layer);
  mutatePlan(`배치를 ${path}(으)로 옮겼습니다.`, (p) => ({
    ...p,
    nodes: p.nodes.map((n) => (n.id === id ? { ...n, path } : n)),
  }));
}

export function saveNode(id: string, draft: NodeDraft): void {
  const current = planStore.get().plan.nodes.find((n) => n.id === id);
  if (!current) return;
  const updated = { ...current, ...draft };
  mutatePlan('모듈 변경이 반영되었습니다.', (p) => ({ ...p, nodes: p.nodes.map((n) => (n.id === id ? updated : n)) }));
  closeEditor();
}

export function deleteNode(id: string): void {
  const node = planStore.get().plan.nodes.find((n) => n.id === id);
  if (!node || !confirm(`${node.name ?? node.path} 모듈과 그 연결을 삭제할까요?`)) return;
  mutatePlan('모듈을 삭제했습니다.', (p) => ({
    ...p,
    nodes: p.nodes.filter((n) => n.id !== id),
    edges: p.edges.filter((e) => e.from !== id && e.to !== id),
    requests: p.requests.filter((r) => r.nodeId !== id),
  }));
  clearSelection();
}

export function removeEdge(edgeId: string): void {
  mutatePlan('연결을 삭제했습니다.', (p) => ({ ...p, edges: p.edges.filter((e) => e.id !== edgeId) }));
}

/** Sets which interfaces of the target a relation uses. */
export function setEdgeUses(edgeId: string, uses: string[]): void {
  commitPlan((p) => ({ ...p, edges: p.edges.map((e) => (e.id === edgeId ? { ...e, uses } : e)) }));
}

export function undo(): void {
  if (undoPlan()) notify('이전 상태로 되돌렸습니다.');
}

/** Empties the plan. The previous plan can be brought back with undo. */
export function clearPlan(): void {
  if (planStore.get().plan.nodes.length > 0 && !confirm('계획의 모듈을 모두 지울까요?')) return;
  mutatePlan('계획을 비웠습니다.', (p) => ({ ...emptyPlan(), version: p.version }));
  clearSelection();
}
