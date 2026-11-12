// Brings one slice of the analyzed repository into the plan as existing modules, for context.
import { kindOf, mutatePlan, planStore, type PlanNode } from '@/entities/plan';
import { repositoryStore, sliceKeyOf } from '@/entities/repository';
import { notify } from '@/shared/ui/toast';

/** Placements that the plan already shows as existing modules. New modules may share a placement with them. */
const existingPaths = (nodes: PlanNode[]) => new Set(nodes.filter((n) => kindOf(n) === 'existing').map((n) => n.path));

/** How many segments of the slice the plan already shows as existing modules. */
export function countSliceInPlan(sliceKey: string, planNodes: PlanNode[]): number {
  const repository = repositoryStore.get();
  if (!repository) return 0;
  const paths = existingPaths(planNodes);
  return repository.nodes.filter((n) => sliceKeyOf(n) === sliceKey && paths.has(n.path)).length;
}

/**
 * Adds the slice's segments to the plan, together with the observed imports between them and the
 * nodes already in the plan. Segments that are already there are left as they are.
 */
export function addSliceToPlan(sliceKey: string): void {
  const repository = repositoryStore.get();
  if (!repository) return;
  const present = existingPaths(planStore.get().plan.nodes);
  const added = repository.nodes
    .filter((n) => sliceKeyOf(n) === sliceKey && !present.has(n.path))
    .map((n) => ({ ...n, kind: 'existing' as const }));
  if (added.length === 0) {
    notify('이미 계획에 있는 슬라이스입니다.');
    return;
  }
  mutatePlan(`${sliceKey}의 세그먼트 ${added.length}개를 계획에 추가했습니다.`, (plan) => {
    const nodes = [...plan.nodes, ...added];
    // Scan ids and plan ids can differ for the same module, so relations are matched by path.
    const idByPath = new Map(nodes.filter((n) => kindOf(n) === 'existing').map((n) => [n.path, n.id]));
    const pathOfScanId = new Map(repository.nodes.map((n) => [n.id, n.path]));
    const known = new Set(plan.edges.map((e) => `${e.from}>${e.to}>${e.type}`));
    const edges = [...plan.edges];
    for (const edge of repository.edges) {
      const from = idByPath.get(pathOfScanId.get(edge.from) ?? '');
      const to = idByPath.get(pathOfScanId.get(edge.to) ?? '');
      if (!from || !to || known.has(`${from}>${to}>${edge.type}`)) continue;
      known.add(`${from}>${to}>${edge.type}`);
      edges.push({ ...edge, id: `${edge.id}-${edges.length}`, from, to });
    }
    return { ...plan, nodes, edges };
  });
}
