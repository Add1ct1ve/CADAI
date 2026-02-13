import type { GenerationEntry } from '$lib/types';

const MAX_ENTRIES = 20;

let entries = $state<GenerationEntry[]>([]);
let compareIds = $state<[string, string] | null>(null);
let previewId = $state<string | null>(null);

export function getGenerationHistoryStore() {
  return {
    get entries() {
      return entries;
    },
    get compareIds() {
      return compareIds;
    },
    get previewId() {
      return previewId;
    },

    addEntry(entry: GenerationEntry) {
      entries = [entry, ...entries];
      // Evict oldest non-pinned if over limit
      if (entries.length > MAX_ENTRIES) {
        const lastNonPinnedIdx = entries.findLastIndex((e) => !e.pinned);
        if (lastNonPinnedIdx >= 0) {
          entries = entries.filter((_, i) => i !== lastNonPinnedIdx);
        }
      }
    },

    togglePin(id: string) {
      const idx = entries.findIndex((e) => e.id === id);
      if (idx >= 0) {
        entries[idx] = { ...entries[idx], pinned: !entries[idx].pinned };
      }
    },

    setCompare(ids: [string, string] | null) {
      compareIds = ids;
    },

    setPreview(id: string | null) {
      previewId = id;
    },

    getEntry(id: string): GenerationEntry | undefined {
      return entries.find((e) => e.id === id);
    },

    removeEntry(id: string) {
      entries = entries.filter((e) => e.id !== id);
      if (compareIds && (compareIds[0] === id || compareIds[1] === id)) {
        compareIds = null;
      }
      if (previewId === id) {
        previewId = null;
      }
    },

    clear() {
      entries = [];
      compareIds = null;
      previewId = null;
    },
  };
}
