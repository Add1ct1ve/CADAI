<script lang="ts">
  import type { ChatMessage } from '$lib/types';
  import { getProjectStore } from '$lib/stores/project.svelte';
  import { getViewportStore } from '$lib/stores/viewport.svelte';
  import { executeCode } from '$lib/services/tauri';

  interface Props {
    message: ChatMessage;
    onRetry?: (failedCode: string, errorMessage: string) => void;
    onExplainError?: (errorMessage: string, failedCode: string) => void;
    disableActions?: boolean;
  }

  let { message, onRetry, onExplainError, disableActions = false }: Props = $props();

  const project = getProjectStore();
  const viewportStore = getViewportStore();

  let copiedIndex = $state<number | null>(null);

  let formattedTime = $derived(() => {
    const d = new Date(message.timestamp);
    return d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
  });

  /**
   * Simple code block detection: split content into text and code segments
   */
  function parseContent(content: string): Array<{ type: 'text' | 'code'; value: string }> {
    const parts: Array<{ type: 'text' | 'code'; value: string }> = [];
    const regex = /```[\w]*\n?([\s\S]*?)```/g;
    let lastIndex = 0;
    let match: RegExpExecArray | null;

    while ((match = regex.exec(content)) !== null) {
      if (match.index > lastIndex) {
        parts.push({ type: 'text', value: content.slice(lastIndex, match.index) });
      }
      parts.push({ type: 'code', value: match[1].trim() });
      lastIndex = match.index + match[0].length;
    }

    if (lastIndex < content.length) {
      parts.push({ type: 'text', value: content.slice(lastIndex) });
    }

    if (parts.length === 0) {
      parts.push({ type: 'text', value: content });
    }

    return parts;
  }

  let contentParts = $derived(parseContent(message.content));

  /** Whether this message is an error message that supports retry/explain actions. */
  let isActionableError = $derived(
    message.isError && message.failedCode && message.errorMessage
  );

  async function copyCode(code: string, index: number) {
    try {
      await navigator.clipboard.writeText(code);
      copiedIndex = index;
      setTimeout(() => {
        copiedIndex = null;
      }, 2000);
    } catch {
      // Fallback: use textarea trick
      const textarea = document.createElement('textarea');
      textarea.value = code;
      document.body.appendChild(textarea);
      textarea.select();
      document.execCommand('copy');
      document.body.removeChild(textarea);
      copiedIndex = index;
      setTimeout(() => {
        copiedIndex = null;
      }, 2000);
    }
  }

  async function runCode(code: string) {
    // Set the code in the editor
    project.setCode(code);

    // Execute the code
    try {
      const result = await executeCode(code);
      if (result.success && result.stl_base64) {
        viewportStore.setPendingStl(result.stl_base64);
      }
    } catch (err) {
      console.error('Run code failed:', err);
    }
  }

  function handleRetryClick() {
    if (onRetry && message.failedCode && message.errorMessage) {
      onRetry(message.failedCode, message.errorMessage);
    }
  }

  function handleExplainClick() {
    if (onExplainError && message.errorMessage && message.failedCode) {
      onExplainError(message.errorMessage, message.failedCode);
    }
  }
</script>

<div class="chat-message {message.role}" class:error-message={message.isError} class:code-updated={message.codeUpdatedByAi}>
  <div class="message-header">
    <span class="role-badge {message.role}" class:error-badge={message.isError}>
      {#if message.role === 'user'}
        You
      {:else if message.role === 'assistant'}
        AI
      {:else if message.isError}
        Err
      {:else if message.codeUpdatedByAi}
        Fix
      {:else}
        Sys
      {/if}
    </span>
    <span class="timestamp">{formattedTime()}</span>
    {#if message.retryAttempt}
      <span class="retry-badge">Retry {message.retryAttempt}/3</span>
    {/if}
  </div>
  <div class="message-body">
    {#each contentParts as part, i}
      {#if part.type === 'code'}
        <div class="code-block-wrapper">
          <div class="code-block-header">
            <span class="code-lang">python</span>
            <div class="code-block-actions">
              <button class="code-action-btn" onclick={() => copyCode(part.value, i)}>
                {copiedIndex === i ? 'Copied!' : 'Copy'}
              </button>
              <button class="code-action-btn run-btn" onclick={() => runCode(part.value)}>
                Run
              </button>
            </div>
          </div>
          <pre class="code-block"><code>{part.value}</code></pre>
        </div>
      {:else}
        <p class="text-content">{part.value}</p>
      {/if}
    {/each}
  </div>

  {#if isActionableError}
    <div class="error-actions">
      <button
        class="error-action-btn retry-btn"
        onclick={handleRetryClick}
        disabled={disableActions}
        title="Ask AI to fix the code"
      >
        Retry
      </button>
      <button
        class="error-action-btn explain-btn"
        onclick={handleExplainClick}
        disabled={disableActions}
        title="Ask AI to explain the error"
      >
        Explain Error
      </button>
    </div>
  {/if}
</div>

<style>
  .chat-message {
    padding: 8px 12px;
    border-bottom: 1px solid var(--border-subtle);
  }

  .chat-message.user {
    background: var(--bg-surface);
  }

  .chat-message.assistant {
    background: var(--bg-overlay);
  }

  .chat-message.system {
    background: var(--bg-mantle);
    font-style: italic;
    opacity: 0.8;
  }

  .chat-message.error-message {
    background: rgba(243, 139, 168, 0.08);
    border-left: 3px solid var(--error, #f38ba8);
    font-style: normal;
    opacity: 1;
  }

  .chat-message.code-updated {
    background: rgba(166, 227, 161, 0.08);
    border-left: 3px solid var(--success, #a6e3a1);
    font-style: normal;
    opacity: 1;
  }

  .message-header {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 4px;
  }

  .role-badge {
    font-size: 10px;
    font-weight: 700;
    text-transform: uppercase;
    padding: 1px 6px;
    border-radius: 3px;
    letter-spacing: 0.5px;
  }

  .role-badge.user {
    background: rgba(137, 180, 250, 0.2);
    color: var(--accent);
  }

  .role-badge.assistant {
    background: rgba(166, 227, 161, 0.2);
    color: var(--success);
  }

  .role-badge.system {
    background: rgba(108, 112, 134, 0.2);
    color: var(--text-muted);
  }

  .role-badge.error-badge {
    background: rgba(243, 139, 168, 0.2);
    color: var(--error, #f38ba8);
  }

  .retry-badge {
    font-size: 9px;
    font-weight: 600;
    padding: 1px 5px;
    border-radius: 3px;
    background: rgba(250, 179, 135, 0.2);
    color: var(--warning, #fab387);
    text-transform: uppercase;
    letter-spacing: 0.3px;
  }

  .timestamp {
    font-size: 10px;
    color: var(--text-muted);
  }

  .message-body {
    font-size: 13px;
    line-height: 1.5;
  }

  .text-content {
    margin: 2px 0;
    white-space: pre-wrap;
    word-break: break-word;
  }

  .code-block-wrapper {
    margin: 6px 0;
    border: 1px solid var(--border-subtle);
    border-radius: 4px;
    overflow: hidden;
  }

  .code-block-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 4px 10px;
    background: var(--bg-base);
    border-bottom: 1px solid var(--border-subtle);
  }

  .code-lang {
    font-size: 10px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    color: var(--text-muted);
  }

  .code-block-actions {
    display: flex;
    gap: 4px;
  }

  .code-action-btn {
    background: none;
    border: 1px solid var(--border-subtle);
    color: var(--text-secondary);
    cursor: pointer;
    font-size: 10px;
    padding: 2px 8px;
    border-radius: 3px;
    transition: all 0.15s ease;
  }

  .code-action-btn:hover {
    color: var(--text-primary);
    background: var(--bg-overlay);
    border-color: var(--border);
  }

  .code-action-btn.run-btn {
    color: var(--success);
    border-color: var(--success);
  }

  .code-action-btn.run-btn:hover {
    background: rgba(166, 227, 161, 0.1);
  }

  .code-block {
    margin: 0;
    padding: 8px 10px;
    background: var(--bg-mantle);
    overflow-x: auto;
    font-family: var(--font-mono);
    font-size: 12px;
    line-height: 1.4;
    border-radius: 0;
    border: none;
  }

  .code-block code {
    color: var(--text-primary);
  }

  /* Error action buttons */
  .error-actions {
    display: flex;
    gap: 6px;
    margin-top: 6px;
    padding-top: 6px;
    border-top: 1px solid rgba(243, 139, 168, 0.15);
  }

  .error-action-btn {
    font-size: 11px;
    font-weight: 600;
    padding: 3px 10px;
    border-radius: 3px;
    cursor: pointer;
    transition: all 0.15s ease;
    border: 1px solid;
  }

  .error-action-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .error-action-btn.retry-btn {
    background: rgba(250, 179, 135, 0.15);
    border-color: var(--warning, #fab387);
    color: var(--warning, #fab387);
  }

  .error-action-btn.retry-btn:hover:not(:disabled) {
    background: rgba(250, 179, 135, 0.25);
  }

  .error-action-btn.explain-btn {
    background: rgba(137, 180, 250, 0.15);
    border-color: var(--accent, #89b4fa);
    color: var(--accent, #89b4fa);
  }

  .error-action-btn.explain-btn:hover:not(:disabled) {
    background: rgba(137, 180, 250, 0.25);
  }
</style>
