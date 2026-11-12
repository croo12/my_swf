// User actions that change the graph: nodes, relations, layout, undo.
import {
  CONTENT_LEFT,
  EDGE_TYPES,
  SAMPLE,
  clearSelection,
  closeEditor,
  commitPlan,
  laneY,
  layerOf,
  layoutAll,
  mutatePlan,
  openEditor,
  planStore,
  selectionStore,
  sliceOf,
  startConnecting,
  stopConnecting,
  undoPlan,
} from '@/entities/plan';
import { createStore } from '@/shared/lib';
import { notify } from '@/shared/ui/toast';

export interface NodeDraft {
  path: string;
  description: string;
  inputs: string[];
  outputs: string[];
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
    notify('연결할 출발 노드를 먼저 선택하세요.');
    return;
  }
  startConnecting(selectedId);
}

export function moveNode(id: string, x: number, y: number): void {
  commitPlan((p) => ({ ...p, nodes: p.nodes.map((n) => (n.id === id ? { ...n, x, y } : n)) }));
}

export function addNode(): void {
  const id = 'module-' + Date.now();
  const node = {
    id,
    path: 'features/new-feature/model',
    description: '새 모듈의 책임을 적어주세요.',
    inputs: [],
    outputs: [],
    x: CONTENT_LEFT + 20,
    y: laneY('features') + 62,
  };
  mutatePlan('새 모듈이 추가되었습니다.', (p) => layoutAll({ ...p, nodes: [...p.nodes, node] }));
  openEditor(id);
}

export function saveNode(id: string, draft: NodeDraft): void {
  const current = planStore.get().plan.nodes.find((n) => n.id === id);
  if (!current) return;
  const updated = { ...current, ...draft };
  // Re-run the layout only when the node moved to another layer or slice.
  const changedGroup = layerOf(current) !== layerOf(updated) || sliceOf(current) !== sliceOf(updated);
  mutatePlan('노드 설계 변경이 반영되었습니다.', (p) => {
    const next = { ...p, nodes: p.nodes.map((n) => (n.id === id ? updated : n)) };
    return changedGroup ? layoutAll(next) : next;
  });
  closeEditor();
}

export function deleteNode(id: string): void {
  const node = planStore.get().plan.nodes.find((n) => n.id === id);
  if (!node || !confirm(`${node.path} 노드와 연결을 삭제할까요?`)) return;
  mutatePlan('노드가 삭제되었습니다.', (p) => ({
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

export function autoLayout(): void {
  mutatePlan('Layer → Slice → Segment 기준으로 정렬했습니다.', layoutAll);
}

export function undo(): void {
  if (undoPlan()) notify('이전 상태로 되돌렸습니다.');
}

export function resetToSample(): void {
  if (!confirm('현재 설계를 지우고 샘플 상태로 초기화할까요?')) return;
  mutatePlan('샘플 그래프로 초기화했습니다.', () => structuredClone(SAMPLE));
  clearSelection();
}
