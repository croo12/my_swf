import { useState } from 'react';
import { clearSelection, planStore, selectionStore } from '@/entities/plan';
import { JobBar, startJob } from '@/features/plan-job';
import { recordRequest, removeRequest } from '@/features/record-request';
import { useStore } from '@/shared/lib';

/**
 * Bottom area for design requests. A request is addressed either to the selected node or to the
 * whole graph; the target is always shown, and the list below shows the requests of that target.
 */
export function CommandBar() {
  const { plan } = useStore(planStore);
  const { selectedId } = useStore(selectionStore);
  const [command, setCommand] = useState('');

  const target = plan.nodes.find((n) => n.id === selectedId) ?? null;
  const targetId = target?.id ?? null;
  const requests = plan.requests.filter((r) => r.nodeId === targetId).toReversed();
  const nodeRequestCount = plan.requests.filter((r) => r.nodeId !== null).length;
  const graphRequestCount = plan.requests.length - nodeRequestCount;

  const record = () => {
    if (recordRequest(command, targetId)) setCommand('');
  };
  const requestPlan = async () => {
    if (await startJob(command, target?.path ?? null)) setCommand('');
  };

  return (
    <section className="commandarea">
      <div className="command-inner">
        <div className={target ? 'request-target is-node' : 'request-target'}>
          <span className="request-target-kind">{target ? '노드' : '전체 그래프'}</span>
          {target && (
            <>
              <span className="request-target-path">{target.path}</span>
              <button
                title="전체 그래프를 대상으로 전환"
                aria-label="전체 그래프를 대상으로 전환"
                onClick={clearSelection}
              >
                ×
              </button>
            </>
          )}
        </div>
        <textarea
          rows={1}
          placeholder={
            target
              ? `${target.path} 노드에 대한 요청 (예: 상태를 entities로 옮겨줘)`
              : '그래프 전체에 대한 요청 (예: 구매 흐름을 feature로 분리해줘) · 노드를 선택하면 그 노드에 대한 요청이 됩니다'
          }
          value={command}
          onChange={(ev) => setCommand(ev.target.value)}
          onKeyDown={(ev) => {
            if (ev.key === 'Enter' && (ev.metaKey || ev.ctrlKey)) record();
          }}
        />
        <button className="btn" title="요청을 메모로 남깁니다 (Ctrl+Enter)" onClick={record}>
          메모로 기록
        </button>
        <button className="btn primary" title="Codex에 이 요청으로 변경안을 만들게 합니다" onClick={requestPlan}>
          ✦ Herdr 계획 요청
        </button>
      </div>
      <div className="requests">
        <span className="requests-summary">
          기록된 요청 · 전체 그래프 {graphRequestCount} · 노드 {nodeRequestCount}
        </span>
        {requests.map((r) => (
          <div key={r.id} className="request-pill" title={r.text}>
            <span>{r.text}</span>
            <button title="요청 삭제" onClick={() => removeRequest(r.id)}>
              ×
            </button>
          </div>
        ))}
      </div>
      <JobBar />
    </section>
  );
}
