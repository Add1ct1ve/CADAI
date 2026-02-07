import type { ViewportState } from '$lib/types';

let isLoading = $state(false);
let hasModel = $state(false);
let error = $state<string | null>(null);
let pendingStl = $state<string | null>(null);
let pendingClear = $state(false);

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
    getState(): ViewportState {
      return { isLoading, hasModel, error };
    },
  };
}
