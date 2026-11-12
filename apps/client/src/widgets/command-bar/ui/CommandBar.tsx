import { useState } from 'react';
import { planStore } from '@/entities/plan';
import { JobBar, startJob } from '@/features/plan-job';
import { recordRequest, removeRequest } from '@/features/record-request';
import { useStore } from '@/shared/lib';

const RECENT_REQUESTS = 5;

/** Bottom area: the design-request input, recent requests, and the planning job status. */
export function CommandBar() {
  const { plan } = useStore(planStore);
  const [command, setCommand] = useState('');

  const record = () => {
    if (recordRequest(command)) setCommand('');
  };
  const requestPlan = async () => {
    if (await startJob(command)) setCommand('');
  };

  return (
    <section className="commandarea">
      <div className="command-top">
        <div className="command-title">
          ✳ 설계 수정 요청 <span className="minor-note">(선택 노드에 연결)</span>
        </div>
        <div className="command-hint">기록은 로컬에 보관 · Herdr 요청은 서버에서 실행</div>
      </div>
      <div className="command-inner">
        <textarea
          rows={1}
          placeholder="예: book-selection의 상태를 entities/book으로 옮기고, 의존성을 수정해줘"
          value={command}
          onChange={(ev) => setCommand(ev.target.value)}
          onKeyDown={(ev) => {
            if (ev.key === 'Enter' && (ev.metaKey || ev.ctrlKey)) record();
          }}
        />
        <button className="btn" onClick={record}>
          요청 기록
        </button>
        <button className="btn primary" onClick={requestPlan}>
          ✦ Herdr 계획 요청
        </button>
      </div>
      <div className="requests">
        {plan.requests
          .slice(-RECENT_REQUESTS)
          .toReversed()
          .map((r) => (
            <div key={r.id} className="request-pill" title={r.text}>
              <span>
                {plan.nodes.find((n) => n.id === r.nodeId)?.path ?? '전체'} · {r.text}
              </span>
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
