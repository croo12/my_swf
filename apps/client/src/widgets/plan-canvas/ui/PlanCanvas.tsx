import { useRef, useState, type PointerEvent, type RefObject } from 'react';
import {
  CONTENT_LEFT,
  Edges,
  LayerLanes,
  NodeCard,
  SliceGroups,
  canvasHeight,
  canvasWidth,
  layerAtY,
  layerOf,
  openEditor,
  planStore,
  selectionStore,
  type PlanNode,
} from '@/entities/plan';
import { activateNode, moveNode, relocateNode, toggleConnectFrom } from '@/features/edit-graph';
import { useStore } from '@/shared/lib';

interface DragStart {
  id: string;
  startX: number;
  startY: number;
  x: number;
  y: number;
  moved: boolean;
}

/** Half the height of a node card: the card belongs to the lane its middle is over. */
const NODE_MIDDLE = 100;

/** The scrollable graph: layer lanes, slice groups, relation arrows, and draggable nodes. */
export function PlanCanvas({ scrollRef }: { scrollRef: RefObject<HTMLDivElement | null> }) {
  const { plan } = useStore(planStore);
  const { selectedId, connectFromId } = useStore(selectionStore);
  // While a node is dragged its position lives here; the plan only changes on drop (one undo step).
  const [dragPos, setDragPos] = useState<{ id: string; x: number; y: number } | null>(null);
  const dragStart = useRef<DragStart | null>(null);
  const suppressClick = useRef(false);

  const nodes = dragPos
    ? plan.nodes.map((n) => (n.id === dragPos.id ? { ...n, x: dragPos.x, y: dragPos.y } : n))
    : plan.nodes;
  const width = canvasWidth(nodes);
  const height = canvasHeight();
  const connectSource = connectFromId ? plan.nodes.find((n) => n.id === connectFromId) : undefined;
  const dragged = dragPos ? plan.nodes.find((n) => n.id === dragPos.id) : undefined;
  const dropLayer = dragPos ? layerAtY(dragPos.y + NODE_MIDDLE) : null;
  // Shown only while the card is over a lane other than its own.
  const dragLayer = dragged && dropLayer && dropLayer !== layerOf(dragged) ? dropLayer : null;

  const startDrag = (ev: PointerEvent<HTMLDivElement>, node: PlanNode) => {
    if (ev.button !== 0 || connectFromId) return;
    dragStart.current = { id: node.id, startX: ev.clientX, startY: ev.clientY, x: node.x, y: node.y, moved: false };
    ev.currentTarget.setPointerCapture(ev.pointerId);
  };
  const moveDrag = (ev: PointerEvent<HTMLDivElement>, node: PlanNode) => {
    const drag = dragStart.current;
    if (!drag || drag.id !== node.id) return;
    const dx = ev.clientX - drag.startX;
    const dy = ev.clientY - drag.startY;
    if (Math.abs(dx) + Math.abs(dy) > 5) drag.moved = true;
    if (!drag.moved) return;
    // The card follows the pointer; where it is dropped decides its layer.
    setDragPos({
      id: drag.id,
      x: Math.max(CONTENT_LEFT + 10, drag.x + dx),
      y: Math.max(0, Math.min(height - NODE_MIDDLE, drag.y + dy)),
    });
  };
  const endDrag = (commit: boolean) => {
    const drag = dragStart.current;
    dragStart.current = null;
    if (drag?.moved && commit && dragPos) {
      // The click that follows a drag must not open the editor.
      suppressClick.current = true;
      setTimeout(() => {
        suppressClick.current = false;
      }, 0);
      const node = plan.nodes.find((n) => n.id === dragPos.id);
      const layer = layerAtY(dragPos.y + NODE_MIDDLE);
      // Dropping in another lane places the module in that layer; within a lane only x changes.
      if (node && layer !== layerOf(node)) relocateNode(node.id, layer);
      else if (node) moveNode(node.id, dragPos.x, node.y);
    }
    setDragPos(null);
  };
  const clickNode = (id: string) => {
    if (suppressClick.current) {
      suppressClick.current = false;
      return;
    }
    activateNode(id);
  };

  return (
    <>
      <div className="graphscroll" ref={scrollRef}>
        <div className="workspace" style={{ width, height }}>
          <LayerLanes nodes={nodes} width={width} />
          <SliceGroups nodes={nodes} />
          <Edges edges={plan.edges} nodes={nodes} width={width} height={height} />
          <div>
            {nodes.map((n) => (
              <NodeCard
                key={n.id}
                node={n}
                requestCount={plan.requests.filter((r) => r.nodeId === n.id).length}
                selected={selectedId === n.id}
                connectSource={connectFromId === n.id}
                dragging={dragPos?.id === n.id}
                onClick={() => clickNode(n.id)}
                onEdit={() => openEditor(n.id)}
                onToggleConnect={() => toggleConnectFrom(n.id)}
                onPointerDown={(ev) => startDrag(ev, n)}
                onPointerMove={(ev) => moveDrag(ev, n)}
                onPointerUp={() => endDrag(true)}
                onPointerCancel={() => endDrag(false)}
              />
            ))}
          </div>
        </div>
      </div>
      {plan.nodes.length === 0 && (
        <div className="plan-empty">
          <strong>아직 계획이 없습니다</strong>
          <p>
            아래에 만들 기능을 적고 <b>✦ 계획 요청</b>을 누르면, 필요한 모듈과 각 모듈을 둘 위치를 제안합니다.
          </p>
          <p>상단의 ＋ 노드로 모듈을 직접 추가할 수도 있습니다.</p>
        </div>
      )}
      {dragLayer && <div className="canvas-tip">{dragLayer} 레이어에 놓기</div>}
      {connectSource && (
        <div className="canvas-tip mode">연결 대상 노드를 클릭하세요. 출발점: {connectSource.path}</div>
      )}
    </>
  );
}
