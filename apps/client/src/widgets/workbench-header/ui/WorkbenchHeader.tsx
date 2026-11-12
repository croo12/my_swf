import { useRef } from 'react';
import { EDGE_TYPES, LAYERS, planStore, selectionStore, type Layer } from '@/entities/plan';
import { openConnectionDialog, serverStatusStore } from '@/features/connect-server';
import {
  addNode,
  autoLayout,
  edgeTypeStore,
  resetToSample,
  toggleConnectFromSelected,
  undo,
} from '@/features/edit-graph';
import { exportPlan, importPlanFile } from '@/features/plan-transfer';
import { analyzeRepository, approveOnServer, loadFromServer, saveToServer } from '@/features/server-sync';
import { useStore } from '@/shared/lib';

/** Top bar with the toolbar, plus the status row underneath it. */
export function WorkbenchHeader({ onJumpToLayer }: { onJumpToLayer: (layer: Layer) => void }) {
  const { plan, history } = useStore(planStore);
  const { connectFromId } = useStore(selectionStore);
  const edgeType = useStore(edgeTypeStore);
  const server = useStore(serverStatusStore);
  const fileRef = useRef<HTMLInputElement>(null);

  return (
    <>
      <header className="topbar">
        <div className="brand">
          <div className="logo">⌘</div>
          <div>
            <div className="eyebrow">SOFTWARE FACTORY / PLANNING</div>
            <h1>
              Planning Workbench <span>· Layer / Slice / Segment</span>
            </h1>
          </div>
        </div>
        <div className="toolbar">
          <button className="btn" title="마지막 변경 되돌리기" disabled={history.length === 0} onClick={undo}>
            ↶ 되돌리기
          </button>
          <button className="btn" onClick={addNode}>
            ＋ 노드 추가
          </button>
          <select
            className="btn"
            aria-label="연결 종류"
            value={edgeType}
            onChange={(ev) => edgeTypeStore.set(ev.target.value)}
          >
            {EDGE_TYPES.map((type) => (
              <option key={type} value={type}>
                {type}
              </option>
            ))}
          </select>
          <button className={connectFromId ? 'btn active' : 'btn'} onClick={toggleConnectFromSelected}>
            {connectFromId ? '연결 취소' : '⤳ 연결'}
          </button>
          <button className="btn" title="FSD 구조에 따라 노드 배치" onClick={autoLayout}>
            ▦ 구조 정렬
          </button>
          <button className="btn" onClick={openConnectionDialog}>
            ⚙ 서버 연결
          </button>
          <button className="btn" onClick={analyzeRepository}>
            ⌕ 저장소 분석
          </button>
          <button className="btn" title="Mac Studio 저장본 불러오기" onClick={loadFromServer}>
            서버 불러오기
          </button>
          <button className="btn" title="Mac Studio에 임시 계획 저장" onClick={saveToServer}>
            서버 저장
          </button>
          <button className="btn" title="현재 저장본을 승인된 스냅샷으로 보존" onClick={approveOnServer}>
            계획 승인
          </button>
          <button className="btn" onClick={() => fileRef.current?.click()}>
            JSON 불러오기 ↑
          </button>
          <input
            ref={fileRef}
            type="file"
            accept=".json,application/json"
            hidden
            onChange={(ev) => {
              const input = ev.target;
              const file = input.files?.[0];
              if (file)
                void importPlanFile(file).finally(() => {
                  input.value = '';
                });
            }}
          />
          <button className="btn primary" onClick={exportPlan}>
            Plan JSON ↓
          </button>
        </div>
      </header>

      <div className="statusrow">
        <div className="leftmeta">
          <span className="tinycircle" />
          <span className="meta">
            <strong>Draft</strong> · v{plan.version}
          </span>
          <div className="sep" />
          <span className="meta">FSD Architecture · Structured View</span>
          <span className="sep" />
          <span className="meta" style={{ color: server.color }}>
            {server.text}
          </span>
          <div className="sep" />
          <span className="meta">
            {plan.nodes.length} nodes · {plan.edges.length} relations
          </span>
        </div>
        <div className="rightmeta">
          <div className="layer-nav" aria-label="FSD 레이어로 이동">
            {LAYERS.map((layer) => (
              <button key={layer} type="button" className="nav-layer" onClick={() => onJumpToLayer(layer)}>
                {layer}
              </button>
            ))}
          </div>
          <button className="btn quiet tiny" title="샘플로 초기화" onClick={resetToSample}>
            초기화
          </button>
        </div>
      </div>
    </>
  );
}
