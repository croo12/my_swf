import { useEffect, useRef, useState, useSyncExternalStore, type ChangeEvent } from 'react';
import { apiRequest, checkConnection, errorMessage, getServerStatus, isAllowedServerUrl, saveConnection, subscribeServerStatus } from './api/factory';
import { Canvas } from './components/Canvas';
import { ConnectionModal } from './components/ConnectionModal';
import { EditPanel, type NodeDraft } from './components/EditPanel';
import {
  CONTENT_LEFT, EDGE_TYPES, LAYERS, MAX_HISTORY, SAMPLE, laneY, layerOf, layoutAll, loadPlan, normalizeImported, sliceOf, storePlan,
  type Plan,
} from './model/plan';

interface Doc { plan: Plan; history: Plan[] }
interface JobView { message: string; proposal: Plan | null; baseVersion: number }
interface JobStatus { job: { status: string; message: string }; proposedPlan: unknown }

const pushHistory = (history: Plan[], plan: Plan) => [...history, plan].slice(-MAX_HISTORY);

export function App() {
  const [doc, setDoc] = useState<Doc>(() => ({ plan: loadPlan(), history: [] }));
  const [selected, setSelected] = useState<string | null>(null);
  const [editOpen, setEditOpen] = useState(false);
  const [editSession, setEditSession] = useState(0);
  const [connectFrom, setConnectFrom] = useState<string | null>(null);
  const [edgeType, setEdgeType] = useState<string>(EDGE_TYPES[0]);
  const [command, setCommand] = useState('');
  const [toast, setToast] = useState({ message: '', visible: false });
  const [connectionOpen, setConnectionOpen] = useState(false);
  const server = useSyncExternalStore(subscribeServerStatus, getServerStatus);
  const [job, setJob] = useState<JobView | null>(null);

  const toastTimer = useRef<ReturnType<typeof setTimeout>>(undefined);
  const jobTimer = useRef<ReturnType<typeof setTimeout>>(undefined);
  const scrollRef = useRef<HTMLDivElement>(null);
  const fileRef = useRef<HTMLInputElement>(null);

  const { plan, history } = doc;
  const selectedNode = plan.nodes.find((n) => n.id === selected);

  useEffect(() => storePlan(plan), [plan]);

  const notify = (message: string) => {
    setToast({ message, visible: true });
    clearTimeout(toastTimer.current);
    toastTimer.current = setTimeout(() => setToast((t) => ({ ...t, visible: false })), 2200);
  };

  /** Applies one undoable change: records history, bumps the plan version, shows a toast. */
  const mutate = (label: string, change: (plan: Plan) => Plan) => {
    setDoc((d) => {
      const next = change(d.plan);
      return { plan: { ...next, version: next.version + 1 }, history: pushHistory(d.history, d.plan) };
    });
    notify(label);
  };

  /** Replaces the whole plan from the server; this is not undoable. */
  const replacePlan = (next: Plan) => {
    setDoc({ plan: layoutAll(next), history: [] });
    setSelected(null);
    setEditOpen(false);
    setConnectFrom(null);
  };

  const openEdit = (id: string) => {
    setSelected(id);
    setEditOpen(true);
    setEditSession((n) => n + 1);
  };

  // ── Graph editing ────────────────────────────────────────────────────────
  const clickNode = (id: string) => {
    if (!connectFrom) { openEdit(id); return; }
    const from = connectFrom;
    setConnectFrom(null);
    if (from === id) return;
    if (plan.edges.some((e) => e.from === from && e.to === id && e.type === edgeType)) { notify('이미 존재하는 연결입니다.'); return; }
    const edge = { id: 'edge-' + Date.now(), from, to: id, type: edgeType };
    mutate(`${edgeType} 관계를 추가했습니다.`, (p) => ({ ...p, edges: [...p.edges, edge] }));
  };

  const toggleConnect = (id: string) => {
    setSelected(id);
    setEditOpen(false);
    setConnectFrom(connectFrom === id ? null : id);
  };

  const connectFromSelected = () => {
    if (connectFrom) { setConnectFrom(null); return; }
    if (!selected) { notify('연결할 출발 노드를 먼저 선택하세요.'); return; }
    setConnectFrom(selected);
    setEditOpen(false);
  };

  const moveNode = (id: string, x: number, y: number) => {
    setDoc((d) => ({
      plan: { ...d.plan, version: d.plan.version + 1, nodes: d.plan.nodes.map((n) => (n.id === id ? { ...n, x, y } : n)) },
      history: pushHistory(d.history, d.plan),
    }));
  };

  const addNode = () => {
    const id = 'module-' + Date.now();
    const node = { id, path: 'features/new-feature/model', description: '새 모듈의 책임을 적어주세요.', inputs: [], outputs: [], x: CONTENT_LEFT + 20, y: laneY('features') + 62 };
    mutate('새 모듈이 추가되었습니다.', (p) => layoutAll({ ...p, nodes: [...p.nodes, node] }));
    openEdit(id);
  };

  const saveNode = (draft: NodeDraft) => {
    if (!selectedNode) return;
    const id = selectedNode.id;
    const moved = { ...selectedNode, ...draft };
    // Re-run the layout only when the node moved to another layer or slice.
    const changedGroup = layerOf(selectedNode) !== layerOf(moved) || sliceOf(selectedNode) !== sliceOf(moved);
    mutate('노드 설계 변경이 반영되었습니다.', (p) => {
      const next = { ...p, nodes: p.nodes.map((n) => (n.id === id ? { ...n, ...draft } : n)) };
      return changedGroup ? layoutAll(next) : next;
    });
    setEditOpen(false);
  };

  const deleteNode = () => {
    if (!selectedNode || !confirm(`${selectedNode.path} 노드와 연결을 삭제할까요?`)) return;
    const id = selectedNode.id;
    mutate('노드가 삭제되었습니다.', (p) => ({
      ...p,
      nodes: p.nodes.filter((n) => n.id !== id),
      edges: p.edges.filter((e) => e.from !== id && e.to !== id),
      requests: p.requests.filter((r) => r.nodeId !== id),
    }));
    setSelected(null);
    setEditOpen(false);
  };

  const removeEdge = (edgeId: string) => mutate('연결을 삭제했습니다.', (p) => ({ ...p, edges: p.edges.filter((e) => e.id !== edgeId) }));

  const undo = () => {
    const previous = history.at(-1);
    if (!previous) return;
    setDoc({ plan: previous, history: history.slice(0, -1) });
    if (!previous.nodes.some((n) => n.id === selected)) setSelected(null);
    setEditOpen(false);
    setConnectFrom(null);
    notify('이전 상태로 되돌렸습니다.');
  };

  const reset = () => {
    if (!confirm('현재 설계를 지우고 샘플 상태로 초기화할까요?')) return;
    mutate('샘플 그래프로 초기화했습니다.', () => structuredClone(SAMPLE));
    setSelected(null);
    setEditOpen(false);
    setConnectFrom(null);
  };

  // ── Requests ─────────────────────────────────────────────────────────────
  const recordRequest = () => {
    const text = command.trim();
    if (!text) { notify('수정 요청을 입력하세요.'); return; }
    const request = { id: 'r-' + Date.now(), nodeId: selected, text, status: 'pending' };
    mutate('수정 요청을 기록했습니다. AI 실행은 아직 연결되지 않았습니다.', (p) => ({ ...p, requests: [...p.requests, request] }));
    setCommand('');
  };

  const removeRequest = (id: string) => mutate('수정 요청을 제거했습니다.', (p) => ({ ...p, requests: p.requests.filter((r) => r.id !== id) }));

  // ── JSON import / export ─────────────────────────────────────────────────
  const exportPlan = () => {
    const payload = { schemaVersion: '0.3', exportedAt: new Date().toISOString(), planningState: 'draft', ...plan };
    const url = URL.createObjectURL(new Blob([JSON.stringify(payload, null, 2)], { type: 'application/json' }));
    const link = document.createElement('a');
    link.href = url;
    link.download = 'plan-workbench.json';
    document.body.appendChild(link);
    link.click();
    link.remove();
    setTimeout(() => URL.revokeObjectURL(url), 500);
    notify('Plan JSON을 내보냈습니다.');
  };

  const importPlan = async (ev: ChangeEvent<HTMLInputElement>) => {
    const input = ev.target;
    const file = input.files?.[0];
    if (!file) return;
    try {
      const parsed = layoutAll(normalizeImported(JSON.parse(await file.text())));
      mutate('설계를 불러왔습니다.', () => parsed);
      setSelected(null);
      setEditOpen(false);
    } catch (err) {
      notify('JSON 불러오기 실패: ' + errorMessage(err));
    }
    input.value = '';
  };

  // ── Factory server ───────────────────────────────────────────────────────
  useEffect(() => () => {
    clearTimeout(toastTimer.current);
    clearTimeout(jobTimer.current);
  }, []);

  const saveConnectionSettings = async (url: string, token: string) => {
    if (!isAllowedServerUrl(url)) { notify('HTTPS 서버 주소를 입력하세요. HTTP는 localhost만 허용됩니다.'); return; }
    setConnectionOpen(false);
    try {
      await saveConnection(url, token);
    } catch (err) {
      notify('토큰을 보안 저장소에 저장하지 못했습니다: ' + errorMessage(err));
      return;
    }
    notify((await checkConnection()) ? '서버와 연결되었습니다.' : '연결 확인 실패: 서버 주소, 토큰, Tailscale 확인');
  };

  const syncPlan = () => apiRequest('/api/plan', 'PUT', { schemaVersion: '0.3', planningState: 'draft', ...plan });

  const saveToServer = async () => {
    try { await syncPlan(); notify('Factory 서버에 Plan을 저장했습니다.'); }
    catch (err) { notify('저장 실패: ' + errorMessage(err)); }
  };

  const loadFromServer = async () => {
    if (!confirm('서버 저장본으로 현재 그래프를 교체하시겠습니까?')) return;
    try {
      const data = await apiRequest<{ plan: unknown }>('/api/plan');
      replacePlan(normalizeImported(data.plan));
      notify('서버 Plan을 불러왔습니다.');
    } catch (err) { notify('불러오기 실패: ' + errorMessage(err)); }
  };

  const approveOnServer = async () => {
    if (!confirm('현재 계획을 승인하고 스냅샷으로 보존할까요?')) return;
    try {
      await syncPlan();
      const result = await apiRequest<{ file: string }>('/api/approve', 'POST');
      notify('승인 스냅샷 저장: ' + result.file);
    } catch (err) { notify('승인 실패: ' + errorMessage(err)); }
  };

  const analyzeRepository = async () => {
    if (!confirm('Mac Studio 저장소의 실제 디렉터리 구조를 읽어 그래프를 교체하시겠습니까? 현재 로컬 수정 내용은 덮어씁니다.')) return;
    try {
      const result = await apiRequest<{ plan: unknown }>('/api/repository/analyze', 'POST');
      replacePlan(normalizeImported(result.plan));
      notify('실제 FSD 구조를 불러왔습니다. I/O는 아직 추론하지 않았습니다.');
    } catch (err) { notify('저장소 분석 실패: ' + errorMessage(err)); }
  };

  // ── Herdr planning job ───────────────────────────────────────────────────
  const pollJob = async (id: string, baseVersion: number) => {
    try {
      const { job: remote, proposedPlan } = await apiRequest<JobStatus>('/api/jobs/' + id);
      const message = 'Herdr · ' + remote.status + ' · ' + remote.message;
      if (remote.status === 'completed') {
        setJob({ message, proposal: normalizeImported(proposedPlan), baseVersion });
        notify('AI 설계 제안이 준비되었습니다. 검토 후 적용하세요.');
        return;
      }
      setJob({ message, proposal: null, baseVersion });
      if (remote.status === 'queued' || remote.status === 'running') {
        jobTimer.current = setTimeout(() => void pollJob(id, baseVersion), 3000);
      }
    } catch (err) {
      setJob({ message: '작업 상태 조회 실패: ' + errorMessage(err), proposal: null, baseVersion });
    }
  };

  const startJob = async () => {
    const instruction = command.trim();
    if (!instruction) { notify('Herdr에게 전달할 설계 요청을 입력하세요.'); return; }
    if (!confirm('Mac Studio의 Codex에 현재 계획을 전달해 변경안을 생성할까요?')) return;
    clearTimeout(jobTimer.current);
    const baseVersion = plan.version;
    try {
      const response = await apiRequest<{ id: string }>('/api/jobs', 'POST', { instruction, currentPlan: { schemaVersion: '0.3', ...plan } });
      setJob({ message: 'Herdr 작업 시작: ' + response.id.slice(0, 8), proposal: null, baseVersion });
      setCommand('');
      await pollJob(response.id, baseVersion);
    } catch (err) {
      setJob({ message: 'AI 작업 시작 실패: ' + errorMessage(err), proposal: null, baseVersion });
    }
  };

  const applyProposal = () => {
    const proposal = job?.proposal;
    if (!job || !proposal) return;
    if (plan.version !== job.baseVersion && !confirm('AI 작업 도중 현재 계획이 수정되었습니다. AI 제안으로 덮어쓰시겠습니까?')) return;
    if (!confirm('AI가 제안한 전체 그래프로 교체합니다. 적용 후 되돌리기가 가능합니다. 계속할까요?')) return;
    mutate('AI 제안을 현재 계획에 적용했습니다.', () => layoutAll(proposal));
    setJob(null);
  };

  return (
    <>
      <div className="app">
        <header className="topbar">
          <div className="brand">
            <div className="logo">⌘</div>
            <div>
              <div className="eyebrow">SOFTWARE FACTORY / PLANNING</div>
              <h1>Planning Workbench <span>· Layer / Slice / Segment</span></h1>
            </div>
          </div>
          <div className="toolbar">
            <button className="btn" title="마지막 변경 되돌리기" disabled={history.length === 0} onClick={undo}>↶ 되돌리기</button>
            <button className="btn" onClick={addNode}>＋ 노드 추가</button>
            <select className="btn" aria-label="연결 종류" value={edgeType} onChange={(ev) => setEdgeType(ev.target.value)}>
              {EDGE_TYPES.map((type) => <option key={type} value={type}>{type}</option>)}
            </select>
            <button className={connectFrom ? 'btn active' : 'btn'} onClick={connectFromSelected}>{connectFrom ? '연결 취소' : '⤳ 연결'}</button>
            <button className="btn" title="FSD 구조에 따라 노드 배치" onClick={() => mutate('Layer → Slice → Segment 기준으로 정렬했습니다.', layoutAll)}>▦ 구조 정렬</button>
            <button className="btn" onClick={() => setConnectionOpen(true)}>⚙ 서버 연결</button>
            <button className="btn" onClick={analyzeRepository}>⌕ 저장소 분석</button>
            <button className="btn" title="Mac Studio 저장본 불러오기" onClick={loadFromServer}>서버 불러오기</button>
            <button className="btn" title="Mac Studio에 임시 계획 저장" onClick={saveToServer}>서버 저장</button>
            <button className="btn" title="현재 저장본을 승인된 스냅샷으로 보존" onClick={approveOnServer}>계획 승인</button>
            <button className="btn" onClick={() => fileRef.current?.click()}>JSON 불러오기 ↑</button>
            <input ref={fileRef} type="file" accept=".json,application/json" hidden onChange={importPlan} />
            <button className="btn primary" onClick={exportPlan}>Plan JSON ↓</button>
          </div>
        </header>

        <div className="statusrow">
          <div className="leftmeta">
            <span className="tinycircle" />
            <span className="meta"><strong>Draft</strong> · v{plan.version}</span>
            <div className="sep" />
            <span className="meta">FSD Architecture · Structured View</span>
            <span className="sep" />
            <span className="meta" style={{ color: server.color }}>{server.text}</span>
            <div className="sep" />
            <span className="meta">{plan.nodes.length} nodes · {plan.edges.length} relations</span>
          </div>
          <div className="rightmeta">
            <div className="layer-nav" aria-label="FSD 레이어로 이동">
              {LAYERS.map((layer) => (
                <button key={layer} type="button" className="nav-layer" onClick={() => scrollRef.current?.scrollTo({ top: laneY(layer) - 18, behavior: 'smooth' })}>
                  {layer}
                </button>
              ))}
            </div>
            <button className="btn quiet tiny" title="샘플로 초기화" onClick={reset}>초기화</button>
          </div>
        </div>

        <main className="stage">
          <Canvas
            plan={plan}
            selected={selected}
            connectFrom={connectFrom}
            scrollRef={scrollRef}
            onNodeClick={clickNode}
            onEdit={openEdit}
            onToggleConnect={toggleConnect}
            onMove={moveNode}
          />
          {editOpen && selectedNode && (
            <EditPanel
              key={editSession}
              node={selectedNode}
              nodes={plan.nodes}
              edges={plan.edges}
              onSave={saveNode}
              onDelete={deleteNode}
              onRemoveEdge={removeEdge}
              onClose={() => setEditOpen(false)}
            />
          )}
          <div className={toast.visible ? 'toast show' : 'toast'} role="status">{toast.message}</div>
        </main>

        <section className="commandarea">
          <div className="command-top">
            <div className="command-title">✳ 설계 수정 요청 <span className="minor-note">(선택 노드에 연결)</span></div>
            <div className="command-hint">기록은 로컬에 보관 · Herdr 요청은 서버에서 실행</div>
          </div>
          <div className="command-inner">
            <textarea
              rows={1}
              placeholder="예: book-selection의 상태를 entities/book으로 옮기고, 의존성을 수정해줘"
              value={command}
              onChange={(ev) => setCommand(ev.target.value)}
              onKeyDown={(ev) => { if (ev.key === 'Enter' && (ev.metaKey || ev.ctrlKey)) recordRequest(); }}
            />
            <button className="btn" onClick={recordRequest}>요청 기록</button>
            <button className="btn primary" onClick={startJob}>✦ Herdr 계획 요청</button>
          </div>
          <div className="requests">
            {plan.requests.slice(-5).reverse().map((r) => (
              <div key={r.id} className="request-pill" title={r.text}>
                <span>{plan.nodes.find((n) => n.id === r.nodeId)?.path ?? '전체'} · {r.text}</span>
                <button title="요청 삭제" onClick={() => removeRequest(r.id)}>×</button>
              </div>
            ))}
          </div>
          {job && (
            <div className="jobbar">
              <span>{job.message}</span>
              {job.proposal && <button className="btn primary tiny" onClick={applyProposal}>제안 그래프 적용</button>}
            </div>
          )}
        </section>
      </div>
      {connectionOpen && <ConnectionModal onSave={saveConnectionSettings} onCancel={() => setConnectionOpen(false)} />}
    </>
  );
}
