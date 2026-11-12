import type { PointerEvent } from 'react';
import { kindOf, layerOf, moduleNameOf } from '../model/path';
import { INTERFACE_KIND_LABELS, MODULE_KIND_LABELS, type PlanNode } from '../model/types';

interface NodeCardProps {
  node: PlanNode;
  x: number;
  y: number;
  requestCount: number;
  selected: boolean;
  connectSource: boolean;
  dragging: boolean;
  onClick: () => void;
  onToggleConnect: () => void;
  onPointerDown: (ev: PointerEvent<HTMLDivElement>) => void;
  onPointerMove: (ev: PointerEvent<HTMLDivElement>) => void;
  onPointerUp: () => void;
  onPointerCancel: () => void;
}

/** The card has a fixed height; the editor shows everything. */
const VISIBLE_INTERFACES = 3;

/** One planned module: what it is, where it is placed, what it exposes, and what it asks of the server. */
export function NodeCard({
  node,
  x,
  y,
  requestCount,
  selected,
  connectSource,
  dragging,
  onClick,
  onToggleConnect,
  ...drag
}: NodeCardProps) {
  const kind = kindOf(node);
  const className = ['node', selected && 'selected', connectSource && 'connect-source', dragging && 'dragging']
    .filter(Boolean)
    .join(' ');
  const hidden = node.interfaces.length - VISIBLE_INTERFACES;
  return (
    <article
      className={className}
      data-id={node.id}
      data-layer={layerOf(node)}
      data-kind={kind}
      style={{ left: x, top: y }}
      aria-label={moduleNameOf(node)}
      onClick={onClick}
    >
      <div className="node-head" {...drag}>
        <div className="node-sub">
          <span className="kindbadge">{MODULE_KIND_LABELS[kind]}</span>
          <span className="mini" title={node.path}>
            {node.path}
          </span>
        </div>
        <div className="nodename">{moduleNameOf(node)}</div>
        <div className="node-desc">{node.description}</div>
      </div>
      <div className="node-interfaces">
        <div className="io-label">
          <span className="port output-port" /> 인터페이스 {node.interfaces.length}
        </div>
        <div className="io-pills">
          {node.interfaces.length === 0 && <span className="minor-note">—</span>}
          {node.interfaces.slice(0, VISIBLE_INTERFACES).map((item) => (
            <span
              key={item.name}
              className={`io-pill output kind-${item.kind}`}
              title={[
                `${INTERFACE_KIND_LABELS[item.kind]} · ${item.name}`,
                item.condition && `조건: ${item.condition}`,
                item.result && `결과: ${item.result}`,
                `입력: ${item.inputs.join(', ') || '없음'}`,
                `출력: ${item.outputs.join(', ') || '없음'}`,
              ]
                .filter(Boolean)
                .join('\n')}
            >
              {item.kind === 'server' ? '⇄ ' : ''}
              {item.name}
            </span>
          ))}
          {hidden > 0 && <span className="io-more">+{hidden}</span>}
        </div>
      </div>
      <div className="node-bottom">
        <span>{requestCount > 0 ? `요청 ${requestCount}` : ''}</span>
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
    </article>
  );
}
