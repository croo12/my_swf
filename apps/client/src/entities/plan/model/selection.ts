// Which node the user is working on. Shared by the canvas, the toolbar, and the node editor.
import { createStore } from '@/shared/lib';

export interface Selection {
  selectedId: string | null;
  editorOpen: boolean;
  /** Bumped each time the editor is opened so its form starts from the current values of the node. */
  editorSession: number;
  /** Source node while the user is picking the target of a new relation. */
  connectFromId: string | null;
}

export const selectionStore = createStore<Selection>({
  selectedId: null,
  editorOpen: false,
  editorSession: 0,
  connectFromId: null,
});

export const openEditor = (id: string) =>
  selectionStore.set((s) => ({
    selectedId: id,
    editorOpen: true,
    editorSession: s.editorSession + 1,
    connectFromId: null,
  }));
export const closeEditor = () => selectionStore.set((s) => ({ ...s, editorOpen: false }));
export const startConnecting = (id: string) =>
  selectionStore.set((s) => ({ ...s, selectedId: id, editorOpen: false, connectFromId: id }));
export const stopConnecting = () => selectionStore.set((s) => ({ ...s, connectFromId: null }));
export const clearSelection = () =>
  selectionStore.set((s) => ({ ...s, selectedId: null, editorOpen: false, connectFromId: null }));
