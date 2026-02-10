<script lang="ts">
  import type { GenerationEntry } from '$lib/types';
  import { getGenerationHistoryStore } from '$lib/stores/generationHistory.svelte';
  import { computeDiff } from '$lib/utils/diff';

  interface Props {
    onRestoreEntry: (entry: GenerationEntry) => void;
    onPreviewEntry: (entry: GenerationEntry) => void;
  }

  let { onRestoreEntry, onPreviewEntry }: Props = $props();

  const store = getGenerationHistoryStore();

  let viewMode = $state<'list' | 'detail' | 'compare'>('list');
  let selectedId = $state<string | null>(null);
  let checkedIds = $state<Set<string>>(new Set());
  let confirmDeleteId = $state<string | null>(null);

  function getSelectedEntry(): GenerationEntry | undefined {
    return selectedId ? store.getEntry(selectedId) : undefined;
  }

  function timeAgo(timestamp: number): string {
    const seconds = Math.floor((Date.now() - timestamp) / 1000);
    if (seconds < 60) return `${seconds}s ago`;
    const minutes = Math.floor(seconds / 60);
    if (minutes < 60) return `${minutes}m ago`;
    const hours = Math.floor(minutes / 60);
    return `${hours}h ago`;
  }

  function shortModel(model: string): string {
    if (model.length <= 16) return model;
    // Take last segment after last dash or slash
    const parts = model.split(/[-/]/);
    return parts[parts.length - 1] || model.slice(-12);
  }

  function truncatePrompt(text: string, max: number = 60): string {
    if (text.length <= max) return text;
    return text.slice(0, max) + '...';
  }

  function formatDuration(ms: number): string {
    if (ms < 1000) return `${ms}ms`;
    return `${(ms / 1000).toFixed(1)}s`;
  }

  function handleCheckToggle(id: string) {
    const next = new Set(checkedIds);
    if (next.has(id)) {
      next.delete(id);
    } else {
      if (next.size >= 2) {
        // Replace the oldest checked
        const first = next.values().next().value;
        if (first) next.delete(first);
      }
      next.add(id);
    }
    checkedIds = next;
  }

  function handleCompare() {
    const ids = Array.from(checkedIds);
    if (ids.length === 2) {
      store.setCompare([ids[0], ids[1]]);
      viewMode = 'compare';
    }
  }

  function handleDelete(id: string) {
    const entry = store.getEntry(id);
    if (entry?.pinned) {
      confirmDeleteId = id;
      return;
    }
    store.removeEntry(id);
    if (viewMode === 'detail') viewMode = 'list';
  }

  function handleConfirmDelete(id: string) {
    store.removeEntry(id);
    confirmDeleteId = null;
    if (viewMode === 'detail') viewMode = 'list';
  }

  function backToList() {
    viewMode = 'list';
    selectedId = null;
    checkedIds = new Set();
    store.setCompare(null);
  }
</script>

<div class="gen-history">
  {#if store.entries.length === 0}
    <div class="empty-state">
      <div class="empty-icon">&#128203;</div>
      <div class="empty-text">No generations yet</div>
      <div class="empty-hint">Generate a model to see its history here</div>
    </div>
  {:else if viewMode === 'list'}
    <!-- List View -->
    <div class="list-header">
      <span class="list-title">{store.entries.length} generation{store.entries.length !== 1 ? 's' : ''}</span>
      {#if checkedIds.size === 2}
        <button class="compare-btn" onclick={handleCompare}>Compare</button>
      {:else if checkedIds.size > 0}
        <span class="check-hint">Select 2 to compare</span>
      {/if}
    </div>
    <div class="entry-list">
      {#each store.entries as entry (entry.id)}
        <div class="entry-card" class:pinned={entry.pinned}>
          <div class="entry-row">
            <input
              type="checkbox"
              class="entry-check"
              checked={checkedIds.has(entry.id)}
              onchange={() => handleCheckToggle(entry.id)}
            />
            <button class="entry-body" onclick={() => { selectedId = entry.id; viewMode = 'detail'; }}>
              <div class="entry-prompt">{truncatePrompt(entry.userPrompt)}</div>
              <div class="entry-meta">
                <span class="entry-badge" class:success={entry.success} class:fail={!entry.success}>
                  {entry.success ? 'OK' : 'FAIL'}
                </span>
                <span class="entry-model">{shortModel(entry.model)}</span>
                <span class="entry-duration">{formatDuration(entry.durationMs)}</span>
                {#if entry.tokenUsage}
                  <span class="entry-tokens">{entry.tokenUsage.total_tokens.toLocaleString()}t</span>
                {/if}
                <span class="entry-time">{timeAgo(entry.timestamp)}</span>
              </div>
            </button>
            <button
              class="pin-btn"
              class:is-pinned={entry.pinned}
              onclick={() => store.togglePin(entry.id)}
              title={entry.pinned ? 'Unpin' : 'Pin'}
            >
              {entry.pinned ? '\u2605' : '\u2606'}
            </button>
          </div>
        </div>
      {/each}
    </div>

  {:else if viewMode === 'detail'}
    <!-- Detail View -->
    {@const entry = getSelectedEntry()}
    {#if entry}
      <div class="detail-header">
        <button class="back-btn" onclick={backToList}>&larr; Back</button>
        <span class="detail-type">{entry.generationType}</span>
      </div>
      <div class="detail-body">
        <div class="detail-prompt">{entry.userPrompt}</div>
        <div class="detail-meta-grid">
          <div class="meta-item"><span class="meta-label">Provider</span><span class="meta-value">{entry.provider}</span></div>
          <div class="meta-item"><span class="meta-label">Model</span><span class="meta-value">{entry.model}</span></div>
          <div class="meta-item"><span class="meta-label">Duration</span><span class="meta-value">{formatDuration(entry.durationMs)}</span></div>
          <div class="meta-item"><span class="meta-label">Status</span><span class="meta-value" class:text-success={entry.success} class:text-fail={!entry.success}>{entry.success ? 'Success' : 'Failed'}</span></div>
          {#if entry.tokenUsage}
            <div class="meta-item"><span class="meta-label">Tokens In</span><span class="meta-value">{entry.tokenUsage.input_tokens.toLocaleString()}</span></div>
            <div class="meta-item"><span class="meta-label">Tokens Out</span><span class="meta-value">{entry.tokenUsage.output_tokens.toLocaleString()}</span></div>
            <div class="meta-item"><span class="meta-label">Total</span><span class="meta-value">{entry.tokenUsage.total_tokens.toLocaleString()}</span></div>
            {#if entry.tokenUsage.cost_usd != null && entry.tokenUsage.cost_usd > 0}
              <div class="meta-item"><span class="meta-label">Cost</span><span class="meta-value">${entry.tokenUsage.cost_usd.toFixed(4)}</span></div>
            {/if}
          {/if}
          {#if entry.confidenceScore != null}
            <div class="meta-item"><span class="meta-label">Confidence</span><span class="meta-value">{entry.confidenceScore}% ({entry.confidenceLevel})</span></div>
          {/if}
          <div class="meta-item"><span class="meta-label">Type</span><span class="meta-value">{entry.generationType}</span></div>
          <div class="meta-item"><span class="meta-label">Retries</span><span class="meta-value">{entry.retryCount}</span></div>
        </div>

        {#if entry.error}
          <div class="detail-error">{entry.error}</div>
        {/if}

        {#if entry.code}
          <div class="detail-code-label">Code</div>
          <pre class="detail-code"><code>{entry.code}</code></pre>
        {/if}

        <div class="detail-actions">
          {#if entry.stl_base64}
            <button class="action-btn preview-btn" onclick={() => onPreviewEntry(entry)}>Preview 3D</button>
          {/if}
          <button class="action-btn restore-btn" onclick={() => onRestoreEntry(entry)}>Restore</button>
          {#if confirmDeleteId === entry.id}
            <span class="confirm-text">Pinned — delete?</span>
            <button class="action-btn delete-confirm-btn" onclick={() => handleConfirmDelete(entry.id)}>Yes</button>
            <button class="action-btn" onclick={() => confirmDeleteId = null}>No</button>
          {:else}
            <button class="action-btn delete-btn" onclick={() => handleDelete(entry.id)}>Delete</button>
          {/if}
        </div>
      </div>
    {/if}

  {:else if viewMode === 'compare'}
    <!-- Compare View -->
    {@const entryA = store.compareIds ? store.getEntry(store.compareIds[0]) : undefined}
    {@const entryB = store.compareIds ? store.getEntry(store.compareIds[1]) : undefined}
    {#if entryA && entryB}
      {@const diffLines = computeDiff(entryA.code, entryB.code)}
      <div class="detail-header">
        <button class="back-btn" onclick={backToList}>&larr; Back</button>
        <span class="detail-type">Compare</span>
      </div>
      <div class="compare-body">
        <!-- Metadata comparison -->
        <div class="compare-meta">
          <table class="compare-table">
            <thead>
              <tr><th></th><th>A</th><th>B</th></tr>
            </thead>
            <tbody>
              <tr><td>Model</td><td>{shortModel(entryA.model)}</td><td>{shortModel(entryB.model)}</td></tr>
              <tr><td>Duration</td><td>{formatDuration(entryA.durationMs)}</td><td>{formatDuration(entryB.durationMs)}</td></tr>
              <tr><td>Status</td><td class:text-success={entryA.success} class:text-fail={!entryA.success}>{entryA.success ? 'OK' : 'Fail'}</td><td class:text-success={entryB.success} class:text-fail={!entryB.success}>{entryB.success ? 'OK' : 'Fail'}</td></tr>
              {#if entryA.tokenUsage || entryB.tokenUsage}
                <tr><td>Tokens</td><td>{entryA.tokenUsage?.total_tokens.toLocaleString() ?? '-'}</td><td>{entryB.tokenUsage?.total_tokens.toLocaleString() ?? '-'}</td></tr>
              {/if}
              <tr><td>Type</td><td>{entryA.generationType}</td><td>{entryB.generationType}</td></tr>
            </tbody>
          </table>
        </div>

        <!-- Code diff -->
        <div class="compare-diff-label">Code Diff</div>
        <div class="compare-diff">
          {#each diffLines as line, i}
            <div class="diff-line diff-{line.tag}">
              <span class="diff-num">{i + 1}</span>
              <span class="diff-marker">
                {#if line.tag === 'insert'}+{:else if line.tag === 'delete'}-{:else}&nbsp;{/if}
              </span>
              <span class="diff-text">{line.text}</span>
            </div>
          {/each}
        </div>

        <div class="detail-actions">
          {#if entryA.stl_base64}
            <button class="action-btn preview-btn" onclick={() => onPreviewEntry(entryA)}>Preview A</button>
          {/if}
          {#if entryB.stl_base64}
            <button class="action-btn preview-btn" onclick={() => onPreviewEntry(entryB)}>Preview B</button>
          {/if}
        </div>
      </div>
    {/if}
  {/if}
</div>

<style>
  .gen-history {
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow: hidden;
  }

  .empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    height: 100%;
    gap: 6px;
    color: var(--text-muted);
  }

  .empty-icon {
    font-size: 24px;
    opacity: 0.4;
  }

  .empty-text {
    font-size: 12px;
    font-weight: 600;
  }

  .empty-hint {
    font-size: 10px;
    opacity: 0.6;
  }

  /* List View */
  .list-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 6px 10px;
    border-bottom: 1px solid var(--border-subtle);
    flex-shrink: 0;
  }

  .list-title {
    font-size: 11px;
    color: var(--text-muted);
    font-weight: 500;
  }

  .compare-btn {
    background: var(--accent);
    color: var(--bg-base);
    border: none;
    border-radius: 3px;
    padding: 2px 8px;
    font-size: 10px;
    font-weight: 600;
    cursor: pointer;
  }

  .compare-btn:hover {
    background: var(--accent-hover);
  }

  .check-hint {
    font-size: 10px;
    color: var(--text-muted);
    font-style: italic;
  }

  .entry-list {
    flex: 1;
    overflow-y: auto;
    min-height: 0;
  }

  .entry-card {
    border-bottom: 1px solid var(--border-subtle);
    transition: background 0.1s ease;
  }

  .entry-card:hover {
    background: var(--bg-overlay);
  }

  .entry-card.pinned {
    border-left: 2px solid var(--accent);
  }

  .entry-row {
    display: flex;
    align-items: flex-start;
    gap: 6px;
    padding: 6px 8px;
  }

  .entry-check {
    margin-top: 3px;
    flex-shrink: 0;
    cursor: pointer;
    accent-color: var(--accent);
  }

  .entry-body {
    flex: 1;
    min-width: 0;
    background: none;
    border: none;
    padding: 0;
    cursor: pointer;
    text-align: left;
    color: inherit;
  }

  .entry-prompt {
    font-size: 11px;
    color: var(--text-primary);
    line-height: 1.3;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .entry-meta {
    display: flex;
    gap: 6px;
    align-items: center;
    margin-top: 3px;
    flex-wrap: wrap;
  }

  .entry-badge {
    font-size: 9px;
    font-weight: 700;
    padding: 0 4px;
    border-radius: 3px;
    line-height: 14px;
  }

  .entry-badge.success {
    background: rgba(64, 160, 43, 0.15);
    color: #40a02b;
  }

  .entry-badge.fail {
    background: rgba(230, 69, 83, 0.15);
    color: #e64553;
  }

  .entry-model,
  .entry-duration,
  .entry-tokens,
  .entry-time {
    font-size: 10px;
    color: var(--text-muted);
  }

  .pin-btn {
    background: none;
    border: none;
    cursor: pointer;
    font-size: 14px;
    color: var(--text-muted);
    padding: 0 2px;
    line-height: 1;
    flex-shrink: 0;
    transition: color 0.1s ease;
  }

  .pin-btn:hover,
  .pin-btn.is-pinned {
    color: var(--accent);
  }

  /* Detail View */
  .detail-header {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 10px;
    border-bottom: 1px solid var(--border-subtle);
    flex-shrink: 0;
  }

  .back-btn {
    background: none;
    border: none;
    color: var(--accent);
    cursor: pointer;
    font-size: 11px;
    padding: 2px 4px;
  }

  .back-btn:hover {
    text-decoration: underline;
  }

  .detail-type {
    font-size: 10px;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  .detail-body {
    flex: 1;
    overflow-y: auto;
    min-height: 0;
    padding: 8px 10px;
  }

  .detail-prompt {
    font-size: 12px;
    color: var(--text-primary);
    line-height: 1.4;
    margin-bottom: 8px;
    padding-bottom: 8px;
    border-bottom: 1px solid var(--border-subtle);
  }

  .detail-meta-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 4px 12px;
    margin-bottom: 8px;
  }

  .meta-item {
    display: flex;
    justify-content: space-between;
    font-size: 10px;
  }

  .meta-label {
    color: var(--text-muted);
  }

  .meta-value {
    color: var(--text-secondary);
    font-weight: 500;
    text-align: right;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 120px;
  }

  .text-success { color: #40a02b; }
  .text-fail { color: #e64553; }

  .detail-error {
    font-size: 10px;
    color: #e64553;
    background: rgba(230, 69, 83, 0.08);
    padding: 6px 8px;
    border-radius: 4px;
    margin-bottom: 8px;
    max-height: 80px;
    overflow-y: auto;
  }

  .detail-code-label {
    font-size: 10px;
    color: var(--text-muted);
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    margin-bottom: 4px;
  }

  .detail-code {
    background: var(--bg-base);
    border: 1px solid var(--border-subtle);
    border-radius: 4px;
    padding: 8px;
    font-family: 'Fira Code', 'Consolas', 'Monaco', monospace;
    font-size: 10px;
    line-height: 1.5;
    max-height: 300px;
    overflow-y: auto;
    white-space: pre-wrap;
    color: var(--text-secondary);
    margin: 0 0 8px 0;
  }

  .detail-actions {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
    padding-top: 6px;
    border-top: 1px solid var(--border-subtle);
  }

  .action-btn {
    background: var(--bg-mantle);
    color: var(--text-secondary);
    border: 1px solid var(--border-subtle);
    border-radius: 3px;
    padding: 3px 8px;
    font-size: 10px;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.1s ease;
  }

  .action-btn:hover {
    background: var(--bg-overlay);
    color: var(--text-primary);
  }

  .preview-btn {
    color: var(--accent);
    border-color: var(--accent);
  }

  .preview-btn:hover {
    background: var(--accent);
    color: var(--bg-base);
  }

  .restore-btn {
    color: #40a02b;
    border-color: #40a02b;
  }

  .restore-btn:hover {
    background: #40a02b;
    color: #fff;
  }

  .delete-btn {
    color: #e64553;
    border-color: #e64553;
  }

  .delete-btn:hover {
    background: #e64553;
    color: #fff;
  }

  .delete-confirm-btn {
    background: #e64553;
    color: #fff;
    border-color: #e64553;
  }

  .confirm-text {
    font-size: 10px;
    color: #e64553;
    align-self: center;
  }

  /* Compare View */
  .compare-body {
    flex: 1;
    overflow-y: auto;
    min-height: 0;
    padding: 8px 10px;
  }

  .compare-meta {
    margin-bottom: 8px;
  }

  .compare-table {
    width: 100%;
    border-collapse: collapse;
    font-size: 10px;
  }

  .compare-table th,
  .compare-table td {
    padding: 2px 6px;
    text-align: left;
    border-bottom: 1px solid var(--border-subtle);
  }

  .compare-table th {
    color: var(--text-muted);
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.3px;
  }

  .compare-table td:first-child {
    color: var(--text-muted);
    font-weight: 500;
    width: 70px;
  }

  .compare-diff-label {
    font-size: 10px;
    color: var(--text-muted);
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    margin-bottom: 4px;
  }

  .compare-diff {
    background: var(--bg-base);
    border: 1px solid var(--border-subtle);
    border-radius: 4px;
    max-height: 400px;
    overflow-y: auto;
    font-family: 'Fira Code', 'Consolas', 'Monaco', monospace;
    font-size: 10px;
    line-height: 1.5;
    margin-bottom: 8px;
  }

  .diff-line {
    display: flex;
    padding: 0 6px;
    white-space: pre;
  }

  .diff-num {
    flex-shrink: 0;
    width: 28px;
    text-align: right;
    color: var(--text-muted);
    opacity: 0.5;
    user-select: none;
    padding-right: 6px;
  }

  .diff-marker {
    flex-shrink: 0;
    width: 14px;
    text-align: center;
    user-select: none;
    color: var(--text-muted);
  }

  .diff-text {
    flex: 1;
    min-width: 0;
  }

  .diff-line.diff-insert {
    background: rgba(64, 160, 43, 0.12);
    color: #40a02b;
  }

  .diff-line.diff-insert .diff-marker {
    color: #40a02b;
  }

  .diff-line.diff-delete {
    background: rgba(230, 69, 83, 0.12);
    color: #e64553;
  }

  .diff-line.diff-delete .diff-marker {
    color: #e64553;
  }

  .diff-line.diff-equal {
    color: var(--text-muted);
  }
</style>
