import type { PointerEvent } from 'react';
import { layerOf, segmentOf, sliceOf } from '../model/path';
import type { PlanNode } from '../model/types';

interface NodeCardProps {
  node: PlanNode;
  requestCount: number;
  selected: boolean;
  connectSource: boolean;
  onClick: () => void;
  onEdit: () => void;
  onToggleConnect: () => void;
  onPointerDown: (ev: PointerEvent<HTMLDivElement>) => void;
  onPointerMove: (ev: PointerEvent<HTMLDivElement>) => void;
  onPointerUp: () => void;
  onPointerCancel: () => void;
}

/** One module on the canvas. The header is the drag handle. */
export function NodeCard({
  node,
  requestCount,
  selected,
  connectSource,
  onClick,
  onEdit,
  onToggleConnect,
  ...drag
}: NodeCardProps) {
  const segment = segmentOf(node);
  const slice = sliceOf(node);
  const className = ['node', selected && 'selected', connectSource && 'connect-source'].filter(Boolean).join(' ');
  return (
    <article
      className={className}
      data-id={node.id}
      data-layer={layerOf(node)}
      style={{ left: node.x, top: node.y }}
      aria-label={node.path}
      onClick={onClick}
    >
      <div className="node-head" {...drag}>
        <div className="node-sub">
          <span className="layerbadge">{segment}</span>
          <span className="mini">{slice ? 'slice / ' + slice : 'direct segment'}</span>
        </div>
        <div className="nodename">
          {slice ?? segment}
          <span className="node-segment"> / {segment}</span>
        </div>
        <div className="node-desc">{node.description}</div>
      </div>
      <div className="io">
        <IoColumn label="INPUT" ports={node.inputs} />
        <IoColumn label="OUTPUT" ports={node.outputs} output />
      </div>
      <div className="node-bottom">
        <span>{requestCount} requests</span>
        <div className="node-actions">
          <button
            className="textbtn"
            onClick={(ev) => {
              ev.stopPropagation();
              onEdit();
            }}
          >
            수정
          </button>
          <button
            className="textbtn link"
            onClick={(ev) => {
              ev.stopPropagation();
              onToggleConnect();
            }}
          >
            연결
          </button>
        </div>
      </div>
    </article>
  );
}

function IoColumn({ label, ports, output = false }: { label: string; ports: string[]; output?: boolean }) {
  return (
    <div className="io-col">
      <div className="io-label">
        <span className={output ? 'port output-port' : 'port input-port'} /> {label}
      </div>
      <div className="io-pills">
        {ports.length === 0 && <span className="minor-note">—</span>}
        {ports.map((p, i) => (
          <span key={i} className={output ? 'io-pill output' : 'io-pill'} title={p}>
            {p}
          </span>
        ))}
      </div>
    </div>
  );
}
