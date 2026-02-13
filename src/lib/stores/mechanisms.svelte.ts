import { installMechanismPack, listMechanisms, removeMechanismPack, searchMechanisms } from '$lib/services/tauri';
import type { MechanismImportReport, MechanismItem, MechanismPackage } from '$lib/types';

let loading = $state(false);
let loadError = $state<string | null>(null);
let mechanisms = $state<MechanismItem[]>([]);
let packages = $state<MechanismPackage[]>([]);
let lastQuery = $state('');

export function getMechanismStore() {
  return {
    get loading() {
      return loading;
    },
    get loadError() {
      return loadError;
    },
    get mechanisms() {
      return mechanisms;
    },
    get packages() {
      return packages;
    },
    get lastQuery() {
      return lastQuery;
    },
    async refresh() {
      loading = true;
      loadError = null;
      try {
        const result = await listMechanisms();
        packages = result.packages;
        mechanisms = result.mechanisms;
      } catch (err) {
        loadError = String(err);
      } finally {
        loading = false;
      }
    },
    async search(query: string) {
      loading = true;
      loadError = null;
      lastQuery = query;
      try {
        if (!query.trim()) {
          await this.refresh();
          return;
        }
        mechanisms = await searchMechanisms(query, 100);
      } catch (err) {
        loadError = String(err);
      } finally {
        loading = false;
      }
    },
    async install(manifestUrl: string): Promise<MechanismImportReport> {
      const report = await installMechanismPack(manifestUrl);
      await this.refresh();
      return report;
    },
    async remove(packageId: string): Promise<boolean> {
      const removed = await removeMechanismPack(packageId);
      if (removed) {
        await this.refresh();
      }
      return removed;
    },
  };
}
