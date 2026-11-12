import { useRef } from 'react';
import { laneY, type Layer } from '@/entities/plan';
import { ConnectionModal } from '@/features/connect-server';
import { NodeEditor } from '@/features/edit-graph';
import { Toast } from '@/shared/ui/toast';
import { CommandBar } from '@/widgets/command-bar';
import { PlanCanvas } from '@/widgets/plan-canvas';
import { WorkbenchHeader } from '@/widgets/workbench-header';

/** The single screen of the app: header, graph canvas with its editor overlay, and the command bar. */
export function WorkbenchPage() {
  const scrollRef = useRef<HTMLDivElement>(null);
  const jumpToLayer = (layer: Layer) => scrollRef.current?.scrollTo({ top: laneY(layer) - 18, behavior: 'smooth' });

  return (
    <>
      <div className="app">
        <WorkbenchHeader onJumpToLayer={jumpToLayer} />
        <main className="stage">
          <PlanCanvas scrollRef={scrollRef} />
          <NodeEditor />
          <Toast />
        </main>
        <CommandBar />
      </div>
      <ConnectionModal />
    </>
  );
}
