import type { SceneObject, ObjectId } from '$lib/types/cad';

export interface SceneSnapshot {
  objects: SceneObject[];
  selectedIds: ObjectId[];
}

const MAX_HISTORY = 50;

let undoStack = $state<SceneSnapshot[]>([]);
let redoStack = $state<SceneSnapshot[]>([]);

export function getHistoryStore() {
  return {
    get canUndo() {
      return undoStack.length > 0;
    },
    get canRedo() {
      return redoStack.length > 0;
    },

    pushSnapshot(snapshot: SceneSnapshot) {
      undoStack = [...undoStack.slice(-(MAX_HISTORY - 1)), snapshot];
      redoStack = [];
    },

    undo(current: SceneSnapshot): SceneSnapshot | null {
      if (undoStack.length === 0) return null;
      const prev = undoStack[undoStack.length - 1];
      undoStack = undoStack.slice(0, -1);
      redoStack = [...redoStack, current];
      return prev;
    },

    redo(current: SceneSnapshot): SceneSnapshot | null {
      if (redoStack.length === 0) return null;
      const next = redoStack[redoStack.length - 1];
      redoStack = redoStack.slice(0, -1);
      undoStack = [...undoStack, current];
      return next;
    },

    clear() {
      undoStack = [];
      redoStack = [];
    },
  };
}
