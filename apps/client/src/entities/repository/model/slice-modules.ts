// The modules inside one slice, read from the repository by the server on request.
import type { PlanEdge, PlanNode } from '@/entities/plan/@x/repository';
import { apiRequest, errorMessage } from '@/shared/api';
import { createStore } from '@/shared/lib';

/** Code outside the slice that uses a module (`incoming`) or that a module uses (`outgoing`). */
export interface OutsideLink {
  module: string;
  placement: string;
  names: string[];
}

/**
 * A slice as it exists on disk. A module is one direct child of a segment (a file, or a folder
 * with everything in it); its interfaces are the names other files import from it.
 */
export interface SliceModules {
  slice: string;
  nodes: PlanNode[];
  edges: PlanEdge[];
  incoming: OutsideLink[];
  outgoing: OutsideLink[];
}

export type SliceModulesState = { status: 'loading' } | { status: 'failed'; reason: string } | SliceModules;

/** Loaded slices by key. Not persisted: the repository may have changed since. */
export const sliceModulesStore = createStore<Record<string, SliceModulesState>>({});

const put = (key: string, state: SliceModulesState) =>
  sliceModulesStore.set({ ...sliceModulesStore.get(), [key]: state });

/** Reads the modules of a slice from the server. Returns null when that fails. */
export async function loadSliceModules(key: string, reload = false): Promise<SliceModules | null> {
  const known = sliceModulesStore.get()[key];
  if (known && !('status' in known) && !reload) return known;
  put(key, { status: 'loading' });
  try {
    const modules = await apiRequest<SliceModules>('/api/repository/modules', 'POST', { slice: key });
    put(key, modules);
    return modules;
  } catch (err) {
    put(key, { status: 'failed', reason: errorMessage(err) });
    return null;
  }
}
