import { useRef, useState, type PointerEvent, type RefObject } from 'react';
import {
  CONTENT_LEFT, LANE_H, LANE_LEFT, LAYERS, LAYER_DESCRIPTIONS, LAYER_LABELS, NODE_H, NODE_W, SLICE_PAD,
  canvasHeight, canvasWidth, isInvalidEdge, laneY, layerOf, segmentOf, sliceOf,
  type Plan, type PlanEdge, type PlanNode,
} from '../model/plan';

interface CanvasProps {
  plan: Plan;
  selected: string | null;
  connectFrom: string | null;
  scrollRef: RefObject<HTMLDivElement | null>;
  onNodeClick: (id: string) => void;
  onEdit: (id: string) => void;
  onToggleConnect: (id: string) => void;
  onMove: (id: string, x: number, y: number) => void;
}

interface DragStart { id: string; startX: number; startY: number; x: number; moved: boolean }

export function Canvas({ plan, selected, connectFrom, scrollRef, onNodeClick, onEdit, onToggleConnect, onMove }: CanvasProps) {
  // While a node is dragged its position lives here; the plan only changes on drop (one undo step).
  const [dragPos, setDragPos] = useState<{ id: string; x: number; y: number } | null>(null);
  const dragStart = useRef<DragStart | null>(null);
  const suppressClick = useRef(false);

  const nodes = dragPos ? plan.nodes.map((n) => (n.id === dragPos.id ? { ...n, x: dragPos.x, y: dragPos.y } : n)) : plan.nodes;
  const width = canvasWidth(nodes);
  const height = canvasHeight();
  const connectSource = connectFrom ? plan.nodes.find((n) => n.id === connectFrom) : undefined;

  const startDrag = (ev: PointerEvent<HTMLDivElement>, node: PlanNode) => {
    if (ev.button !== 0 || connectFrom) return;
    dragStart.current = { id: node.id, startX: ev.clientX, startY: ev.clientY, x: node.x, moved: false };
    ev.currentTarget.setPointerCapture(ev.pointerId);
  };
  const moveDrag = (ev: PointerEvent<HTMLDivElement>, node: PlanNode) => {
    const drag = dragStart.current;
    if (!drag || drag.id !== node.id) return;
    const dx = ev.clientX - drag.startX;
    const dy = ev.clientY - drag.startY;
    if (Math.abs(dx) + Math.abs(dy) > 5) drag.moved = true;
    if (!drag.moved) return;
    // Nodes move horizontally only; they stay inside their layer lane.
    setDragPos({ id: drag.id, x: Math.max(CONTENT_LEFT + 10, Math.min(3000, drag.x + dx)), y: laneY(layerOf(node)) + 62 });
  };
  const endDrag = (commit: boolean) => {
    const drag = dragStart.current;
    dragStart.current = null;
    if (drag?.moved && commit && dragPos) {
      // The click that follows a drag must not open the editor.
      suppressClick.current = true;
      setTimeout(() => { suppressClick.current = false; }, 0);
      onMove(dragPos.id, dragPos.x, dragPos.y);
    }
    setDragPos(null);
  };
  const clickNode = (id: string) => {
    if (suppressClick.current) { suppressClick.current = false; return; }
    onNodeClick(id);
  };

  return (
    <div className="graphscroll" ref={scrollRef}>
      <div className="workspace" style={{ width, height }}>
        <div className="graph-title">
          <strong>FSD ARCHITECTURE / LAYER → SLICE → SEGMENT</strong> <span>· 수평 레이어 안에 slice와 segment를 배치합니다</span>
        </div>
        <LayerLanes nodes={nodes} width={width} />
        <SliceGroups nodes={nodes} />
        <Edges edges={plan.edges} nodes={nodes} width={width} height={height} />
        <div>
          {nodes.map((n) => (
            <NodeCard
              key={n.id}
              node={n}
              requestCount={plan.requests.filter((r) => r.nodeId === n.id).length}
              selected={selected === n.id}
              connectSource={connectFrom === n.id}
              onClick={() => clickNode(n.id)}
              onEdit={() => onEdit(n.id)}
              onToggleConnect={() => onToggleConnect(n.id)}
              onPointerDown={(ev) => startDrag(ev, n)}
              onPointerMove={(ev) => moveDrag(ev, n)}
              onPointerUp={() => endDrag(true)}
              onPointerCancel={() => endDrag(false)}
            />
          ))}
        </div>
        <div className={connectSource ? 'canvas-tip mode' : 'canvas-tip'}>
          {connectSource
            ? `연결 대상 노드를 클릭하세요. 출발점: ${connectSource.path}`
            : 'Layer → Slice → Segment. 모듈 편집, 노드 이동, 연결 추가가 가능합니다.'}
        </div>
      </div>
    </div>
  );
}

function LayerLanes({ nodes, width }: { nodes: PlanNode[]; width: number }) {
  return (
    <div className="layer-groups" aria-hidden="true">
      {LAYERS.map((layer, i) => (
        <section
          key={layer}
          className="layer-lane"
          data-layer={layer}
          style={{ left: LANE_LEFT, top: laneY(layer), width: width - LANE_LEFT * 2, height: LANE_H }}
        >
          <div className="lane-bar">
            <span className="lane-index">0{i + 1}</span>
            <span className="lane-name">{LAYER_LABELS[layer]}</span>
            <span className="lane-rule">{layer === 'app' || layer === 'shared' ? 'DIRECT SEGMENTS' : 'SLICES → SEGMENTS'}</span>
            <span className="lane-count">{nodes.filter((n) => layerOf(n) === layer).length} modules</span>
            <div className="lane-desc">{LAYER_DESCRIPTIONS[layer]}</div>
          </div>
        </section>
      ))}
    </div>
  );
}

function SliceGroups({ nodes }: { nodes: PlanNode[] }) {
  const groups = new Map<string, { layer: string; slice: string; members: PlanNode[] }>();
  for (const n of nodes) {
    const slice = sliceOf(n);
    if (!slice) continue;
    const key = layerOf(n) + '|' + slice;
    const group = groups.get(key) ?? { layer: layerOf(n), slice, members: [] };
    group.members.push(n);
    groups.set(key, group);
  }
  return (
    <div className="slice-groups" aria-hidden="true">
      {[...groups.entries()].map(([key, { layer, slice, members }]) => {
        const left = Math.min(...members.map((n) => n.x)) - SLICE_PAD;
        const right = Math.max(...members.map((n) => n.x + NODE_W)) + SLICE_PAD;
        return (
          <section
            key={key}
            className="slice-group"
            data-layer={layer}
            style={{ left, top: laneY(layer) + 46, width: right - left, height: LANE_H - 52 }}
          >
            <span className="slice-label">
              <span className="slice-symbol">▤</span> {slice}{' '}
              <span className="slice-module-count">{members.length} segment{members.length === 1 ? '' : 's'}</span>
            </span>
          </section>
        );
      })}
    </div>
  );
}

/** Same-layer edges arc above the nodes; cross-layer edges run between the facing node edges. */
function edgeGeometry(a: PlanNode, b: PlanNode) {
  if (layerOf(a) === layerOf(b)) {
    const forwards = b.x > a.x;
    const x1 = forwards ? a.x + NODE_W : a.x;
    const x2 = forwards ? b.x : b.x + NODE_W;
    const y1 = a.y + 100;
    const y2 = b.y + 100;
    const lift = (33 + (Math.abs(x2 - x1) < 150 ? 30 : 0)) * (forwards ? 1 : -1);
    return { d: `M ${x1} ${y1} C ${x1 + lift} ${y1 - 56}, ${x2 - lift} ${y2 - 56}, ${x2} ${y2}`, cx: (x1 + x2) / 2, cy: (y1 + y2) / 2 - 44 };
  }
  const downward = laneY(layerOf(b)) > laneY(layerOf(a));
  const x1 = a.x + NODE_W / 2;
  const x2 = b.x + NODE_W / 2;
  const y1 = downward ? a.y + NODE_H : a.y;
  const y2 = downward ? b.y : b.y + NODE_H;
  const bend = Math.max(45, Math.abs(y2 - y1) * 0.42) * (downward ? 1 : -1);
  return { d: `M ${x1} ${y1} C ${x1} ${y1 + bend}, ${x2} ${y2 - bend}, ${x2} ${y2}`, cx: (x1 + x2) / 2, cy: (y1 + y2) / 2 };
}

function Edges({ edges, nodes, width, height }: { edges: PlanEdge[]; nodes: PlanNode[]; width: number; height: number }) {
  return (
    <svg className="edges" viewBox={`0 0 ${width} ${height}`} style={{ width, height }} aria-label="모듈 관계 그래프">
      <defs>
        <marker id="arrow" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="5" markerHeight="5" orient="auto">
          <path d="M0 0 L10 5 L0 10 z" fill="#758eab" />
        </marker>
        <marker id="arrowDanger" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="5" markerHeight="5" orient="auto">
          <path d="M0 0 L10 5 L0 10 z" fill="#f18081" />
        </marker>
      </defs>
      {edges.map((e) => {
        const a = nodes.find((n) => n.id === e.from);
        const b = nodes.find((n) => n.id === e.to);
        if (!a || !b) return null;
        const { d, cx, cy } = edgeGeometry(a, b);
        const bad = isInvalidEdge(e, nodes);
        const pathClass = ['edge-path', e.type === 'navigates' && 'navigation', bad && 'invalid'].filter(Boolean).join(' ');
        return (
          <g key={e.id}>
            <path className={pathClass} d={d} markerEnd={`url(#${bad ? 'arrowDanger' : 'arrow'})`} />
            <g className="edge-badge">
              <rect className="edge-label-bg" x={cx - 33} y={cy - 11} width="66" height="20" rx="7" />
              <text x={cx} y={cy + 2} textAnchor="middle">{e.type}{bad ? ' ⚠' : ''}</text>
            </g>
          </g>
        );
      })}
    </svg>
  );
}

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

function NodeCard({ node, requestCount, selected, connectSource, onClick, onEdit, onToggleConnect, ...drag }: NodeCardProps) {
  const segment = segmentOf(node);
  const slice = sliceOf(node);
  const className = ['node', selected && 'selected', connectSource && 'connect-source'].filter(Boolean).join(' ');
  return (
    <article className={className} data-id={node.id} data-layer={layerOf(node)} style={{ left: node.x, top: node.y }} aria-label={node.path} onClick={onClick}>
      <div className="node-head" {...drag}>
        <div className="node-sub">
          <span className="layerbadge">{segment}</span>
          <span className="mini">{slice ? 'slice / ' + slice : 'direct segment'}</span>
        </div>
        <div className="nodename">{slice ?? segment}<span className="node-segment"> / {segment}</span></div>
        <div className="node-desc">{node.description}</div>
      </div>
      <div className="io">
        <IoColumn label="INPUT" ports={node.inputs} />
        <IoColumn label="OUTPUT" ports={node.outputs} output />
      </div>
      <div className="node-bottom">
        <span>{requestCount} requests</span>
        <div className="node-actions">
          <button className="textbtn" onClick={(ev) => { ev.stopPropagation(); onEdit(); }}>수정</button>
          <button className="textbtn link" onClick={(ev) => { ev.stopPropagation(); onToggleConnect(); }}>연결</button>
        </div>
      </div>
    </article>
  );
}

function IoColumn({ label, ports, output = false }: { label: string; ports: string[]; output?: boolean }) {
  return (
    <div className="io-col">
      <div className="io-label"><span className={output ? 'port output-port' : 'port input-port'} /> {label}</div>
      <div className="io-pills">
        {ports.length === 0 && <span className="minor-note">—</span>}
        {ports.map((p, i) => (
          <span key={i} className={output ? 'io-pill output' : 'io-pill'} title={p}>{p}</span>
        ))}
      </div>
    </div>
  );
}
