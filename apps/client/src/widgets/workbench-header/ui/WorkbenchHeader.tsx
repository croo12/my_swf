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
import { Menu } from '@/shared/ui/menu';

/** Single-row header: title, graph editing tools, layer shortcuts, server status, and menus. */
export function WorkbenchHeader({ onJumpToLayer }: { onJumpToLayer: (layer: Layer) => void }) {
  const { plan, history } = useStore(planStore);
  const { connectFromId } = useStore(selectionStore);
  const edgeType = useStore(edgeTypeStore);
  const server = useStore(serverStatusStore);
  const fileRef = useRef<HTMLInputElement>(null);

  return (
    <header className="topbar">
      <div className="brand">
        <div className="logo">⌘</div>
        <div>
          <h1>Planning Workbench</h1>
          <div className="brand-meta">
            Draft v{plan.version} · {plan.nodes.length} nodes · {plan.edges.length} relations
          </div>
        </div>
      </div>

      <div className="toolgroup" aria-label="그래프 편집">
        <button className="btn" title="마지막 변경 되돌리기" disabled={history.length === 0} onClick={undo}>
          ↶ 되돌리기
        </button>
        <button className="btn" onClick={addNode}>
          ＋ 노드
        </button>
        <div className="connect-tool">
          <button
            className={connectFromId ? 'btn active' : 'btn'}
            title="선택한 노드에서 다른 노드로 관계를 연결"
            onClick={toggleConnectFromSelected}
          >
            {connectFromId ? '연결 취소' : '⤳ 연결'}
          </button>
          <select
            className="btn"
            aria-label="연결 종류"
            title="연결 종류"
            value={edgeType}
            onChange={(ev) => edgeTypeStore.set(ev.target.value)}
          >
            {EDGE_TYPES.map((type) => (
              <option key={type} value={type}>
                {type}
              </option>
            ))}
          </select>
        </div>
        <button className="btn" title="FSD 구조에 따라 노드를 다시 배치" onClick={autoLayout}>
          ▦ 정렬
        </button>
      </div>

      <div className="layer-nav" aria-label="FSD 레이어로 이동">
        {LAYERS.map((layer) => (
          <button key={layer} type="button" className="nav-layer" onClick={() => onJumpToLayer(layer)}>
            {layer}
          </button>
        ))}
      </div>

      <div className="toolgroup">
        <button
          className="server-chip"
          style={{ color: server.color }}
          title="서버 연결 설정"
          onClick={openConnectionDialog}
        >
          {server.text}
        </button>
        <Menu
          label="서버"
          items={[
            { label: '연결 설정', hint: '서버 주소와 API 토큰', onSelect: openConnectionDialog },
            { label: '저장소 분석', hint: '실제 폴더 구조로 그래프 교체', onSelect: analyzeRepository },
            { label: '서버에서 불러오기', hint: '서버 저장본으로 그래프 교체', onSelect: loadFromServer },
            { label: '서버에 저장', hint: '현재 그래프를 임시 저장', onSelect: saveToServer },
            { label: '계획 승인', hint: '저장 후 승인 스냅샷으로 보존', onSelect: approveOnServer },
          ]}
        />
        <Menu
          label="파일"
          items={[
            { label: 'JSON 불러오기', onSelect: () => fileRef.current?.click() },
            { label: 'JSON 내보내기', onSelect: exportPlan },
            { label: '샘플로 초기화', danger: true, onSelect: resetToSample },
          ]}
        />
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
      </div>
    </header>
  );
}
