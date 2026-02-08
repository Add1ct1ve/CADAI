import type { ToolId } from '$lib/types/cad';

let activeTool = $state<ToolId>('select');
let uniformScale = $state(false);
let translateSnap = $state<number | null>(1);
let rotationSnap = $state<number | null>(15);

export function getToolStore() {
  return {
    get activeTool() {
      return activeTool;
    },

    setTool(tool: ToolId) {
      activeTool = tool;
    },

    revertToSelect() {
      activeTool = 'select';
    },

    get isAddTool(): boolean {
      return activeTool.startsWith('add-');
    },

    get uniformScale() {
      return uniformScale;
    },
    setUniformScale(val: boolean) {
      uniformScale = val;
    },

    get translateSnap() {
      return translateSnap;
    },
    setTranslateSnap(val: number | null) {
      translateSnap = val;
    },

    get rotationSnap() {
      return rotationSnap;
    },
    setRotationSnap(val: number | null) {
      rotationSnap = val;
    },
  };
}
