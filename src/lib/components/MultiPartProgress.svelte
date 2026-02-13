<script lang="ts">
  import type { PartProgress } from '$lib/types';

  interface Props {
    parts: PartProgress[];
    assemblyStl?: string | null;
    isGenerating: boolean;
    onPreviewPart: (partIndex: number) => void;
    onRetryPart: (partIndex: number) => void;
    onShowAssembly: () => void;
    disableActions: boolean;
  }

  let { parts, assemblyStl = null, isGenerating, onPreviewPart, onRetryPart, onShowAssembly, disableActions }: Props = $props();

  let expandedCode = $state<number | null>(null);

  function toggleCode(index: number) {
    expandedCode = expandedCode === index ? null : index;
  }

  function statusIcon(status: PartProgress['status']): string {
    switch (status) {
      case 'pending': return '\u25CB';
      case 'generating': return '\u25CE';
      case 'complete': return '\u2713';
      case 'failed': return '\u2717';
    }
  }

  function statusLabel(status: PartProgress['status']): string {
    switch (status) {
      case 'pending': return 'Pending';
      case 'generating': return 'Generating...';
      case 'complete': return 'Complete';
      case 'failed': return 'Failed';
    }
  }

  function statusColor(status: PartProgress['status']): string {
    switch (status) {
      case 'pending': return 'var(--text-muted)';
      case 'generating': return 'var(--accent)';
      case 'complete': return '#40a02b';
      case 'failed': return '#e64553';
    }
  }

  const completed = $derived(parts.filter(p => p.status === 'complete').length);
  const failed = $derived(parts.filter(p => p.status === 'failed').length);
</script>

<div class="multi-part-progress">
  <div class="mpp-header">
    <span class="mpp-title">Multi-Part Assembly</span>
    <span class="mpp-counter">{completed}/{parts.length}</span>
    {#if failed > 0}
      <span class="mpp-failed-badge">{failed} failed</span>
    {/if}
  </div>

  <div class="mpp-cards">
    {#each parts as part, index (index)}
      <div class="mpp-card" style="border-left-color: {statusColor(part.status)}">
        <div class="mpp-card-header">
          <span class="mpp-status-icon" style="color: {statusColor(part.status)}">
            {statusIcon(part.status)}
          </span>
          <span class="mpp-part-name">{part.name}</span>
          <span class="mpp-status-label" style="color: {statusColor(part.status)}">
            {statusLabel(part.status)}
          </span>
          {#if part.status === 'generating'}
            <span class="mpp-spinner"></span>
          {/if}
        </div>

        {#if part.description}
          <div class="mpp-description">{part.description}</div>
        {/if}

        {#if part.constraints && part.constraints.length > 0}
          <ul class="mpp-constraints">
            {#each part.constraints as constraint}
              <li>{constraint}</li>
            {/each}
          </ul>
        {/if}

        {#if part.status === 'failed' && part.error}
          <div class="mpp-error">{part.error}</div>
        {/if}

        <div class="mpp-actions">
          {#if part.status === 'complete' && part.stl_base64}
            <button
              class="mpp-btn mpp-btn-preview"
              onclick={() => onPreviewPart(index)}
              disabled={disableActions}
            >
              Preview
            </button>
          {/if}
          {#if part.status === 'complete' && part.code}
            <button
              class="mpp-btn mpp-btn-code"
              onclick={() => toggleCode(index)}
              disabled={false}
            >
              {expandedCode === index ? 'Hide Code' : 'Code'}
            </button>
          {/if}
          {#if part.status === 'failed' && !isGenerating}
            <button
              class="mpp-btn mpp-btn-retry"
              onclick={() => onRetryPart(index)}
              disabled={disableActions}
            >
              Retry
            </button>
          {/if}
        </div>

        {#if expandedCode === index && part.code}
          <div class="mpp-code-viewer">
            <pre><code>{part.code}</code></pre>
          </div>
        {/if}
      </div>
    {/each}
  </div>

  {#if assemblyStl}
    <div class="mpp-footer">
      <button
        class="mpp-btn mpp-btn-assembly"
        onclick={onShowAssembly}
        disabled={disableActions}
      >
        Show Full Assembly
      </button>
    </div>
  {/if}
</div>

<style>
  .multi-part-progress {
    margin: 4px 12px;
    border: 1px solid var(--border-subtle);
    border-radius: 6px;
    background: var(--bg-mantle);
    overflow: hidden;
  }

  .mpp-header {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 10px;
    border-bottom: 1px solid var(--border-subtle);
  }

  .mpp-title {
    font-size: 11px;
    font-weight: 600;
    color: var(--text-secondary);
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  .mpp-counter {
    font-size: 11px;
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
  }

  .mpp-failed-badge {
    font-size: 10px;
    color: #e64553;
    background: rgba(230, 69, 83, 0.12);
    padding: 1px 6px;
    border-radius: 3px;
    font-weight: 600;
  }

  .mpp-cards {
    display: flex;
    flex-direction: column;
  }

  .mpp-card {
    padding: 8px 10px;
    border-left: 3px solid var(--text-muted);
    border-bottom: 1px solid var(--border-subtle);
  }

  .mpp-card:last-child {
    border-bottom: none;
  }

  .mpp-card-header {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .mpp-status-icon {
    font-size: 12px;
    flex-shrink: 0;
    width: 14px;
    text-align: center;
  }

  .mpp-part-name {
    font-size: 12px;
    font-weight: 600;
    color: var(--text-primary);
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .mpp-status-label {
    font-size: 10px;
    flex-shrink: 0;
  }

  .mpp-spinner {
    width: 10px;
    height: 10px;
    border: 2px solid var(--accent);
    border-top-color: transparent;
    border-radius: 50%;
    animation: mpp-spin 0.8s linear infinite;
    flex-shrink: 0;
  }

  @keyframes mpp-spin {
    to { transform: rotate(360deg); }
  }

  .mpp-description {
    font-size: 10px;
    color: var(--text-muted);
    margin-top: 4px;
    overflow: hidden;
    text-overflow: ellipsis;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    -webkit-box-orient: vertical;
  }

  .mpp-constraints {
    margin: 4px 0 0 16px;
    padding: 0;
    list-style: disc;
    font-size: 10px;
    color: var(--text-muted);
  }

  .mpp-constraints li {
    margin-bottom: 1px;
  }

  .mpp-error {
    margin-top: 4px;
    padding: 4px 8px;
    font-size: 10px;
    color: #e64553;
    background: rgba(230, 69, 83, 0.08);
    border-radius: 3px;
    white-space: pre-wrap;
    word-break: break-word;
  }

  .mpp-actions {
    display: flex;
    gap: 4px;
    margin-top: 6px;
  }

  .mpp-btn {
    border: none;
    border-radius: 3px;
    padding: 3px 8px;
    font-size: 10px;
    font-weight: 600;
    cursor: pointer;
    transition: opacity 0.15s ease;
  }

  .mpp-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .mpp-btn-preview {
    background: var(--accent);
    color: var(--bg-base);
  }

  .mpp-btn-preview:hover:not(:disabled) {
    opacity: 0.85;
  }

  .mpp-btn-code {
    background: var(--bg-overlay);
    color: var(--text-secondary);
    border: 1px solid var(--border-subtle);
  }

  .mpp-btn-code:hover:not(:disabled) {
    background: var(--bg-surface);
  }

  .mpp-btn-retry {
    background: #e64553;
    color: #fff;
  }

  .mpp-btn-retry:hover:not(:disabled) {
    opacity: 0.85;
  }

  .mpp-btn-assembly {
    background: var(--accent);
    color: var(--bg-base);
  }

  .mpp-btn-assembly:hover:not(:disabled) {
    opacity: 0.85;
  }

  .mpp-code-viewer {
    margin-top: 6px;
    max-height: 200px;
    overflow-y: auto;
    background: var(--bg-base);
    border: 1px solid var(--border-subtle);
    border-radius: 3px;
  }

  .mpp-code-viewer pre {
    margin: 0;
    padding: 8px;
    font-size: 11px;
    line-height: 1.4;
    font-family: 'Fira Code', 'Consolas', 'Monaco', monospace;
    white-space: pre-wrap;
    word-break: break-all;
  }

  .mpp-code-viewer code {
    color: var(--text-primary);
  }

  .mpp-footer {
    padding: 8px 10px;
    border-top: 1px solid var(--border-subtle);
    display: flex;
    justify-content: center;
  }
</style>
