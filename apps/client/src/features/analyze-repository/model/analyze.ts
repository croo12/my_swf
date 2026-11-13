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
    const result = await apiRequest<{
      plan: { analysis?: ScanInfo };
      git?: { branch: string; commit: string } | null;
      pullError?: string | null;
    }>('/api/repository/analyze', 'POST');
    const scan = normalizeImported(result.plan);
    const repository = buildRepository(scan.nodes, scan.edges, result.plan.analysis ?? {});
    repositoryStore.set(repository);
    const based = result.git ? ` · ${result.git.branch} @ ${result.git.commit}` : '';
    // The server pulls first; if that failed, the analysis is of the older checkout.
    notify(
      result.pullError
        ? `최신으로 갱신하지 못해 이전 상태를 분석했습니다${based}: ${result.pullError}`
        : `저장소를 분석했습니다${based}. 슬라이스 ${repository.slices.length}개`,
    );
    return true;
  } catch (err) {
    notify('저장소 분석 실패: ' + errorMessage(err));
    return false;
  } finally {
    analyzingStore.set(false);
  }
}
