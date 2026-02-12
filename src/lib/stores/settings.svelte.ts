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
  grid_size: 500,
  grid_spacing: 2,
  snap_translate: 1,
  snap_rotation: 15,
  snap_sketch: 0.5,
  enable_consensus: false,
  auto_approve_plan: false,
  retrieval_enabled: true,
  retrieval_token_budget: 3500,
  telemetry_enabled: true,
  max_validation_attempts: 4,
  generation_reliability_profile: 'reliability_first',
  preview_on_partial_failure: true,
  max_generation_runtime_seconds: 600,
  semantic_contract_strict: true,
  reviewer_mode: 'advisory_only',
  deterministic_fallback_enabled: true,
  fallback_after_part_failures: 1,
  quality_gates_strict: true,
  allow_euler_override: true,
  semantic_bbox_mode: 'semantic_aware',
  mechanisms_enabled: true,
  mechanism_import_enabled: false,
  mechanism_cache_max_mb: 512,
  allowed_spdx_licenses: ['MIT', 'Apache-2.0', 'BSD-2-Clause', 'BSD-3-Clause', 'CC0-1.0'],
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
