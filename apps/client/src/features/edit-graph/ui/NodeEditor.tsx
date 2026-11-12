import { Fragment, useState } from 'react';
import {
  MODULE_KINDS,
  MODULE_KIND_LABELS,
  cleanPath,
  closeEditor,
  isValidPath,
  kindOf,
  moduleNameOf,
  planStore,
  selectionStore,
  type ModuleKind,
  type PlanEdge,
  type PlanNode,
} from '@/entities/plan';
import { useStore } from '@/shared/lib';
import { deleteNode, removeEdge, saveNode } from '../model/actions';

const METHODS = ['GET', 'POST', 'PUT', 'PATCH', 'DELETE'];

const lines = (text: string) =>
  text
    .split('\n')
    .map((s) => s.trim())
    .filter(Boolean);

/** Rows of the interface list while it is being edited; inputs and outputs are one entry per line. */
interface InterfaceRow {
  key: number;
  name: string;
  description: string;
  inputs: string;
  outputs: string;
}
interface CallRow {
  key: number;
  method: string;
  path: string;
  description: string;
}

/** Side panel for editing the selected module. Renders nothing while no editor is open. */
export function NodeEditor() {
  const { plan } = useStore(planStore);
  const { selectedId, editorOpen, editorSession } = useStore(selectionStore);
  const node = plan.nodes.find((n) => n.id === selectedId);
  if (!editorOpen || !node) return null;
  // The key restarts the form whenever the editor is opened again.
  return <NodeEditorForm key={editorSession} node={node} nodes={plan.nodes} edges={plan.edges} />;
}

function NodeEditorForm({ node, nodes, edges }: { node: PlanNode; nodes: PlanNode[]; edges: PlanEdge[] }) {
  const [name, setName] = useState(node.name ?? '');
  const [kind, setKind] = useState<ModuleKind>(kindOf(node));
  const [path, setPath] = useState(node.path);
  const [description, setDescription] = useState(node.description);
  const [behaviors, setBehaviors] = useState((node.behaviors ?? []).join('\n'));
  const [files, setFiles] = useState((node.files ?? []).join('\n'));
  const [interfaces, setInterfaces] = useState<InterfaceRow[]>(() =>
    node.interfaces.map((item, key) => ({
      key,
      name: item.name,
      description: item.description,
      inputs: item.inputs.join('\n'),
      outputs: item.outputs.join('\n'),
    })),
  );
  const [calls, setCalls] = useState<CallRow[]>(() => node.serverCalls.map((call, key) => ({ key, ...call })));
  const [nextKey, setNextKey] = useState(node.interfaces.length + node.serverCalls.length);
  const [invalid, setInvalid] = useState(false);

  const pathParts = cleanPath(path).split('/').filter(Boolean);
  const related = edges.filter((e) => e.from === node.id || e.to === node.id);
  const other = (e: PlanEdge) => nodes.find((n) => n.id === (e.from === node.id ? e.to : e.from));

  const patchInterface = (key: number, change: Partial<InterfaceRow>) =>
    setInterfaces((rows) => rows.map((row) => (row.key === key ? { ...row, ...change } : row)));
  const patchCall = (key: number, change: Partial<CallRow>) =>
    setCalls((rows) => rows.map((row) => (row.key === key ? { ...row, ...change } : row)));
  const addInterface = () => {
    setInterfaces((rows) => [...rows, { key: nextKey, name: '', description: '', inputs: '', outputs: '' }]);
    setNextKey(nextKey + 1);
  };
  const addCall = () => {
    setCalls((rows) => [...rows, { key: nextKey, method: 'GET', path: '/', description: '' }]);
    setNextKey(nextKey + 1);
  };

  const save = () => {
    const cleaned = cleanPath(path);
    if (!isValidPath(cleaned)) {
      setInvalid(true);
      return;
    }
    saveNode(node.id, {
      name: name.trim(),
      kind,
      path: cleaned,
      description: description.trim(),
      behaviors: lines(behaviors),
      files: lines(files),
      interfaces: interfaces
        .filter((row) => row.name.trim())
        .map((row) => ({
          name: row.name.trim(),
          description: row.description.trim(),
          inputs: lines(row.inputs),
          outputs: lines(row.outputs),
        })),
      serverCalls: calls
        .filter((row) => row.path.trim())
        .map((row) => ({ method: row.method, path: row.path.trim(), description: row.description.trim() })),
    });
  };

  return (
    <aside className="overlay open" aria-label="모듈 편집">
      <div className="overlay-head">
        <div>
          <div className="panel-tag">MODULE</div>
          <h2>{moduleNameOf(node)}</h2>
        </div>
        <button className="close" aria-label="닫기" onClick={closeEditor}>
          ×
        </button>
      </div>
      <div className="overlay-body">
        <div className="field">
          <label htmlFor="editName">모듈 이름</label>
          <input
            id="editName"
            placeholder="예: 계획 작업 취소"
            value={name}
            onChange={(ev) => setName(ev.target.value)}
          />
        </div>
        <div className="field">
          <label htmlFor="editKind">구분</label>
          <select id="editKind" value={kind} onChange={(ev) => setKind(ev.target.value as ModuleKind)}>
            {MODULE_KINDS.map((value) => (
              <option key={value} value={value}>
                {MODULE_KIND_LABELS[value]}
              </option>
            ))}
          </select>
          <div className="help">신규: 새로 만듦 · 수정: 있는 모듈을 고침 · 기존: 참고용으로만 표시</div>
        </div>
        <div className="field">
          <label htmlFor="editPath">배치 위치 (FSD 경로)</label>
          <input
            id="editPath"
            spellCheck={false}
            placeholder="features/my-feature/model"
            value={path}
            onChange={(ev) => setPath(ev.target.value)}
          />
          <div className="path-preview">
            {pathParts.length === 0 && <span className="help">경로를 입력해 주세요</span>}
            {pathParts.map((part, i) => (
              <Fragment key={i}>
                {i > 0 && <span className="path-divider">/</span>}
                <span className="path-part" data-depth={i}>
                  {part}
                </span>
              </Fragment>
            ))}
          </div>
          <div className="help">
            pages/widgets/features/entities는 layer / slice / segment, app/shared는 layer / segment입니다.
          </div>
        </div>
        <div className="field">
          <label htmlFor="editDesc">책임</label>
          <textarea id="editDesc" rows={2} value={description} onChange={(ev) => setDescription(ev.target.value)} />
        </div>
        <div className="field">
          <label htmlFor="editBehaviors">동작 · 한 줄에 하나씩</label>
          <textarea id="editBehaviors" rows={3} value={behaviors} onChange={(ev) => setBehaviors(ev.target.value)} />
        </div>

        <div className="field">
          <label>인터페이스 · 모듈 바깥으로 노출하는 것</label>
          {interfaces.length === 0 && <div className="help">노출하는 인터페이스 없음</div>}
          {interfaces.map((row) => (
            <div key={row.key} className="edit-card">
              <div className="edit-card-head">
                <input
                  aria-label="인터페이스 이름"
                  spellCheck={false}
                  placeholder="이름 (예: cancelJob, JobBar)"
                  value={row.name}
                  onChange={(ev) => patchInterface(row.key, { name: ev.target.value })}
                />
                <button
                  className="textbtn"
                  onClick={() => setInterfaces((rows) => rows.filter((r) => r.key !== row.key))}
                >
                  삭제
                </button>
              </div>
              <input
                aria-label="인터페이스 설명"
                placeholder="설명 (선택)"
                value={row.description}
                onChange={(ev) => patchInterface(row.key, { description: ev.target.value })}
              />
              <div className="edit-card-io">
                <textarea
                  aria-label="입력"
                  rows={2}
                  spellCheck={false}
                  placeholder="입력 · 한 줄에 하나 (없으면 비움)"
                  value={row.inputs}
                  onChange={(ev) => patchInterface(row.key, { inputs: ev.target.value })}
                />
                <textarea
                  aria-label="출력"
                  rows={2}
                  spellCheck={false}
                  placeholder="출력 · 한 줄에 하나 (없으면 비움)"
                  value={row.outputs}
                  onChange={(ev) => patchInterface(row.key, { outputs: ev.target.value })}
                />
              </div>
            </div>
          ))}
          <button className="btn tiny" onClick={addInterface}>
            ＋ 인터페이스
          </button>
        </div>

        <div className="field">
          <label>서버 호출 · 이 모듈에서 시작되는 요청</label>
          {calls.length === 0 && <div className="help">서버 호출 없음</div>}
          {calls.map((row) => (
            <div key={row.key} className="edit-card">
              <div className="edit-card-head">
                <select
                  aria-label="메서드"
                  value={row.method}
                  onChange={(ev) => patchCall(row.key, { method: ev.target.value })}
                >
                  {METHODS.map((method) => (
                    <option key={method} value={method}>
                      {method}
                    </option>
                  ))}
                </select>
                <input
                  aria-label="경로"
                  spellCheck={false}
                  placeholder="/api/..."
                  value={row.path}
                  onChange={(ev) => patchCall(row.key, { path: ev.target.value })}
                />
                <button className="textbtn" onClick={() => setCalls((rows) => rows.filter((r) => r.key !== row.key))}>
                  삭제
                </button>
              </div>
              <input
                aria-label="서버 호출 설명"
                placeholder="설명 (선택)"
                value={row.description}
                onChange={(ev) => patchCall(row.key, { description: ev.target.value })}
              />
            </div>
          ))}
          <button className="btn tiny" onClick={addCall}>
            ＋ 서버 호출
          </button>
          <div className="help">서버 호출이 있는 모듈은 shared/api 모듈을 거쳐야 합니다.</div>
        </div>

        <div className="field">
          <label htmlFor="editFiles">파일 · 한 줄에 하나씩</label>
          <textarea
            id="editFiles"
            rows={2}
            spellCheck={false}
            placeholder="cancel-job.ts"
            value={files}
            onChange={(ev) => setFiles(ev.target.value)}
          />
        </div>
        <div className="field">
          <label>연결 관계</label>
          {related.length === 0 && <div className="help">연결 없음</div>}
          {related.map((e) => {
            const target = other(e);
            return (
              <div key={e.id} className="edge-row">
                <span>
                  {e.from === node.id ? '→' : '←'} {e.type} · {target ? moduleNameOf(target) : '?'}
                </span>
                <button className="textbtn" onClick={() => removeEdge(e.id)}>
                  삭제
                </button>
              </div>
            );
          })}
        </div>
        {invalid && (
          <div className="warning">
            FSD 경로 형식을 확인하세요. pages/widgets/features/entities는 layer/slice/segment, app/shared는
            layer/segment입니다.
          </div>
        )}
        <div className="panel-actions">
          <button className="btn tiny danger" onClick={() => deleteNode(node.id)}>
            모듈 삭제
          </button>
          <button className="btn primary" onClick={save}>
            변경 적용
          </button>
        </div>
      </div>
    </aside>
  );
}
