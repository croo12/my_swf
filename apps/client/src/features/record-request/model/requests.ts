// Design-change requests the user notes down, either against one node or against the whole graph.
import { mutatePlan } from '@/entities/plan';
import { notify } from '@/shared/ui/toast';

/** Records a request for a node, or for the whole graph when nodeId is null. Returns false when the text is empty. */
export function recordRequest(rawText: string, nodeId: string | null): boolean {
  const text = rawText.trim();
  if (!text) {
    notify('수정 요청을 입력하세요.');
    return false;
  }
  const request = { id: 'r-' + Date.now(), nodeId, text, status: 'pending' };
  mutatePlan('요청을 기록했습니다.', (p) => ({
    ...p,
    requests: [...p.requests, request],
  }));
  return true;
}

export function removeRequest(id: string): void {
  mutatePlan('요청을 제거했습니다.', (p) => ({ ...p, requests: p.requests.filter((r) => r.id !== id) }));
}
