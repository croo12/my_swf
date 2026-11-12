// Plan JSON files: download the current plan, or load one from disk.
import { SCHEMA_VERSION, clearSelection, layoutAll, mutatePlan, normalizeImported, planStore } from '@/entities/plan';
import { errorMessage } from '@/shared/api';
import { notify } from '@/shared/ui/toast';

export function exportPlan(): void {
  const payload = {
    schemaVersion: SCHEMA_VERSION,
    exportedAt: new Date().toISOString(),
    planningState: 'draft',
    ...planStore.get().plan,
  };
  const url = URL.createObjectURL(new Blob([JSON.stringify(payload, null, 2)], { type: 'application/json' }));
  const link = document.createElement('a');
  link.href = url;
  link.download = 'plan-workbench.json';
  document.body.appendChild(link);
  link.click();
  link.remove();
  setTimeout(() => URL.revokeObjectURL(url), 500);
  notify('Plan JSON을 내보냈습니다.');
}

/** Replaces the graph with the plan in the file. The import can be undone. */
export async function importPlanFile(file: File): Promise<void> {
  try {
    const parsed = layoutAll(normalizeImported(JSON.parse(await file.text())));
    mutatePlan('설계를 불러왔습니다.', () => parsed);
    clearSelection();
  } catch (err) {
    notify('JSON 불러오기 실패: ' + errorMessage(err));
  }
}
