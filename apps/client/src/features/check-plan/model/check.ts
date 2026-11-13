// Has the server check the plan on screen: the same static rules that proposals must pass, plus
// the code around the plan that the repository's imports connect it to.
import { commitPlan, normalizeImported, planStore, type PlanEdge, type PlanNode } from '@/entities/plan';
import { apiRequest, errorMessage } from '@/shared/api';
import { createStore } from '@/shared/lib';
import { notify } from '@/shared/ui/toast';

export interface CheckResult {
  /** One sentence per broken rule. Empty when the plan passes. */
  errors: string[];
  /** Branch and commit of the repository the plan was checked against. */
  basedOn: string;
  checkedAt: string;
}

export interface CheckState {
  running: boolean;
  /** The last result, until the panel is closed. */
  result: CheckResult | null;
}

export const checkStore = createStore<CheckState>({ running: false, result: null });

export const closeCheckResult = () => checkStore.set({ ...checkStore.get(), result: null });

interface CheckResponse {
  errors: string[];
  nodes: PlanNode[];
  edges: PlanEdge[];
  git?: { branch: string; commit: string } | null;
}

const withoutObserved = <T extends { observed?: boolean }>(items: T[]) => items.filter((item) => !item.observed);

const observedOf = (p: { nodes: PlanNode[]; edges: PlanEdge[] }) =>
  JSON.stringify([p.nodes.filter((n) => n.observed), p.edges.filter((e) => e.observed)]);

/** Checks the plan and replaces the surrounding code shown in it with what the server found. */
export async function checkPlan(): Promise<void> {
  if (checkStore.get().running) return;
  checkStore.set({ ...checkStore.get(), running: true });
  try {
    const { plan } = planStore.get();
    const response = await apiRequest<CheckResponse>('/api/plan/check', 'POST', { plan });
    const next = normalizeImported({
      ...plan,
      nodes: [...withoutObserved(plan.nodes), ...response.nodes],
      edges: [...withoutObserved(plan.edges), ...response.edges],
    });
    // The surroundings are facts about the repository, not an edit; only a real change is recorded.
    if (observedOf(next) !== observedOf(plan)) commitPlan(() => next);
    checkStore.set({
      running: false,
      result: {
        errors: response.errors,
        basedOn: response.git ? `${response.git.branch} @ ${response.git.commit}` : '',
        checkedAt: new Date().toISOString(),
      },
    });
  } catch (err) {
    checkStore.set({ ...checkStore.get(), running: false });
    notify('계획 검사 실패: ' + errorMessage(err));
  }
}
