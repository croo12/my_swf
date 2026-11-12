import { Fragment, useState } from 'react';
import {
  cleanPath,
  closeEditor,
  isValidPath,
  planStore,
  selectionStore,
  titleOf,
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

/** Side panel for editing the selected node. Renders nothing while no editor is open. */
export function NodeEditor() {
  const { plan } = useStore(planStore);
  const { selectedId, editorOpen, editorSession } = useStore(selectionStore);
  const node = plan.nodes.find((n) => n.id === selectedId);
  if (!editorOpen || !node) return null;
  // The key restarts the form whenever the editor is opened again.
  return <NodeEditorForm key={editorSession} node={node} nodes={plan.nodes} edges={plan.edges} />;
}

function NodeEditorForm({ node, nodes, edges }: { node: PlanNode; nodes: PlanNode[]; edges: PlanEdge[] }) {
  const [path, setPath] = useState(node.path);
  const [description, setDescription] = useState(node.description);
  const [inputs, setInputs] = useState(node.inputs.join('\n'));
  const [outputs, setOutputs] = useState(node.outputs.join('\n'));
  const [invalid, setInvalid] = useState(false);

  const pathParts = cleanPath(path).split('/').filter(Boolean);
  const related = edges.filter((e) => e.from === node.id || e.to === node.id);
  const otherPath = (e: PlanEdge) => nodes.find((n) => n.id === (e.from === node.id ? e.to : e.from))?.path;

  const save = () => {
    const cleaned = cleanPath(path);
    if (!isValidPath(cleaned)) {
      setInvalid(true);
      return;
    }
    saveNode(node.id, {
      path: cleaned,
      description: description.trim(),
      inputs: lines(inputs),
      outputs: lines(outputs),
    });
  };

  return (
    <aside className="overlay open" aria-label="노드 편집">
      <div className="overlay-head">
        <div>
          <div className="panel-tag">EDIT ARCHITECTURE NODE</div>
          <h2>{titleOf(node)}</h2>
        </div>
        <button className="close" aria-label="닫기" onClick={closeEditor}>
          ×
        </button>
      </div>
      <div className="overlay-body">
        <div className="field">
          <label htmlFor="editPath">FSD 경로</label>
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
          <label htmlFor="editDesc">모듈 책임</label>
          <textarea id="editDesc" rows={2} value={description} onChange={(ev) => setDescription(ev.target.value)} />
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
          {related.map((e) => (
            <div key={e.id} className="edge-row">
              <span>
                {e.type} · {otherPath(e)}
              </span>
              <button className="textbtn" onClick={() => removeEdge(e.id)}>
                삭제
              </button>
            </div>
          ))}
        </div>
        {invalid && (
          <div className="warning">
            FSD 경로 형식을 확인하세요. pages/widgets/features/entities는 layer/slice/segment, app/shared는
            layer/segment입니다.
          </div>
        )}
        <div className="panel-actions">
          <button className="btn tiny danger" onClick={() => deleteNode(node.id)}>
            노드 삭제
          </button>
          <button className="btn primary" onClick={save}>
            변경 적용
          </button>
        </div>
      </div>
    </aside>
  );
}
