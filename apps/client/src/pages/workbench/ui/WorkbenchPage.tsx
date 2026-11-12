import { ConnectionModal } from '@/features/connect-server';
import { NodeEditor } from '@/features/edit-graph';
import { viewStore } from '@/features/switch-view';
import { useStore } from '@/shared/lib';
import { Toast } from '@/shared/ui/toast';
import { CommandBar } from '@/widgets/command-bar';
import { PlanCanvas } from '@/widgets/plan-canvas';
import { RepositoryOverview } from '@/widgets/repository-overview';
import { WorkbenchHeader } from '@/widgets/workbench-header';

/**
 * The single screen of the app. It has two views: the read-only repository overview, and the
 * plan, where the graph is edited and design requests are written.
 */
export function WorkbenchPage() {
  const view = useStore(viewStore);

  return (
    <>
      <div className="app">
        <WorkbenchHeader />
        <main className="stage">
          {view === 'plan' ? (
            <>
              <PlanCanvas />
              <NodeEditor />
            </>
          ) : (
            <RepositoryOverview />
          )}
          <Toast />
        </main>
        {view === 'plan' && <CommandBar />}
      </div>
      <ConnectionModal />
    </>
  );
}
