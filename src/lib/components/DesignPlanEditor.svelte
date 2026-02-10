<script lang="ts">
  import type { PlanTemplate, DiffLine } from '$lib/types';
  import ConfidenceBadge from './ConfidenceBadge.svelte';

  interface Props {
    planText: string;
    previousPlanText: string | null;
    riskScore: number;
    warnings: string[];
    isValid: boolean;
    onApprove: (editedPlanText: string) => void;
    onReject: () => void;
    templates: PlanTemplate[];
    confidenceLevel?: 'high' | 'medium' | 'low';
    confidenceScore?: number;
    confidenceMessage?: string;
  }

  let {
    planText,
    previousPlanText,
    riskScore,
    warnings,
    isValid,
    onApprove,
    onReject,
    templates,
    confidenceLevel,
    confidenceScore,
    confidenceMessage,
  }: Props = $props();

  let editedText = $state(planText);
  let showDiff = $state(false);
  let selectedTemplate = $state('');

  // Compute diff lines when diff toggle is active
  let diffLines = $derived.by(() => {
    if (!showDiff || !previousPlanText) return [];
    return computeDiff(previousPlanText, editedText);
  });

  let hasDiff = $derived(previousPlanText != null && previousPlanText !== editedText);

  function computeDiff(oldText: string, newText: string): DiffLine[] {
    const oldLines = oldText.split('\n');
    const newLines = newText.split('\n');
    const result: DiffLine[] = [];
    const maxLen = Math.max(oldLines.length, newLines.length);

    for (let i = 0; i < maxLen; i++) {
      const oldLine = i < oldLines.length ? oldLines[i] : undefined;
      const newLine = i < newLines.length ? newLines[i] : undefined;

      if (oldLine === newLine) {
        result.push({ tag: 'equal', text: oldLine ?? '' });
      } else {
        if (oldLine !== undefined) {
          result.push({ tag: 'delete', text: oldLine });
        }
        if (newLine !== undefined) {
          result.push({ tag: 'insert', text: newLine });
        }
      }
    }
    return result;
  }

  function handleTemplateChange() {
    const tmpl = templates.find((t) => t.id === selectedTemplate);
    if (tmpl) {
      editedText = tmpl.plan_text;
    }
    selectedTemplate = '';
  }

  function handleApprove() {
    onApprove(editedText);
  }

  function riskBadgeClass(score: number): string {
    if (score <= 3) return 'risk-low';
    if (score <= 6) return 'risk-medium';
    return 'risk-high';
  }
</script>

<div class="plan-editor">
  <div class="plan-editor-header">
    <span class="plan-editor-title">Geometry Design Plan</span>
    <div class="header-badges">
      {#if confidenceLevel}
        <ConfidenceBadge level={confidenceLevel} score={confidenceScore ?? 0}
          message={confidenceMessage} compact />
      {/if}
      <span class="risk-badge {riskBadgeClass(riskScore)}">
        Risk: {riskScore}/10
      </span>
    </div>
  </div>

  <div class="plan-editor-toolbar">
    <select
      class="template-select"
      bind:value={selectedTemplate}
      onchange={handleTemplateChange}
    >
      <option value="">Load template...</option>
      {#each templates as tmpl}
        <option value={tmpl.id}>{tmpl.name}</option>
      {/each}
    </select>

    {#if hasDiff}
      <button
        class="diff-toggle-btn"
        class:active={showDiff}
        onclick={() => { showDiff = !showDiff; }}
        type="button"
      >
        {showDiff ? 'Hide Changes' : 'Show Changes'}
      </button>
    {/if}
  </div>

  {#if showDiff && diffLines.length > 0}
    <div class="diff-view">
      {#each diffLines as line}
        <div class="diff-line diff-{line.tag}">
          <span class="diff-marker">
            {#if line.tag === 'insert'}+{:else if line.tag === 'delete'}-{:else}&nbsp;{/if}
          </span>
          <span class="diff-text">{line.text}</span>
        </div>
      {/each}
    </div>
  {:else}
    <textarea
      class="plan-textarea"
      bind:value={editedText}
      spellcheck="false"
    ></textarea>
  {/if}

  {#if warnings.length > 0}
    <div class="warnings-list">
      {#each warnings as warning}
        <div class="warning-item">{warning}</div>
      {/each}
    </div>
  {/if}

  <div class="plan-editor-actions">
    <button class="btn-cancel" onclick={onReject} type="button">Cancel</button>
    <button class="btn-approve" onclick={handleApprove} type="button">
      Generate Code
    </button>
  </div>
</div>

<style>
  .plan-editor {
    margin: 8px 12px;
    border: 1px solid var(--accent);
    border-radius: 8px;
    background: var(--bg-mantle);
    overflow: hidden;
  }

  .plan-editor-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 8px 12px;
    border-bottom: 1px solid var(--border-subtle);
  }

  .header-badges {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .plan-editor-title {
    font-size: 12px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    color: var(--accent);
  }

  .risk-badge {
    font-size: 11px;
    font-weight: 600;
    padding: 2px 8px;
    border-radius: 10px;
  }

  .risk-low {
    background: rgba(166, 227, 161, 0.15);
    color: var(--success);
  }

  .risk-medium {
    background: rgba(249, 226, 175, 0.15);
    color: var(--warning);
  }

  .risk-high {
    background: rgba(243, 139, 168, 0.15);
    color: var(--error);
  }

  .plan-editor-toolbar {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 12px;
    border-bottom: 1px solid var(--border-subtle);
  }

  .template-select {
    flex: 1;
    padding: 4px 8px;
    background: var(--bg-base);
    border: 1px solid var(--border);
    border-radius: 4px;
    color: var(--text-primary);
    font-size: 12px;
    cursor: pointer;
    appearance: auto;
  }

  .diff-toggle-btn {
    background: var(--bg-overlay);
    border: 1px solid var(--border);
    border-radius: 4px;
    color: var(--text-secondary);
    padding: 4px 10px;
    font-size: 11px;
    cursor: pointer;
    white-space: nowrap;
    transition: all 0.15s ease;
  }

  .diff-toggle-btn:hover {
    color: var(--text-primary);
    border-color: var(--accent);
  }

  .diff-toggle-btn.active {
    background: var(--accent);
    color: var(--bg-base);
    border-color: var(--accent);
  }

  .plan-textarea {
    display: block;
    width: 100%;
    min-height: 200px;
    max-height: 400px;
    padding: 10px 12px;
    background: var(--bg-base);
    border: none;
    color: var(--text-primary);
    font-family: 'Fira Code', 'Consolas', 'Monaco', monospace;
    font-size: 12px;
    line-height: 1.5;
    resize: vertical;
    box-sizing: border-box;
  }

  .plan-textarea:focus {
    outline: none;
  }

  .diff-view {
    max-height: 400px;
    overflow-y: auto;
    font-family: 'Fira Code', 'Consolas', 'Monaco', monospace;
    font-size: 11px;
    line-height: 1.5;
  }

  .diff-line {
    display: flex;
    padding: 0 8px;
    white-space: pre;
  }

  .diff-marker {
    flex-shrink: 0;
    width: 16px;
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
    background: rgba(210, 15, 57, 0.12);
    color: #d20f39;
    text-decoration: line-through;
  }

  .diff-line.diff-delete .diff-marker {
    color: #d20f39;
  }

  .diff-line.diff-equal {
    color: var(--text-muted);
  }

  .warnings-list {
    padding: 6px 12px;
    border-top: 1px solid var(--border-subtle);
  }

  .warning-item {
    font-size: 11px;
    color: var(--warning);
    padding: 2px 0;
  }

  .warning-item::before {
    content: '\26A0 ';
  }

  .plan-editor-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    padding: 8px 12px;
    border-top: 1px solid var(--border-subtle);
  }

  .btn-cancel {
    background: none;
    border: 1px solid var(--border);
    border-radius: 4px;
    color: var(--text-secondary);
    padding: 5px 14px;
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .btn-cancel:hover {
    color: var(--text-primary);
    border-color: var(--text-muted);
  }

  .btn-approve {
    background: var(--accent);
    border: 1px solid var(--accent);
    border-radius: 4px;
    color: var(--bg-base);
    padding: 5px 14px;
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .btn-approve:hover {
    background: var(--accent-hover);
    border-color: var(--accent-hover);
  }
</style>
