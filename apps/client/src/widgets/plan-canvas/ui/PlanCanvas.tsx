import { useRef, useState, type PointerEvent } from 'react';
import {
  Edges,
  LayerLanes,
  NODE_H,
  NodeCard,
  ServerBoundary,
  SliceGroups,
  layerAtY,
  layerOf,
  layoutStore,
  planStore,
  selectionStore,
  type PlanNode,
} from '@/entities/plan';
import { activateNode, relocateNode, toggleConnectFrom } from '@/features/edit-graph';
import { useStore } from '@/shared/lib';

interface DragStart {
  id: string;
  startX: number;
  startY: number;
  x: number;
  y: number;
  moved: boolean;
}

/**
 * The plan as a graph. Positions and relation lines come from the automatic layout; a module can
 * be dragged into another layer to change where it is placed.
 */
export function PlanCanvas() {
  const { plan } = useStore(planStore);
  const layout = useStore(layoutStore);
  const { selectedId, connectFromId } = useStore(selectionStore);
  // While a module is dragged its position lives here; the plan only changes on drop.
  const [dragPos, setDragPos] = useState<{ id: string; x: number; y: number } | null>(null);
  const dragStart = useRef<DragStart | null>(null);
  const suppressClick = useRef(false);

  // Modules the layout has not placed yet (it runs just after a change) are not drawn.
  const placed = plan.nodes.filter((n) => layout.positions[n.id]);
  const connectSource = connectFromId ? plan.nodes.find((n) => n.id === connectFromId) : undefined;
  const dragged = dragPos ? plan.nodes.find((n) => n.id === dragPos.id) : undefined;
  const dropLayer = dragPos ? layerAtY(dragPos.y + NODE_H / 2, layout.lanes) : null;
  // Shown only while the card is over a lane other than its own.
  const dragLayer = dragged && dropLayer && dropLayer !== layerOf(dragged) ? dropLayer : null;

  const startDrag = (ev: PointerEvent<HTMLDivElement>, node: PlanNode) => {
    const position = layout.positions[node.id];
    if (ev.button !== 0 || connectFromId || !position) return;
    dragStart.current = { id: node.id, startX: ev.clientX, startY: ev.clientY, ...position, moved: false };
    ev.currentTarget.setPointerCapture(ev.pointerId);
  };
  const moveDrag = (ev: PointerEvent<HTMLDivElement>, node: PlanNode) => {
    const drag = dragStart.current;
    if (!drag || drag.id !== node.id) return;
    const dx = ev.clientX - drag.startX;
    const dy = ev.clientY - drag.startY;
    if (Math.abs(dx) + Math.abs(dy) > 5) drag.moved = true;
    if (!drag.moved) return;
    setDragPos({ id: drag.id, x: drag.x + dx, y: drag.y + dy });
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
      // Dropping in another lane places the module in that layer. Anywhere else it returns to
      // its place: positions inside a layer are decided by the layout.
      if (dragLayer) relocateNode(drag.id, dragLayer);
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
      <div className="graphscroll">
        <div className="workspace" style={{ width: layout.width, height: layout.height }}>
          <LayerLanes lanes={layout.lanes} />
          <SliceGroups slices={layout.slices} />
          <ServerBoundary server={layout.server} nodes={plan.nodes} />
          <Edges
            routes={layout.edges}
            edges={plan.edges}
            nodes={plan.nodes}
            width={layout.width}
            height={layout.height}
            focusId={selectedId}
          />
          <div>
            {placed.map((n) => {
              const position = dragPos?.id === n.id ? dragPos : layout.positions[n.id];
              return (
                <NodeCard
                  key={n.id}
                  node={n}
                  x={position.x}
                  y={position.y}
                  requestCount={plan.requests.filter((r) => r.nodeId === n.id).length}
                  selected={selectedId === n.id}
                  connectSource={connectFromId === n.id}
                  dragging={dragPos?.id === n.id}
                  onClick={() => clickNode(n.id)}
                  onToggleConnect={() => toggleConnectFrom(n.id)}
                  onPointerDown={(ev) => startDrag(ev, n)}
                  onPointerMove={(ev) => moveDrag(ev, n)}
                  onPointerUp={() => endDrag(true)}
                  onPointerCancel={() => endDrag(false)}
                />
              );
            })}
          </div>
        </div>
      </div>
      {plan.nodes.length === 0 && (
        <div className="plan-empty">
          <strong>아직 계획이 없습니다</strong>
          <p>
            아래에 만들 기능을 적고 <b>✦ 계획 요청</b>을 누르면, 필요한 모듈과 각 모듈을 둘 위치를 제안합니다.
          </p>
          <p>상단의 ＋ 모듈로 직접 추가할 수도 있습니다.</p>
        </div>
      )}
      {dragLayer && <div className="canvas-tip">{dragLayer} 레이어에 놓기</div>}
      {connectSource && (
        <div className="canvas-tip mode">
          연결 대상 모듈을 클릭하세요. 출발점: {connectSource.name ?? connectSource.path}
        </div>
      )}
    </>
  );
}
