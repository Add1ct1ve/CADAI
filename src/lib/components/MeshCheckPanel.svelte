<script lang="ts">
  import type { MeshCheckResult } from '$lib/services/tauri';

  interface Props {
    result: MeshCheckResult;
    onClose: () => void;
  }

  let { result, onClose }: Props = $props();

  let allPassed = $derived(result.issues.length === 0);
</script>

<div class="mesh-check-overlay">
  <div class="mesh-check-panel">
    <div class="panel-header">
      <span class="panel-title">Mesh Check</span>
      <button class="close-btn" onclick={onClose}>&times;</button>
    </div>

    <div class="check-status" class:status-pass={allPassed} class:status-fail={!allPassed}>
      {allPassed ? 'ALL CHECKS PASSED' : `${result.issues.length} ISSUE${result.issues.length > 1 ? 'S' : ''} FOUND`}
    </div>

    <div class="check-list">
      <div class="check-item" class:check-pass={result.watertight} class:check-fail={!result.watertight}>
        <span class="check-icon">{result.watertight ? 'Pass' : 'Fail'}</span>
        <span>Watertight</span>
      </div>
      <div class="check-item" class:check-pass={result.winding_consistent} class:check-fail={!result.winding_consistent}>
        <span class="check-icon">{result.winding_consistent ? 'Pass' : 'Fail'}</span>
        <span>Consistent normals</span>
      </div>
      <div class="check-item" class:check-pass={result.degenerate_faces === 0} class:check-fail={result.degenerate_faces > 0}>
        <span class="check-icon">{result.degenerate_faces === 0 ? 'Pass' : 'Fail'}</span>
        <span>No degenerate faces</span>
      </div>
      <div class="check-item" class:check-pass={result.euler_number === 2} class:check-fail={result.euler_number !== 2}>
        <span class="check-icon">{result.euler_number === 2 ? 'Pass' : 'Fail'}</span>
        <span>Euler number = {result.euler_number}</span>
      </div>
    </div>

    <div class="stats-section">
      <div class="stat-row">
        <span class="stat-label">Triangles</span>
        <span class="stat-value">{result.triangle_count.toLocaleString()}</span>
      </div>
      <div class="stat-row">
        <span class="stat-label">Volume</span>
        <span class="stat-value">{result.volume.toFixed(2)} mm&sup3;</span>
      </div>
    </div>

    {#if result.issues.length > 0}
      <div class="issues-section">
        <div class="issues-title">Issues</div>
        {#each result.issues as issue}
          <div class="issue-item">{issue}</div>
        {/each}
      </div>
    {/if}
  </div>
</div>

<style>
  .mesh-check-overlay {
    position: absolute;
    top: 44px;
    left: 50%;
    transform: translateX(-50%);
    z-index: 25;
  }

  .mesh-check-panel {
    background: var(--bg-mantle);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 12px 16px;
    min-width: 260px;
    max-width: 340px;
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

  .check-status {
    font-size: 11px;
    font-weight: 700;
    text-align: center;
    padding: 4px 8px;
    border-radius: 3px;
    margin-bottom: 8px;
    letter-spacing: 0.5px;
  }

  .status-pass {
    color: #a6e3a1;
    background: rgba(166, 227, 161, 0.1);
    border: 1px solid rgba(166, 227, 161, 0.3);
  }

  .status-fail {
    color: #f38ba8;
    background: rgba(243, 139, 168, 0.1);
    border: 1px solid rgba(243, 139, 168, 0.3);
  }

  .check-list {
    display: flex;
    flex-direction: column;
    gap: 4px;
    margin-bottom: 8px;
  }

  .check-item {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12px;
    padding: 3px 6px;
    border-radius: 3px;
  }

  .check-pass {
    color: #a6e3a1;
  }

  .check-fail {
    color: #f38ba8;
  }

  .check-icon {
    font-size: 10px;
    font-weight: 700;
    text-transform: uppercase;
    min-width: 28px;
  }

  .stats-section {
    border-top: 1px solid var(--border-subtle);
    padding-top: 6px;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .stat-row {
    display: flex;
    justify-content: space-between;
    font-size: 11px;
  }

  .stat-label {
    color: var(--text-muted);
  }

  .stat-value {
    color: var(--text-primary);
    font-family: var(--font-mono);
  }

  .issues-section {
    border-top: 1px solid var(--border-subtle);
    padding-top: 6px;
    margin-top: 6px;
  }

  .issues-title {
    font-size: 10px;
    font-weight: 700;
    color: #f38ba8;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    margin-bottom: 4px;
  }

  .issue-item {
    font-size: 11px;
    color: var(--text-secondary);
    padding: 2px 0;
    padding-left: 8px;
    border-left: 2px solid rgba(243, 139, 168, 0.3);
    margin-bottom: 2px;
  }

  @keyframes fadeIn {
    from { opacity: 0; transform: translateY(-4px); }
    to { opacity: 1; transform: translateY(0); }
  }
</style>
