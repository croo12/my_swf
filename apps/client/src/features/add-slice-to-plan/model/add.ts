// Starts planning from what exists: brings the modules of one slice into the plan as they are in
// the repository, with their real interfaces and the real dependencies between them.
import { mutatePlan, normalizeImported, planStore, type PlanNode } from '@/entities/plan';
import { loadSliceModules } from '@/entities/repository';
import { notify } from '@/shared/ui/toast';

/** How many modules of the slice the plan holds. Code the server added around the plan does not count. */
export function countSliceInPlan(sliceKey: string, planNodes: PlanNode[]): number {
  return planNodes.filter((n) => !n.observed && (n.path === sliceKey || n.path.startsWith(sliceKey + '/'))).length;
}

/**
 * Adds the slice's modules and the dependencies between them to the plan as existing modules.
 * Modules that are already in the plan are left as they are. Returns whether anything was added.
 */
export async function addSliceToPlan(sliceKey: string): Promise<boolean> {
  const modules = await loadSliceModules(sliceKey, true);
  if (!modules) {
    notify('슬라이스 구조를 읽지 못했습니다. 서버 연결을 확인하세요.');
    return false;
  }
  const { nodes, edges } = normalizeImported({ nodes: modules.nodes, edges: modules.edges });
  const present = new Set(planStore.get().plan.nodes.map((n) => n.id));
  const added = nodes.filter((n) => !present.has(n.id));
  if (added.length === 0) {
    notify('이미 계획에 있는 슬라이스입니다.');
    return false;
  }
  mutatePlan(`${sliceKey}의 모듈 ${added.length}개를 계획에 가져왔습니다.`, (plan) => {
    const known = new Set(plan.edges.map((e) => e.id));
    return {
      ...plan,
      nodes: [...plan.nodes, ...added],
      edges: [...plan.edges, ...edges.filter((e) => !known.has(e.id))],
    };
  });
  return true;
}
