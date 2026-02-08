import type { ViewportState } from '$lib/types';
import type { ViewportEngine } from '$lib/services/viewport-engine';
import type { CameraState } from '$lib/types/cad';

let isLoading = $state(false);
let hasModel = $state(false);
let error = $state<string | null>(null);
let pendingStl = $state<string | null>(null);
let pendingClear = $state(false);
let engineRef = $state<ViewportEngine | null>(null);
let gridVisible = $state(true);
let axesVisible = $state(true);

export function getViewportStore() {
  return {
    get isLoading() {
      return isLoading;
    },
    get hasModel() {
      return hasModel;
    },
    get error() {
      return error;
    },
    setLoading(val: boolean) {
      isLoading = val;
    },
    setHasModel(val: boolean) {
      hasModel = val;
    },
    setError(err: string | null) {
      error = err;
    },
    get pendingStl() {
      return pendingStl;
    },
    setPendingStl(base64: string | null) {
      pendingStl = base64;
    },
    get pendingClear() {
      return pendingClear;
    },
    setPendingClear(val: boolean) {
      pendingClear = val;
    },
    setEngine(engine: ViewportEngine | null) {
      engineRef = engine;
    },
    getCameraState(): CameraState | null {
      return engineRef?.getCameraState() ?? null;
    },
    setCameraState(state: CameraState) {
      engineRef?.setCameraState(state);
    },
    get gridVisible() {
      return gridVisible;
    },
    get axesVisible() {
      return axesVisible;
    },
    setGridVisible(val: boolean) {
      gridVisible = val;
      engineRef?.setGridVisible(val);
    },
    setAxesVisible(val: boolean) {
      axesVisible = val;
      engineRef?.setAxesVisible(val);
    },
    animateToView(view: 'top' | 'front' | 'right' | 'iso') {
      engineRef?.animateToView(view);
    },
    fitAll() {
      engineRef?.fitAll();
    },
    getState(): ViewportState {
      return { isLoading, hasModel, error };
    },
  };
}
