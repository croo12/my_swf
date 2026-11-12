// The plan being edited, its undo history, and local persistence.
import { createStore } from '@/shared/lib';
import { notify } from '@/shared/ui/toast';
import { layoutAll, nodeY } from './layout';
import { layerOf } from './path';
import { SAMPLE } from './sample';
import { clearSelection, selectionStore } from './selection';
import type { Plan } from './types';

const STORAGE_KEY = 'planning-workbench-mvp-v3';
const MAX_HISTORY = 40;

export interface PlanState {
  plan: Plan;
  history: Plan[];
}

function loadPlan(): Plan {
  try {
    const saved = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? 'null');
    if (saved && Array.isArray(saved.nodes) && Array.isArray(saved.edges)) return snapToLanes(saved as Plan);
  } catch {
    // Unreadable storage falls back to the sample graph.
  }
  return snapToLanes(structuredClone(SAMPLE));
}

/** Stored plans may carry heights from an older lane layout; the layer decides the height. */
function snapToLanes(plan: Plan): Plan {
  return { ...plan, nodes: plan.nodes.map((n) => ({ ...n, y: nodeY(layerOf(n)) })) };
}

export const planStore = createStore<PlanState>({ plan: loadPlan(), history: [] });

planStore.subscribe(() => {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(planStore.get().plan));
  } catch {
    // Storage may be unavailable; the in-memory plan still works.
  }
});

/** Applies one undoable change and bumps the plan version. */
export function commitPlan(change: (plan: Plan) => Plan): void {
  planStore.set(({ plan, history }) => {
    const next = change(plan);
    return { plan: { ...next, version: next.version + 1 }, history: [...history, plan].slice(-MAX_HISTORY) };
  });
}

/** Same as commitPlan, plus a toast describing what changed. */
export function mutatePlan(label: string, change: (plan: Plan) => Plan): void {
  commitPlan(change);
  notify(label);
}

/** Replaces the whole plan, for example with the copy stored on the server. This is not undoable. */
export function replacePlan(next: Plan): void {
  planStore.set({ plan: layoutAll(next), history: [] });
  clearSelection();
}

/** Steps back one change. Returns false when there is nothing to undo. */
export function undoPlan(): boolean {
  const { history } = planStore.get();
  const previous = history.at(-1);
  if (!previous) return false;
  planStore.set({ plan: previous, history: history.slice(0, -1) });
  const stillExists = previous.nodes.some((n) => n.id === selectionStore.get().selectedId);
  selectionStore.set((s) => ({
    ...s,
    selectedId: stillExists ? s.selectedId : null,
    editorOpen: false,
    connectFromId: null,
  }));
  return true;
}
