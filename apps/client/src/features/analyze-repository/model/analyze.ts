// Asks the server to scan the repository and stores the result as the read-only overview.
import { normalizeImported } from '@/entities/plan';
import { buildRepository, repositoryStore, type ScanInfo } from '@/entities/repository';
import { apiRequest, errorMessage } from '@/shared/api';
import { createStore } from '@/shared/lib';
import { notify } from '@/shared/ui/toast';

export const analyzingStore = createStore(false);

/** Scans the repository. The plan is not touched. Returns whether the scan succeeded. */
export async function analyzeRepository(): Promise<boolean> {
  if (analyzingStore.get()) return false;
  analyzingStore.set(true);
  try {
    const result = await apiRequest<{ plan: { analysis?: ScanInfo } }>('/api/repository/analyze', 'POST');
    const scan = normalizeImported(result.plan);
    const repository = buildRepository(scan.nodes, scan.edges, result.plan.analysis ?? {});
    repositoryStore.set(repository);
    notify(`저장소를 분석했습니다. 슬라이스 ${repository.slices.length}개`);
    return true;
  } catch (err) {
    notify('저장소 분석 실패: ' + errorMessage(err));
    return false;
  } finally {
    analyzingStore.set(false);
  }
}
