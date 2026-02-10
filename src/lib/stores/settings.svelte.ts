import type { AppConfig } from '$lib/types';
import { getSettings, updateSettings } from '$lib/services/tauri';

const defaultConfig: AppConfig = {
  ai_provider: 'claude',
  api_key: null,
  model: 'claude-sonnet-4-5-20250929',
  python_path: null,
  theme: 'dark',
  ollama_base_url: null,
  openai_base_url: null,
  agent_rules_preset: null,
  enable_code_review: true,
  display_units: 'mm',
  grid_size: 100,
  grid_spacing: 1,
  snap_translate: 1,
  snap_rotation: 15,
  snap_sketch: 0.5,
  enable_consensus: false,
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
