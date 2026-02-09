<script lang="ts">
  import type { OrientResult } from '$lib/services/tauri';

  interface Props {
    result: OrientResult;
    onApply: (rotation: [number, number, number]) => void;
    onClose: () => void;
  }

  let { result, onApply, onClose }: Props = $props();

  let isIdentity = $derived(
    result.rotation[0] === 0 && result.rotation[1] === 0 && result.rotation[2] === 0
  );
</script>

<div class="orient-overlay">
  <div class="orient-panel">
    <div class="panel-header">
      <span class="panel-title">Print Orientation</span>
      <button class="close-btn" onclick={onClose}>&times;</button>
    </div>

    <div class="recommendation">
      {#if isIdentity}
        <span class="rec-text good">Current orientation is optimal</span>
      {:else}
        <span class="rec-text">Recommended rotation</span>
      {/if}
    </div>

    <div class="rotation-display">
      <div class="rot-axis">
        <span class="rot-label">X</span>
        <span class="rot-value">{result.rotation[0]}&deg;</span>
      </div>
      <div class="rot-axis">
        <span class="rot-label">Y</span>
        <span class="rot-value">{result.rotation[1]}&deg;</span>
      </div>
      <div class="rot-axis">
        <span class="rot-label">Z</span>
        <span class="rot-value">{result.rotation[2]}&deg;</span>
      </div>
    </div>

    <div class="metrics-section">
      <div class="metric-row">
        <span class="metric-label">Print height</span>
        <span class="metric-value">{result.height} mm</span>
      </div>
      <div class="metric-row">
        <span class="metric-label">Overhang</span>
        <span class="metric-value" class:metric-warn={result.overhang_pct > 20}>{result.overhang_pct}%</span>
      </div>
      <div class="metric-row">
        <span class="metric-label">Base area</span>
        <span class="metric-value">{result.base_area} mm&sup2;</span>
      </div>
      <div class="metric-row">
        <span class="metric-label">Candidates</span>
        <span class="metric-value">{result.candidates_evaluated}</span>
      </div>
    </div>

    <div class="panel-actions">
      {#if !isIdentity}
        <button class="apply-btn" onclick={() => onApply(result.rotation)}>Apply Rotation</button>
      {/if}
      <button class="cancel-btn" onclick={onClose}>Close</button>
    </div>
  </div>
</div>

<style>
  .orient-overlay {
    position: absolute;
    top: 44px;
    left: 50%;
    transform: translateX(-50%);
    z-index: 25;
  }

  .orient-panel {
    background: var(--bg-mantle);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 12px 16px;
    min-width: 240px;
    max-width: 300px;
    box-shadow: 0 4px 16px rgba(0, 0, 0, 0.4);
    animation: fadeIn 0.15s ease;
  }

  .panel-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 8px;
  }

  .panel-title {
    font-size: 12px;
    font-weight: 700;
    color: #f9e2af;
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  .close-btn {
    background: none;
    border: none;
    color: var(--text-muted);
    font-size: 18px;
    cursor: pointer;
    padding: 0 4px;
    line-height: 1;
  }

  .close-btn:hover {
    color: var(--text-primary);
  }

  .recommendation {
    text-align: center;
    margin-bottom: 8px;
  }

  .rec-text {
    font-size: 11px;
    color: var(--text-secondary);
  }

  .rec-text.good {
    color: #a6e3a1;
    font-weight: 600;
  }

  .rotation-display {
    display: flex;
    gap: 12px;
    justify-content: center;
    margin-bottom: 10px;
    padding: 6px 0;
    border: 1px solid var(--border-subtle);
    border-radius: 4px;
    background: var(--bg-base);
  }

  .rot-axis {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 2px;
  }

  .rot-label {
    font-size: 10px;
    color: var(--text-muted);
    font-weight: 600;
  }

  .rot-value {
    font-size: 14px;
    color: var(--text-primary);
    font-family: var(--font-mono);
    font-weight: 600;
  }

  .metrics-section {
    display: flex;
    flex-direction: column;
    gap: 3px;
    margin-bottom: 10px;
  }

  .metric-row {
    display: flex;
    justify-content: space-between;
    font-size: 11px;
  }

  .metric-label {
    color: var(--text-muted);
  }

  .metric-value {
    color: var(--text-primary);
    font-family: var(--font-mono);
  }

  .metric-value.metric-warn {
    color: #fab387;
  }

  .panel-actions {
    display: flex;
    gap: 6px;
  }

  .apply-btn {
    flex: 1;
    background: rgba(249, 226, 175, 0.15);
    border: 1px solid #f9e2af;
    color: #f9e2af;
    border-radius: 3px;
    padding: 5px 10px;
    font-size: 11px;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.12s ease;
  }

  .apply-btn:hover {
    background: rgba(249, 226, 175, 0.25);
  }

  .cancel-btn {
    background: none;
    border: 1px solid var(--border-subtle);
    color: var(--text-muted);
    border-radius: 3px;
    padding: 5px 10px;
    font-size: 11px;
    cursor: pointer;
    transition: all 0.12s ease;
  }

  .cancel-btn:hover {
    border-color: var(--text-secondary);
    color: var(--text-secondary);
  }

  @keyframes fadeIn {
    from { opacity: 0; transform: translateY(-4px); }
    to { opacity: 1; transform: translateY(0); }
  }
</style>
