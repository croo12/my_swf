// The plan being edited, its undo history, and local persistence.
import { createStore } from '@/shared/lib';
import { notify } from '@/shared/ui/toast';
import { normalizeImported } from './rules';
import { clearSelection, selectionStore } from './selection';
import type { Plan } from './types';

// v4: plans start empty and hold planned modules; older stored graphs are left alone under the old key.
const STORAGE_KEY = 'planning-workbench-plan-v4';
const MAX_HISTORY = 40;

/** Nothing exists before planning: a plan starts with no modules. */
export const emptyPlan = (): Plan => ({ version: 1, nodes: [], edges: [], requests: [] });

export interface PlanState {
  plan: Plan;
  history: Plan[];
}

function loadPlan(): Plan {
  try {
    const saved = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? 'null');
    // Normalizing brings plans stored by an older version up to the current shape.
    if (saved && Array.isArray(saved.nodes) && Array.isArray(saved.edges)) return normalizeImported(saved);
  } catch {
    // Unreadable storage falls back to an empty plan.
  }
  return emptyPlan();
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
  planStore.set({ plan: next, history: [] });
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
