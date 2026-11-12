// Design-change requests the user notes down against the plan (optionally tied to the selected node).
import { mutatePlan, selectionStore } from '@/entities/plan';
import { notify } from '@/shared/ui/toast';

/** Records a request. Returns false when the text is empty. */
export function recordRequest(rawText: string): boolean {
  const text = rawText.trim();
  if (!text) {
    notify('수정 요청을 입력하세요.');
    return false;
  }
  const request = { id: 'r-' + Date.now(), nodeId: selectionStore.get().selectedId, text, status: 'pending' };
  mutatePlan('수정 요청을 기록했습니다. AI 실행은 아직 연결되지 않았습니다.', (p) => ({
    ...p,
    requests: [...p.requests, request],
  }));
  return true;
}

export function removeRequest(id: string): void {
  mutatePlan('수정 요청을 제거했습니다.', (p) => ({ ...p, requests: p.requests.filter((r) => r.id !== id) }));
}
