// Saving, loading, approving, and analyzing plans on the Factory server.
import { SCHEMA_VERSION, normalizeImported, planStore, replacePlan } from '@/entities/plan';
import { apiRequest, errorMessage } from '@/shared/api';
import { notify } from '@/shared/ui/toast';

const uploadDraft = () =>
  apiRequest('/api/plan', 'PUT', { schemaVersion: SCHEMA_VERSION, planningState: 'draft', ...planStore.get().plan });

export async function saveToServer(): Promise<void> {
  try {
    await uploadDraft();
    notify('Factory 서버에 Plan을 저장했습니다.');
  } catch (err) {
    notify('저장 실패: ' + errorMessage(err));
  }
}

export async function loadFromServer(): Promise<void> {
  if (!confirm('서버 저장본으로 현재 그래프를 교체하시겠습니까?')) return;
  try {
    const data = await apiRequest<{ plan: unknown }>('/api/plan');
    replacePlan(normalizeImported(data.plan));
    notify('서버 Plan을 불러왔습니다.');
  } catch (err) {
    notify('불러오기 실패: ' + errorMessage(err));
  }
}

/** Saves the current draft, then asks the server to keep it as an immutable approved snapshot. */
export async function approveOnServer(): Promise<void> {
  if (!confirm('현재 계획을 승인하고 스냅샷으로 보존할까요?')) return;
  try {
    await uploadDraft();
    const result = await apiRequest<{ file: string }>('/api/approve', 'POST');
    notify('승인 스냅샷 저장: ' + result.file);
  } catch (err) {
    notify('승인 실패: ' + errorMessage(err));
  }
}

/** Replaces the graph with the directory structure and imports the server finds in the repository. */
export async function analyzeRepository(): Promise<void> {
  if (
    !confirm(
      'Mac Studio 저장소의 실제 디렉터리 구조를 읽어 그래프를 교체하시겠습니까? 현재 로컬 수정 내용은 덮어씁니다.',
    )
  )
    return;
  try {
    const result = await apiRequest<{ plan: unknown }>('/api/repository/analyze', 'POST');
    replacePlan(normalizeImported(result.plan));
    notify('실제 FSD 구조를 불러왔습니다. I/O는 아직 추론하지 않았습니다.');
  } catch (err) {
    notify('저장소 분석 실패: ' + errorMessage(err));
  }
}
