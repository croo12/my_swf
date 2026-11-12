// A Herdr/Codex planning job: start it, poll it, and apply the proposed graph once the user accepts.
import { SCHEMA_VERSION, layoutAll, mutatePlan, normalizeImported, planStore, type Plan } from '@/entities/plan';
import { apiRequest, errorMessage } from '@/shared/api';
import { createStore } from '@/shared/lib';
import { notify } from '@/shared/ui/toast';

const POLL_MS = 3000;

export interface JobView {
  message: string;
  /** Set once the job completed with a valid plan. Nothing is applied until the user accepts it. */
  proposal: Plan | null;
  /** Plan version when the job started, to detect edits made while it ran. */
  baseVersion: number;
}

interface JobStatus {
  job: { status: string; message: string };
  proposedPlan: unknown;
}

export const jobStore = createStore<JobView | null>(null);
let pollTimer: ReturnType<typeof setTimeout> | undefined;

async function pollJob(id: string, baseVersion: number): Promise<void> {
  try {
    const { job, proposedPlan } = await apiRequest<JobStatus>('/api/jobs/' + id);
    const message = 'Herdr · ' + job.status + ' · ' + job.message;
    if (job.status === 'completed') {
      jobStore.set({ message, proposal: normalizeImported(proposedPlan), baseVersion });
      notify('AI 설계 제안이 준비되었습니다. 검토 후 적용하세요.');
      return;
    }
    jobStore.set({ message, proposal: null, baseVersion });
    if (job.status === 'queued' || job.status === 'running') {
      pollTimer = setTimeout(() => void pollJob(id, baseVersion), POLL_MS);
    }
  } catch (err) {
    jobStore.set({ message: '작업 상태 조회 실패: ' + errorMessage(err), proposal: null, baseVersion });
  }
}

/**
 * Sends the instruction and the current plan to the server. When the request is about one node,
 * its path is put in front of the instruction. Returns true once the job was accepted.
 */
export async function startJob(rawInstruction: string, targetPath: string | null): Promise<boolean> {
  const text = rawInstruction.trim();
  const instruction = targetPath && text ? `[대상 노드: ${targetPath}] ${text}` : text;
  if (!instruction) {
    notify('Herdr에게 전달할 설계 요청을 입력하세요.');
    return false;
  }
  if (!confirm('Mac Studio의 Codex에 현재 계획을 전달해 변경안을 생성할까요?')) return false;
  clearTimeout(pollTimer);
  const { plan } = planStore.get();
  const baseVersion = plan.version;
  try {
    const response = await apiRequest<{ id: string }>('/api/jobs', 'POST', {
      instruction,
      currentPlan: { schemaVersion: SCHEMA_VERSION, ...plan },
    });
    jobStore.set({ message: 'Herdr 작업 시작: ' + response.id.slice(0, 8), proposal: null, baseVersion });
    void pollJob(response.id, baseVersion);
    return true;
  } catch (err) {
    jobStore.set({ message: 'AI 작업 시작 실패: ' + errorMessage(err), proposal: null, baseVersion });
    return false;
  }
}

/** Replaces the graph with the proposal after the user confirms. The change can be undone. */
export function applyProposal(): void {
  const job = jobStore.get();
  const proposal = job?.proposal;
  if (!job || !proposal) return;
  if (
    planStore.get().plan.version !== job.baseVersion &&
    !confirm('AI 작업 도중 현재 계획이 수정되었습니다. AI 제안으로 덮어쓰시겠습니까?')
  )
    return;
  if (!confirm('AI가 제안한 전체 그래프로 교체합니다. 적용 후 되돌리기가 가능합니다. 계속할까요?')) return;
  mutatePlan('AI 제안을 현재 계획에 적용했습니다.', () => layoutAll(proposal));
  jobStore.set(null);
}
