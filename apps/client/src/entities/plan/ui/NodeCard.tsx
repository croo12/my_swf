import type { PointerEvent } from 'react';
import { kindOf, layerOf, moduleNameOf } from '../model/path';
import { MODULE_KIND_LABELS, type PlanNode } from '../model/types';

interface NodeCardProps {
  node: PlanNode;
  requestCount: number;
  selected: boolean;
  connectSource: boolean;
  dragging: boolean;
  onClick: () => void;
  onEdit: () => void;
  onToggleConnect: () => void;
  onPointerDown: (ev: PointerEvent<HTMLDivElement>) => void;
  onPointerMove: (ev: PointerEvent<HTMLDivElement>) => void;
  onPointerUp: () => void;
  onPointerCancel: () => void;
}

/** One planned module on the canvas: what it is, where it is placed, and its inputs and outputs. */
export function NodeCard({
  node,
  requestCount,
  selected,
  connectSource,
  dragging,
  onClick,
  onEdit,
  onToggleConnect,
  ...drag
}: NodeCardProps) {
  const kind = kindOf(node);
  const className = ['node', selected && 'selected', connectSource && 'connect-source', dragging && 'dragging']
    .filter(Boolean)
    .join(' ');
  return (
    <article
      className={className}
      data-id={node.id}
      data-layer={layerOf(node)}
      data-kind={kind}
      style={{ left: node.x, top: node.y }}
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
      <div className="io">
        <IoColumn label="INPUT" ports={node.inputs} />
        <IoColumn label="OUTPUT" ports={node.outputs} output />
      </div>
      <div className="node-bottom">
        <span title={(node.behaviors ?? []).join('\n')}>
          {[
            node.behaviors?.length ? `동작 ${node.behaviors.length}` : '',
            requestCount > 0 ? `요청 ${requestCount}` : '',
          ]
            .filter(Boolean)
            .join(' · ')}
        </span>
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

/** The card has a fixed height; the editor shows the full interface. */
const VISIBLE_PORTS = 1;

function IoColumn({ label, ports, output = false }: { label: string; ports: string[]; output?: boolean }) {
  return (
    <div className="io-col">
      <div className="io-label">
        <span className={output ? 'port output-port' : 'port input-port'} /> {label}
      </div>
      <div className="io-pills">
        {ports.length === 0 && <span className="minor-note">—</span>}
        {ports.slice(0, VISIBLE_PORTS).map((p, i) => (
          <span key={i} className={output ? 'io-pill output' : 'io-pill'} title={p}>
            {p}
          </span>
        ))}
        {ports.length > VISIBLE_PORTS && (
          <span className="io-more" title={ports.slice(VISIBLE_PORTS).join('\n')}>
            +{ports.length - VISIBLE_PORTS}
          </span>
        )}
      </div>
    </div>
  );
}
