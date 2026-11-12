// The analyzed repository: a read-only, slice-level picture of what exists on disk.
import {
  isDirectLayer,
  layerOf,
  segmentOf,
  sliceOf,
  type PlanEdge,
  type PlanNode,
} from '@/entities/plan/@x/repository';
import { createStore } from '@/shared/lib';

/** One slice (or, in app/shared, one segment) with the slices it depends on and is used by. */
export interface RepoSlice {
  /** Layer plus slice path, e.g. "pages/home/assignment" or "shared/api". */
  key: string;
  layer: string;
  name: string;
  segments: string[];
  imports: string[];
  importedBy: string[];
}

export interface Repository {
  sourceRoot: string;
  analyzedAt: string;
  unresolvedImports: number;
  slices: RepoSlice[];
  /** Segment-level scan result, kept so a slice can be brought into the plan in full detail. */
  nodes: PlanNode[];
  edges: PlanEdge[];
}

export interface ScanInfo {
  sourceRoot?: string;
  unresolvedImports?: number;
}

/** The overview key of the slice a segment node belongs to. */
export const sliceKeyOf = (node: PlanNode) => {
  const layer = layerOf(node);
  return isDirectLayer(layer) ? node.path : `${layer}/${sliceOf(node)}`;
};

/** Folds a segment-level scan into slices and slice-to-slice dependencies. */
export function buildRepository(nodes: PlanNode[], edges: PlanEdge[], info: ScanInfo): Repository {
  const slices = new Map<string, RepoSlice>();
  const keyOfNode = new Map<string, string>();
  for (const node of nodes) {
    const layer = layerOf(node);
    const key = sliceKeyOf(node);
    keyOfNode.set(node.id, key);
    const slice = slices.get(key) ?? {
      key,
      layer,
      name: key.slice(layer.length + 1),
      segments: [],
      imports: [],
      importedBy: [],
    };
    if (!isDirectLayer(layer)) slice.segments.push(segmentOf(node));
    slices.set(key, slice);
  }
  const seen = new Set<string>();
  for (const edge of edges) {
    const from = keyOfNode.get(edge.from);
    const to = keyOfNode.get(edge.to);
    if (!from || !to || from === to || seen.has(from + '>' + to)) continue;
    seen.add(from + '>' + to);
    slices.get(from)?.imports.push(to);
    slices.get(to)?.importedBy.push(from);
  }
  return {
    sourceRoot: info.sourceRoot ?? '',
    analyzedAt: new Date().toISOString(),
    unresolvedImports: info.unresolvedImports ?? 0,
    slices: [...slices.values()].toSorted((a, b) => a.key.localeCompare(b.key)),
    nodes,
    edges,
  };
}

const STORAGE_KEY = 'factory-repository-v1';

function loadRepository(): Repository | null {
  try {
    const saved = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? 'null');
    return saved && Array.isArray(saved.slices) ? (saved as Repository) : null;
  } catch {
    return null;
  }
}

export const repositoryStore = createStore<Repository | null>(loadRepository());

repositoryStore.subscribe(() => {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(repositoryStore.get()));
  } catch {
    // A large scan may not fit in storage; it then has to be analyzed again after a restart.
  }
});
