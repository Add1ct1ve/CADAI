import type { AppConfig } from '$lib/types';
import { getSettings, updateSettings } from '$lib/services/tauri';

const defaultConfig: AppConfig = {
  ai_provider: 'openai',
  api_key: null,
  model: 'gpt-4',
  python_path: null,
  theme: 'dark',
};

let config = $state<AppConfig>({ ...defaultConfig });
let loaded = $state(false);
let loadError = $state<string | null>(null);

export function getSettingsStore() {
  return {
    get config() {
      return config;
    },
    get loaded() {
      return loaded;
    },
    get loadError() {
      return loadError;
    },
    async load() {
      try {
        loadError = null;
        config = await getSettings();
        loaded = true;
      } catch (err) {
        loadError = String(err);
        console.error('Failed to load settings:', err);
      }
    },
    async save() {
      try {
        loadError = null;
        await updateSettings(config);
      } catch (err) {
        loadError = String(err);
        console.error('Failed to save settings:', err);
      }
    },
    update(partial: Partial<AppConfig>) {
      config = { ...config, ...partial };
    },
  };
}
