// Which screen is shown: the editable plan, or the read-only overview of the analyzed repository.
import { createStore } from '@/shared/lib';

export type WorkbenchView = 'plan' | 'repository';

export const viewStore = createStore<WorkbenchView>('plan');
export const setView = (view: WorkbenchView) => viewStore.set(view);
