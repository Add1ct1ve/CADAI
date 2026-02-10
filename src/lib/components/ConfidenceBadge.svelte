<script lang="ts">
  interface Props {
    level: 'high' | 'medium' | 'low';
    score: number;
    message?: string;
    cookbookMatches?: string[];
    compact?: boolean;
  }

  let {
    level,
    score,
    message,
    cookbookMatches,
    compact = false,
  }: Props = $props();

  let icon = $derived(level === 'high' ? '\u2714' : level === 'medium' ? '\u26A0' : '\u2716');
  let label = $derived(level === 'high' ? 'High Confidence' : level === 'medium' ? 'Medium' : 'Low');
</script>

{#if compact}
  <span class="confidence-pill confidence-{level}" title={message ?? ''}>
    <span class="confidence-icon">{icon}</span>
    <span class="confidence-label">{label}</span>
  </span>
{:else}
  <div class="confidence-block confidence-{level}">
    <div class="confidence-header">
      <span class="confidence-icon">{icon}</span>
      <span class="confidence-title">
        Confidence: {label.split(' ').pop()} ({score}/100)
      </span>
    </div>
    {#if message}
      <div class="confidence-message">{message}</div>
    {/if}
    {#if cookbookMatches && cookbookMatches.length > 0}
      <div class="confidence-matches">
        Matches: {cookbookMatches.join(', ')}
      </div>
    {/if}
  </div>
{/if}

<style>
  .confidence-pill {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 2px 8px;
    border-radius: 10px;
    font-size: 11px;
    font-weight: 600;
    cursor: default;
    white-space: nowrap;
  }

  .confidence-pill.confidence-high {
    background: rgba(166, 227, 161, 0.15);
    color: var(--success);
  }

  .confidence-pill.confidence-medium {
    background: rgba(249, 226, 175, 0.15);
    color: var(--warning);
  }

  .confidence-pill.confidence-low {
    background: rgba(243, 139, 168, 0.15);
    color: var(--error);
  }

  .confidence-icon {
    font-size: 10px;
  }

  .confidence-block {
    margin: 4px 12px;
    padding: 8px 12px;
    border-radius: 6px;
    border: 1px solid var(--border-subtle);
    font-size: 12px;
    line-height: 1.5;
  }

  .confidence-block.confidence-high {
    background: rgba(166, 227, 161, 0.08);
    border-color: var(--success);
  }

  .confidence-block.confidence-medium {
    background: rgba(249, 226, 175, 0.08);
    border-color: var(--warning);
  }

  .confidence-block.confidence-low {
    background: rgba(243, 139, 168, 0.08);
    border-color: var(--error);
  }

  .confidence-header {
    display: flex;
    align-items: center;
    gap: 6px;
    font-weight: 600;
  }

  .confidence-block.confidence-high .confidence-header {
    color: var(--success);
  }

  .confidence-block.confidence-medium .confidence-header {
    color: var(--warning);
  }

  .confidence-block.confidence-low .confidence-header {
    color: var(--error);
  }

  .confidence-message {
    color: var(--text-secondary);
    margin-top: 2px;
    font-size: 11px;
  }

  .confidence-matches {
    color: var(--text-muted);
    margin-top: 2px;
    font-size: 11px;
  }
</style>
