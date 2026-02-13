<script lang="ts">
  import { getSettingsStore } from '$lib/stores/settings.svelte';
  import { getViewportStore } from '$lib/stores/viewport.svelte';
  import { getToolStore } from '$lib/stores/tools.svelte';
  import { getSketchStore } from '$lib/stores/sketch.svelte';
  import { checkPython, setupPython, getProviderRegistry } from '$lib/services/tauri';
  import { applyTheme } from '$lib/services/theme';
  import type { PythonStatus, ProviderInfo } from '$lib/types';
  import type { ThemeId } from '$lib/services/theme';
  import ShortcutsPanel from './ShortcutsPanel.svelte';

  interface Props {
    open: boolean;
    onClose: () => void;
  }

  let { open, onClose }: Props = $props();

  const settings = getSettingsStore();
  const viewport = getViewportStore();
  const tools = getToolStore();
  const sketchStore = getSketchStore();

  // Provider registry loaded from backend
  let registry = $state<ProviderInfo[]>([]);

  // Local copies of settings for editing (committed on Save)
  let provider = $state('claude');
  let model = $state('claude-sonnet-4-5-20250929');
  let apiKey = $state('');
  let baseUrl = $state('');
  let ollamaUrl = $state('http://localhost:11434');
  let agentPreset = $state('default');
  let enableCodeReview = $state(true);
  let enableConsensus = $state(false);
  let autoApprovePlan = $state(false);
  let generationTimeout = $state(600);

  // New settings
  let theme = $state<ThemeId>('dark');
  let displayUnits = $state<'mm' | 'inch'>('mm');
  let gridSize = $state(500);
  let gridSpacing = $state(2);
  let snapTranslateEnabled = $state(true);
  let snapTranslateValue = $state(1);
  let snapRotationEnabled = $state(true);
  let snapRotationValue = $state(15);
  let snapSketchEnabled = $state(true);
  let snapSketchValue = $state(0.5);

  let showApiKey = $state(false);
  let pythonStatus = $state<PythonStatus | null>(null);
  let pythonCheckError = $state(false);
  let setupMessage = $state('');
  let isSettingUp = $state(false);
  let shortcutsOpen = $state(false);

  // Derived: the currently selected provider info from the registry
  let currentProvider = $derived(registry.find((p) => p.id === provider));

  // Sync local state from settings store whenever the modal opens
  $effect(() => {
    if (open) {
      loadRegistry();
      provider = settings.config.ai_provider || 'claude';
      model = settings.config.model || 'claude-sonnet-4-5-20250929';
      apiKey = settings.config.api_key || '';
      baseUrl = settings.config.openai_base_url || '';
      ollamaUrl = settings.config.ollama_base_url || 'http://localhost:11434';
      agentPreset = settings.config.agent_rules_preset || 'default';
      enableCodeReview = settings.config.enable_code_review ?? true;
      enableConsensus = settings.config.enable_consensus ?? false;
      autoApprovePlan = settings.config.auto_approve_plan ?? false;
      generationTimeout = settings.config.max_generation_runtime_seconds ?? 600;
      theme = (settings.config.theme as ThemeId) || 'dark';
      displayUnits = settings.config.display_units || 'mm';
      gridSize = settings.config.grid_size ?? 500;
      gridSpacing = settings.config.grid_spacing ?? 2;
      snapTranslateEnabled = settings.config.snap_translate != null;
      snapTranslateValue = settings.config.snap_translate ?? 1;
      snapRotationEnabled = settings.config.snap_rotation != null;
      snapRotationValue = settings.config.snap_rotation ?? 15;
      snapSketchEnabled = settings.config.snap_sketch != null;
      snapSketchValue = settings.config.snap_sketch ?? 0.5;
      showApiKey = false;
      setupMessage = '';
      refreshPython();
    }
  });

  async function loadRegistry() {
    try {
      registry = await getProviderRegistry();
    } catch (err) {
      console.error('Failed to load provider registry:', err);
    }
  }

  function handleProviderChange() {
    // Auto-select the first model when switching providers
    const p = registry.find((r) => r.id === provider);
    if (p && p.models.length > 0) {
      model = p.models[0].id;
    } else if (p && p.allows_custom_model) {
      model = '';
    }
  }

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
    const snapTranslate = snapTranslateEnabled ? snapTranslateValue : null;
    const snapRotation = snapRotationEnabled ? snapRotationValue : null;
    const snapSketch = snapSketchEnabled ? snapSketchValue : null;

    settings.update({
      ai_provider: provider,
      model,
      api_key: apiKey || null,
      openai_base_url: baseUrl || null,
      ollama_base_url: ollamaUrl || null,
      agent_rules_preset: agentPreset === 'default' ? null : agentPreset,
      enable_code_review: enableCodeReview,
      enable_consensus: enableConsensus,
      auto_approve_plan: autoApprovePlan,
      max_generation_runtime_seconds: generationTimeout,
      theme,
      display_units: displayUnits,
      grid_size: gridSize,
      grid_spacing: gridSpacing,
      snap_translate: snapTranslate,
      snap_rotation: snapRotation,
      snap_sketch: snapSketch,
    });
    await settings.save();

    // Apply theme
    applyTheme(theme);
    viewport.setThemeColors(theme);

    // Apply grid
    viewport.setGridConfig(gridSize, gridSpacing);

    // Apply snap values
    tools.setTranslateSnap(snapTranslate);
    tools.setRotationSnap(snapRotation);
    sketchStore.setSketchSnap(snapSketch);

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
      <!-- General Section -->
      <div class="settings-section">
        <h3 class="section-title">General</h3>

        <div class="form-row">
          <div class="form-group half">
            <label class="form-label" for="theme-select">Theme</label>
            <select id="theme-select" class="form-select" bind:value={theme}>
              <option value="dark">Dark</option>
              <option value="light">Light</option>
            </select>
          </div>

          <div class="form-group half">
            <label class="form-label" for="units-select">Display Units</label>
            <select id="units-select" class="form-select" bind:value={displayUnits}>
              <option value="mm">Millimeters (mm)</option>
              <option value="inch">Inches (in)</option>
            </select>
          </div>
        </div>
      </div>

      <!-- Grid & Snapping Section -->
      <div class="settings-section">
        <h3 class="section-title">Grid & Snapping</h3>

        <div class="form-row">
          <div class="form-group half">
            <label class="form-label" for="grid-size-select">Grid Size</label>
            <select id="grid-size-select" class="form-select" bind:value={gridSize}>
              <option value={100}>100</option>
              <option value={200}>200</option>
              <option value={500}>500</option>
              <option value={1000}>1000</option>
            </select>
          </div>

          <div class="form-group half">
            <label class="form-label" for="grid-spacing-select">Grid Spacing</label>
            <select id="grid-spacing-select" class="form-select" bind:value={gridSpacing}>
              <option value={0.5}>0.5</option>
              <option value={1}>1</option>
              <option value={2}>2</option>
              <option value={5}>5</option>
              <option value={10}>10</option>
            </select>
          </div>
        </div>

        <div class="snap-group">
          <div class="snap-row">
            <label class="form-label-inline">
              <input type="checkbox" bind:checked={snapTranslateEnabled} />
              Translation Snap
            </label>
            <input
              class="form-input snap-input"
              type="number"
              min="0.1"
              step="0.5"
              bind:value={snapTranslateValue}
              disabled={!snapTranslateEnabled}
            />
          </div>

          <div class="snap-row">
            <label class="form-label-inline">
              <input type="checkbox" bind:checked={snapRotationEnabled} />
              Rotation Snap (&deg;)
            </label>
            <input
              class="form-input snap-input"
              type="number"
              min="1"
              step="5"
              bind:value={snapRotationValue}
              disabled={!snapRotationEnabled}
            />
          </div>

          <div class="snap-row">
            <label class="form-label-inline">
              <input type="checkbox" bind:checked={snapSketchEnabled} />
              Sketch Snap
            </label>
            <input
              class="form-input snap-input"
              type="number"
              min="0.1"
              step="0.1"
              bind:value={snapSketchValue}
              disabled={!snapSketchEnabled}
            />
          </div>
        </div>
        <span class="form-hint">3D add placement now snaps to object surfaces first, then falls back to the grid when no surface is hit.</span>

        <button
          class="shortcuts-btn"
          onclick={() => { shortcutsOpen = true; }}
          type="button"
        >
          View Keyboard Shortcuts
        </button>
      </div>

      <!-- AI Provider Section -->
      <div class="settings-section">
        <h3 class="section-title">AI Provider</h3>

        <div class="form-group">
          <label class="form-label" for="provider-select">Provider</label>
          <select
            id="provider-select"
            class="form-select"
            bind:value={provider}
            onchange={handleProviderChange}
          >
            {#each registry as p}
              <option value={p.id}>{p.display_name}</option>
            {/each}
          </select>
        </div>

        <div class="form-group">
          <label class="form-label" for="model-input">Model</label>
          {#if currentProvider?.allows_custom_model}
            <input
              id="model-input"
              class="form-input"
              type="text"
              bind:value={model}
              placeholder="e.g. llama3, codellama, mistral..."
            />
          {:else if currentProvider && currentProvider.models.length > 0}
            <select
              id="model-input"
              class="form-select"
              bind:value={model}
            >
              {#each currentProvider.models as m}
                <option value={m.id}>{m.display_name}</option>
              {/each}
            </select>
          {:else}
            <input
              id="model-input"
              class="form-input"
              type="text"
              bind:value={model}
              placeholder="Model ID"
            />
          {/if}
        </div>

        {#if currentProvider?.requires_api_key}
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
        {/if}

        {#if provider === 'openai'}
          <div class="form-group">
            <label class="form-label" for="base-url-input">Base URL (optional)</label>
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

        <div class="form-group">
          <label class="form-label-inline">
            <input
              type="checkbox"
              bind:checked={enableCodeReview}
            />
            Enable AI code review
          </label>
          <span class="form-hint">After generating code, the AI verifies it matches your request. Adds ~3s.</span>
        </div>

        <div class="form-group">
          <label class="form-label-inline">
            <input
              type="checkbox"
              bind:checked={enableConsensus}
            />
            Enable consensus mode
          </label>
          <span class="form-hint">Runs 2 generation attempts at different temperatures and picks the best result. Uses ~2x tokens.</span>
        </div>

        <div class="form-group">
          <label class="form-label-inline">
            <input
              type="checkbox"
              bind:checked={autoApprovePlan}
            />
            Auto-approve design plan
          </label>
          <span class="form-hint">Skip the plan editor and generate code immediately. Faster but no chance to review the plan.</span>
        </div>

        <div class="form-group">
          <label class="form-label" for="timeout-input">Generation timeout (seconds)</label>
          <input id="timeout-input" class="form-input" type="number"
            min="60" max="1800" step="60" bind:value={generationTimeout} />
          <span class="form-hint">Max time for multipart generation. Increase for complex assemblies.</span>
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
            <div class="status-row" class:ok={pythonStatus.build123d_installed} class:error={!pythonStatus.build123d_installed}>
              <span class="status-dot" class:ok={pythonStatus.build123d_installed} class:error={!pythonStatus.build123d_installed}></span>
              <span>Build123d: {pythonStatus.build123d_installed ? 'Installed' : 'Not installed'}</span>
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
        <span class="form-hint">Creates a virtual environment and installs Build123d. This may take a few minutes.</span>
      </div>
    </div>

    <div class="settings-footer">
      <button class="btn btn-cancel" onclick={handleCancel}>Cancel</button>
      <button class="btn btn-save" onclick={handleSave}>Save</button>
    </div>
  </div>
</div>
{/if}

<ShortcutsPanel open={shortcutsOpen} onClose={() => { shortcutsOpen = false; }} />

<style>
  .settings-overlay {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background: rgba(0, 0, 0, 0.55);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
    backdrop-filter: blur(4px);
  }

  .settings-panel {
    background: var(--bg-surface);
    border: 1px solid var(--border-subtle);
    border-radius: 10px;
    width: 520px;
    max-height: 85vh;
    display: flex;
    flex-direction: column;
    box-shadow: 0 16px 48px rgba(0, 0, 0, 0.5), 0 0 0 1px rgba(255, 255, 255, 0.03);
  }

  .settings-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 18px 22px 14px;
    border-bottom: 1px solid var(--border-subtle);
    flex-shrink: 0;
  }

  .settings-title {
    margin: 0;
    font-size: 15px;
    font-weight: 600;
    color: var(--text-primary);
    letter-spacing: -0.01em;
  }

  .close-btn {
    background: none;
    border: none;
    color: var(--text-muted);
    font-size: 20px;
    cursor: pointer;
    padding: 2px 6px;
    line-height: 1;
    border-radius: 4px;
    transition: all 0.12s ease;
  }

  .close-btn:hover {
    color: var(--text-primary);
    background: var(--bg-overlay);
  }

  .settings-body {
    flex: 1;
    overflow-y: auto;
    padding: 6px 22px 16px;
  }

  .settings-section {
    margin-bottom: 6px;
    padding-top: 14px;
  }

  .settings-section:first-child {
    padding-top: 10px;
  }

  .settings-section:last-child {
    margin-bottom: 0;
  }

  .section-title {
    margin: 0 0 10px 0;
    font-size: 10px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--text-muted);
    padding-bottom: 6px;
    border-bottom: 1px solid var(--border-subtle);
  }

  .form-group {
    margin-bottom: 10px;
  }

  .form-row {
    display: flex;
    gap: 10px;
    margin-bottom: 10px;
  }

  .form-group.half {
    flex: 1;
    margin-bottom: 0;
  }

  .form-label {
    display: block;
    margin-bottom: 4px;
    font-size: 11.5px;
    font-weight: 500;
    color: var(--text-secondary);
  }

  .form-input,
  .form-select {
    width: 100%;
    padding: 6px 10px;
    background: var(--bg-base);
    border: 1px solid var(--border-subtle);
    border-radius: 5px;
    color: var(--text-primary);
    font-size: 12.5px;
    font-family: var(--font-sans);
    transition: border-color 0.12s ease;
  }

  .form-input:hover,
  .form-select:hover {
    border-color: var(--border);
  }

  .form-input:focus,
  .form-select:focus {
    outline: none;
    border-color: var(--accent);
    box-shadow: 0 0 0 2px rgba(137, 180, 250, 0.1);
  }

  /* Custom select — kill native appearance, add clean chevron */
  .form-select {
    cursor: pointer;
    appearance: none;
    -webkit-appearance: none;
    -moz-appearance: none;
    padding-right: 30px;
    background-image: url("data:image/svg+xml,%3Csvg width='10' height='6' viewBox='0 0 10 6' fill='none' xmlns='http://www.w3.org/2000/svg'%3E%3Cpath d='M1 1L5 5L9 1' stroke='%236c7086' stroke-width='1.5' stroke-linecap='round' stroke-linejoin='round'/%3E%3C/svg%3E");
    background-repeat: no-repeat;
    background-position: right 10px center;
  }

  /* Custom number input — hide native spinners */
  .form-input[type="number"] {
    -moz-appearance: textfield;
  }

  .form-input[type="number"]::-webkit-inner-spin-button,
  .form-input[type="number"]::-webkit-outer-spin-button {
    -webkit-appearance: none;
    margin: 0;
  }

  .form-hint {
    display: block;
    margin-top: 4px;
    font-size: 10.5px;
    color: var(--text-muted);
    line-height: 1.4;
  }

  .form-label-inline {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12.5px;
    color: var(--text-primary);
    cursor: pointer;
  }

  /* Custom checkbox */
  .form-label-inline input[type="checkbox"] {
    appearance: none;
    -webkit-appearance: none;
    width: 15px;
    height: 15px;
    border: 1.5px solid var(--border);
    border-radius: 3px;
    background: var(--bg-base);
    cursor: pointer;
    flex-shrink: 0;
    position: relative;
    transition: all 0.12s ease;
  }

  .form-label-inline input[type="checkbox"]:hover {
    border-color: var(--accent);
  }

  .form-label-inline input[type="checkbox"]:checked {
    background: var(--accent);
    border-color: var(--accent);
  }

  .form-label-inline input[type="checkbox"]:checked::after {
    content: '';
    position: absolute;
    left: 4px;
    top: 1px;
    width: 5px;
    height: 9px;
    border: solid var(--bg-base);
    border-width: 0 1.5px 1.5px 0;
    transform: rotate(45deg);
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
    border: 1px solid var(--border-subtle);
    border-radius: 5px;
    color: var(--text-muted);
    padding: 0 10px;
    font-size: 11px;
    font-weight: 500;
    cursor: pointer;
    white-space: nowrap;
    transition: all 0.12s ease;
  }

  .toggle-btn:hover {
    color: var(--text-primary);
    border-color: var(--border);
    background: var(--bg-base);
  }

  /* Snap controls */
  .snap-group {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin-bottom: 10px;
  }

  .snap-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 4px 0;
  }

  .snap-input {
    width: 72px !important;
    text-align: center;
    font-variant-numeric: tabular-nums;
  }

  .snap-input:disabled {
    opacity: 0.3;
  }

  .shortcuts-btn {
    background: none;
    border: 1px solid var(--border-subtle);
    border-radius: 5px;
    color: var(--text-muted);
    padding: 6px 12px;
    font-size: 11.5px;
    font-weight: 500;
    cursor: pointer;
    transition: all 0.12s ease;
    margin-top: 4px;
  }

  .shortcuts-btn:hover {
    color: var(--text-primary);
    border-color: var(--border);
    background: var(--bg-overlay);
  }

  /* Python status */
  .python-status {
    margin-bottom: 10px;
    background: var(--bg-base);
    border-radius: 5px;
    padding: 6px 10px;
    border: 1px solid var(--border-subtle);
  }

  .status-row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 3px 0;
    font-size: 11.5px;
    color: var(--text-secondary);
  }

  .status-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    flex-shrink: 0;
    background: var(--text-muted);
  }

  .status-dot.ok {
    background: var(--success);
    box-shadow: 0 0 4px rgba(166, 227, 161, 0.3);
  }

  .status-dot.error {
    background: var(--error);
    box-shadow: 0 0 4px rgba(243, 139, 168, 0.3);
  }

  .status-dot.checking {
    background: var(--warning);
    animation: pulse 1.2s ease infinite;
  }

  @keyframes pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.3; }
  }

  .setup-message {
    padding: 8px 10px;
    margin-bottom: 10px;
    background: rgba(166, 227, 161, 0.06);
    border: 1px solid rgba(166, 227, 161, 0.15);
    border-radius: 5px;
    font-size: 11.5px;
    color: var(--success);
    white-space: pre-wrap;
    word-break: break-word;
  }

  .setup-message.error {
    background: rgba(243, 139, 168, 0.06);
    border-color: rgba(243, 139, 168, 0.15);
    color: var(--error);
  }

  .setup-btn {
    background: none;
    border: 1px solid var(--border-subtle);
    border-radius: 5px;
    color: var(--text-secondary);
    padding: 6px 14px;
    font-size: 11.5px;
    font-weight: 500;
    cursor: pointer;
    transition: all 0.12s ease;
  }

  .setup-btn:hover:not(:disabled) {
    color: var(--text-primary);
    border-color: var(--border);
    background: var(--bg-overlay);
  }

  .setup-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  /* Footer */
  .settings-footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    padding: 12px 22px;
    border-top: 1px solid var(--border-subtle);
    flex-shrink: 0;
  }

  .btn {
    padding: 6px 16px;
    border-radius: 5px;
    font-size: 12px;
    font-weight: 500;
    cursor: pointer;
    border: 1px solid;
    transition: all 0.12s ease;
  }

  .btn-cancel {
    background: none;
    border-color: var(--border-subtle);
    color: var(--text-muted);
  }

  .btn-cancel:hover {
    color: var(--text-primary);
    border-color: var(--border);
    background: var(--bg-overlay);
  }

  .btn-save {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--bg-base);
    font-weight: 600;
  }

  .btn-save:hover {
    background: var(--accent-hover);
    border-color: var(--accent-hover);
  }
</style>
