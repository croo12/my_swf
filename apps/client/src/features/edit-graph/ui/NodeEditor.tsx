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

const lines = (text: string) =>
  text
    .split('\n')
    .map((s) => s.trim())
    .filter(Boolean);

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
  const [inputs, setInputs] = useState(node.inputs.join('\n'));
  const [outputs, setOutputs] = useState(node.outputs.join('\n'));
  const [invalid, setInvalid] = useState(false);

  const pathParts = cleanPath(path).split('/').filter(Boolean);
  const related = edges.filter((e) => e.from === node.id || e.to === node.id);
  const other = (e: PlanEdge) => nodes.find((n) => n.id === (e.from === node.id ? e.to : e.from));

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
      inputs: lines(inputs),
      outputs: lines(outputs),
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
          <label htmlFor="editFiles">파일 · 한 줄에 하나씩 (선택)</label>
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
          <label htmlFor="editInputs">INPUT · 한 줄에 하나씩</label>
          <textarea
            id="editInputs"
            rows={3}
            spellCheck={false}
            value={inputs}
            onChange={(ev) => setInputs(ev.target.value)}
          />
        </div>
        <div className="field">
          <label htmlFor="editOutputs">OUTPUT · 한 줄에 하나씩</label>
          <textarea
            id="editOutputs"
            rows={3}
            spellCheck={false}
            value={outputs}
            onChange={(ev) => setOutputs(ev.target.value)}
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
