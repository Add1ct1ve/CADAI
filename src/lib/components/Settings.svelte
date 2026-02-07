<script lang="ts">
  import { getSettingsStore } from '$lib/stores/settings.svelte';
  import { checkPython, setupPython } from '$lib/services/tauri';
  import type { PythonStatus } from '$lib/types';
  import { onMount } from 'svelte';

  interface Props {
    open: boolean;
    onClose: () => void;
  }

  let { open, onClose }: Props = $props();

  const settings = getSettingsStore();

  // Local copies of settings for editing (committed on Save)
  let provider = $state('openai');
  let model = $state('gpt-4');
  let apiKey = $state('');
  let baseUrl = $state('');
  let ollamaUrl = $state('http://localhost:11434');
  let agentPreset = $state('default');

  let showApiKey = $state(false);
  let pythonStatus = $state<PythonStatus | null>(null);
  let pythonCheckError = $state(false);
  let setupMessage = $state('');
  let isSettingUp = $state(false);

  // Sync local state from settings store whenever the modal opens
  $effect(() => {
    if (open) {
      provider = settings.config.ai_provider || 'openai';
      model = settings.config.model || 'gpt-4';
      apiKey = settings.config.api_key || '';
      showApiKey = false;
      setupMessage = '';
      refreshPython();
    }
  });

  async function refreshPython() {
    try {
      pythonCheckError = false;
      pythonStatus = await checkPython();
    } catch {
      pythonCheckError = true;
    }
  }

  async function handleSetupPython() {
    isSettingUp = true;
    setupMessage = 'Setting up Python environment...';
    try {
      const result = await setupPython();
      setupMessage = result;
      await refreshPython();
    } catch (err) {
      setupMessage = `Setup failed: ${err}`;
    } finally {
      isSettingUp = false;
    }
  }

  async function handleSave() {
    settings.update({
      ai_provider: provider,
      model,
      api_key: apiKey || null,
    });
    await settings.save();
    onClose();
  }

  function handleCancel() {
    onClose();
  }

  function handleOverlayClick() {
    onClose();
  }

  function handlePanelClick(e: MouseEvent) {
    e.stopPropagation();
  }
</script>

{#if open}
<!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
<div class="settings-overlay" onclick={handleOverlayClick} role="presentation">
  <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
  <div class="settings-panel" onclick={handlePanelClick} role="dialog" aria-modal="true" aria-label="Settings" tabindex="-1">
    <div class="settings-header">
      <h2 class="settings-title">Settings</h2>
      <button class="close-btn" onclick={onClose} title="Close">&times;</button>
    </div>

    <div class="settings-body">
      <!-- AI Provider Section -->
      <div class="settings-section">
        <h3 class="section-title">AI Provider</h3>

        <div class="form-group">
          <label class="form-label" for="provider-select">Provider</label>
          <select
            id="provider-select"
            class="form-select"
            bind:value={provider}
          >
            <option value="claude">Claude</option>
            <option value="openai">OpenAI</option>
            <option value="ollama">Ollama</option>
          </select>
        </div>

        <div class="form-group">
          <label class="form-label" for="model-input">Model</label>
          <input
            id="model-input"
            class="form-input"
            type="text"
            bind:value={model}
            placeholder={provider === 'claude' ? 'claude-sonnet-4-20250514' : provider === 'ollama' ? 'llama3' : 'gpt-4'}
          />
        </div>

        <div class="form-group">
          <label class="form-label" for="api-key-input">API Key</label>
          <div class="input-with-toggle">
            <input
              id="api-key-input"
              class="form-input"
              type={showApiKey ? 'text' : 'password'}
              bind:value={apiKey}
              placeholder="Enter your API key..."
            />
            <button
              class="toggle-btn"
              onclick={() => { showApiKey = !showApiKey; }}
              type="button"
              title={showApiKey ? 'Hide' : 'Show'}
            >
              {showApiKey ? 'Hide' : 'Show'}
            </button>
          </div>
        </div>

        {#if provider === 'openai'}
          <div class="form-group">
            <label class="form-label" for="base-url-input">OpenAI Base URL (optional)</label>
            <input
              id="base-url-input"
              class="form-input"
              type="text"
              bind:value={baseUrl}
              placeholder="https://api.openai.com/v1"
            />
          </div>
        {/if}

        {#if provider === 'ollama'}
          <div class="form-group">
            <label class="form-label" for="ollama-url-input">Ollama Base URL</label>
            <input
              id="ollama-url-input"
              class="form-input"
              type="text"
              bind:value={ollamaUrl}
              placeholder="http://localhost:11434"
            />
          </div>
        {/if}
      </div>

      <!-- Agent Rules Preset -->
      <div class="settings-section">
        <h3 class="section-title">Agent Rules</h3>
        <div class="form-group">
          <label class="form-label" for="preset-select">Preset</label>
          <select
            id="preset-select"
            class="form-select"
            bind:value={agentPreset}
          >
            <option value="default">Default</option>
            <option value="3d-printing">3D Printing</option>
            <option value="cnc">CNC</option>
          </select>
          <span class="form-hint">Affects the system prompt for CAD-specific guidance.</span>
        </div>
      </div>

      <!-- Python Environment Section -->
      <div class="settings-section">
        <h3 class="section-title">Python Environment</h3>

        <div class="python-status">
          {#if pythonCheckError}
            <div class="status-row error">
              <span class="status-dot error"></span>
              <span>Failed to check Python status</span>
            </div>
          {:else if !pythonStatus}
            <div class="status-row">
              <span class="status-dot checking"></span>
              <span>Checking Python...</span>
            </div>
          {:else}
            <div class="status-row" class:ok={pythonStatus.python_found} class:error={!pythonStatus.python_found}>
              <span class="status-dot" class:ok={pythonStatus.python_found} class:error={!pythonStatus.python_found}></span>
              <span>Python: {pythonStatus.python_found ? (pythonStatus.python_version ?? 'found') : 'Not found'}</span>
            </div>
            <div class="status-row" class:ok={pythonStatus.venv_ready} class:error={!pythonStatus.venv_ready}>
              <span class="status-dot" class:ok={pythonStatus.venv_ready} class:error={!pythonStatus.venv_ready}></span>
              <span>Virtual Environment: {pythonStatus.venv_ready ? 'Ready' : 'Not set up'}</span>
            </div>
            <div class="status-row" class:ok={pythonStatus.cadquery_installed} class:error={!pythonStatus.cadquery_installed}>
              <span class="status-dot" class:ok={pythonStatus.cadquery_installed} class:error={!pythonStatus.cadquery_installed}></span>
              <span>CadQuery: {pythonStatus.cadquery_installed ? 'Installed' : 'Not installed'}</span>
            </div>
          {/if}
        </div>

        {#if setupMessage}
          <div class="setup-message" class:error={setupMessage.startsWith('Setup failed')}>
            {setupMessage}
          </div>
        {/if}

        <button
          class="setup-btn"
          onclick={handleSetupPython}
          disabled={isSettingUp}
        >
          {isSettingUp ? 'Setting up...' : 'Setup Python Environment'}
        </button>
        <span class="form-hint">Creates a virtual environment and installs CadQuery. This may take a few minutes.</span>
      </div>
    </div>

    <div class="settings-footer">
      <button class="btn btn-cancel" onclick={handleCancel}>Cancel</button>
      <button class="btn btn-save" onclick={handleSave}>Save</button>
    </div>
  </div>
</div>
{/if}

<style>
  .settings-overlay {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background: rgba(0, 0, 0, 0.6);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
    backdrop-filter: blur(2px);
  }

  .settings-panel {
    background: var(--bg-surface);
    border: 1px solid var(--border);
    border-radius: 8px;
    width: 520px;
    max-height: 85vh;
    display: flex;
    flex-direction: column;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.4);
  }

  .settings-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 16px 20px;
    border-bottom: 1px solid var(--border-subtle);
    flex-shrink: 0;
  }

  .settings-title {
    margin: 0;
    font-size: 16px;
    font-weight: 600;
    color: var(--text-primary);
  }

  .close-btn {
    background: none;
    border: none;
    color: var(--text-muted);
    font-size: 22px;
    cursor: pointer;
    padding: 0 4px;
    line-height: 1;
    border-radius: 3px;
  }

  .close-btn:hover {
    color: var(--text-primary);
    background: var(--bg-overlay);
  }

  .settings-body {
    flex: 1;
    overflow-y: auto;
    padding: 16px 20px;
  }

  .settings-section {
    margin-bottom: 20px;
  }

  .settings-section:last-child {
    margin-bottom: 0;
  }

  .section-title {
    margin: 0 0 12px 0;
    font-size: 12px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    color: var(--accent);
  }

  .form-group {
    margin-bottom: 12px;
  }

  .form-label {
    display: block;
    margin-bottom: 4px;
    font-size: 12px;
    font-weight: 500;
    color: var(--text-secondary);
  }

  .form-input,
  .form-select {
    width: 100%;
    padding: 7px 10px;
    background: var(--bg-base);
    border: 1px solid var(--border);
    border-radius: 4px;
    color: var(--text-primary);
    font-size: 13px;
    font-family: var(--font-sans);
  }

  .form-input:focus,
  .form-select:focus {
    outline: none;
    border-color: var(--accent);
  }

  .form-select {
    cursor: pointer;
    appearance: auto;
  }

  .form-hint {
    display: block;
    margin-top: 4px;
    font-size: 11px;
    color: var(--text-muted);
  }

  .input-with-toggle {
    display: flex;
    gap: 6px;
  }

  .input-with-toggle .form-input {
    flex: 1;
  }

  .toggle-btn {
    background: var(--bg-overlay);
    border: 1px solid var(--border);
    border-radius: 4px;
    color: var(--text-secondary);
    padding: 0 10px;
    font-size: 11px;
    cursor: pointer;
    white-space: nowrap;
  }

  .toggle-btn:hover {
    color: var(--text-primary);
    border-color: var(--accent);
  }

  /* Python status */
  .python-status {
    margin-bottom: 10px;
  }

  .status-row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 4px 0;
    font-size: 12px;
    color: var(--text-secondary);
  }

  .status-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex-shrink: 0;
    background: var(--text-muted);
  }

  .status-dot.ok {
    background: var(--success);
  }

  .status-dot.error {
    background: var(--error);
  }

  .status-dot.checking {
    background: var(--warning);
    animation: blink 1s infinite;
  }

  @keyframes blink {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.3; }
  }

  .setup-message {
    padding: 8px 10px;
    margin-bottom: 10px;
    background: rgba(166, 227, 161, 0.1);
    border: 1px solid rgba(166, 227, 161, 0.2);
    border-radius: 4px;
    font-size: 12px;
    color: var(--success);
    white-space: pre-wrap;
    word-break: break-word;
  }

  .setup-message.error {
    background: rgba(243, 139, 168, 0.1);
    border-color: rgba(243, 139, 168, 0.2);
    color: var(--error);
  }

  .setup-btn {
    background: var(--bg-overlay);
    border: 1px solid var(--border);
    border-radius: 4px;
    color: var(--text-primary);
    padding: 7px 14px;
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .setup-btn:hover:not(:disabled) {
    background: var(--accent);
    color: var(--bg-base);
    border-color: var(--accent);
  }

  .setup-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  /* Footer */
  .settings-footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    padding: 12px 20px;
    border-top: 1px solid var(--border-subtle);
    flex-shrink: 0;
  }

  .btn {
    padding: 7px 18px;
    border-radius: 4px;
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
    border: 1px solid;
    transition: all 0.15s ease;
  }

  .btn-cancel {
    background: none;
    border-color: var(--border);
    color: var(--text-secondary);
  }

  .btn-cancel:hover {
    color: var(--text-primary);
    border-color: var(--text-muted);
  }

  .btn-save {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--bg-base);
  }

  .btn-save:hover {
    background: var(--accent-hover);
    border-color: var(--accent-hover);
  }
</style>
