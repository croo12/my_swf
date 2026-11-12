import { useRef, useState, type PointerEvent, type RefObject } from 'react';
import {
  CONTENT_LEFT,
  Edges,
  LayerLanes,
  NodeCard,
  SliceGroups,
  canvasHeight,
  canvasWidth,
  layerOf,
  nodeY,
  openEditor,
  planStore,
  selectionStore,
  type PlanNode,
} from '@/entities/plan';
import { activateNode, moveNode, toggleConnectFrom } from '@/features/edit-graph';
import { useStore } from '@/shared/lib';

interface DragStart {
  id: string;
  startX: number;
  startY: number;
  x: number;
  moved: boolean;
}

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

  const startDrag = (ev: PointerEvent<HTMLDivElement>, node: PlanNode) => {
    if (ev.button !== 0 || connectFromId) return;
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
    setDragPos({
      id: drag.id,
      x: Math.max(CONTENT_LEFT + 10, Math.min(3000, drag.x + dx)),
      y: nodeY(layerOf(node)),
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
      moveNode(dragPos.id, dragPos.x, dragPos.y);
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
      {connectSource && (
        <div className="canvas-tip mode">연결 대상 노드를 클릭하세요. 출발점: {connectSource.path}</div>
      )}
    </>
  );
}
