<script lang="ts">
  import { getProjectStore } from '$lib/stores/project.svelte';
  import { getViewportStore } from '$lib/stores/viewport.svelte';
  import { executeCode } from '$lib/services/tauri';

  const project = getProjectStore();
  const viewportStore = getViewportStore();

  let isRunning = $state(false);
  let output = $state('');
  let outputIsError = $state(false);
  let aiUpdateNotification = $state('');
  let previousCode = $state('');

  let lineCount = $derived(() => {
    const lines = project.code.split('\n').length;
    return Math.max(lines, 20);
  });

  let lineNumbers = $derived(() => {
    return Array.from({ length: lineCount() }, (_, i) => i + 1);
  });

  // Watch for code changes from AI (auto-retry or new generation).
  $effect(() => {
    const currentCode = project.code;
    if (previousCode && currentCode && previousCode !== currentCode && !isRunning) {
      // Code changed externally (likely by AI)
      aiUpdateNotification = 'Code updated by AI';
      setTimeout(() => {
        aiUpdateNotification = '';
      }, 4000);
    }
    previousCode = currentCode;
  });

  async function handleRun() {
    if (isRunning) return;
    isRunning = true;
    output = '';
    outputIsError = false;

    try {
      const result = await executeCode(project.code);

      // Build output text from stdout and stderr
      let outputParts: string[] = [];
      if (result.stdout) {
        outputParts.push(result.stdout);
      }
      if (result.stderr) {
        outputParts.push(`[stderr]\n${result.stderr}`);
      }

      if (result.success) {
        if (result.stl_base64) {
          viewportStore.setPendingStl(result.stl_base64);
          outputParts.push('Model generated successfully.');
        }
        output = outputParts.join('\n');
        outputIsError = false;
      } else {
        output = outputParts.length > 0
          ? outputParts.join('\n')
          : 'Execution failed with no output.';
        outputIsError = true;
      }
    } catch (err) {
      output = `Error: ${err}`;
      outputIsError = true;
    } finally {
      isRunning = false;
    }
  }

  function handleInput(e: Event) {
    const target = e.target as HTMLTextAreaElement;
    project.setCode(target.value);
  }

  function handleKeydown(e: KeyboardEvent) {
    // Handle Tab key for indentation
    if (e.key === 'Tab') {
      e.preventDefault();
      const target = e.target as HTMLTextAreaElement;
      const start = target.selectionStart;
      const end = target.selectionEnd;
      const value = target.value;
      const newValue = value.substring(0, start) + '    ' + value.substring(end);
      project.setCode(newValue);
      // Restore cursor position
      setTimeout(() => {
        target.selectionStart = target.selectionEnd = start + 4;
      }, 0);
    }
  }
</script>

<div class="code-editor">
  <div class="editor-toolbar">
    <span class="editor-title">Code Editor</span>
    <div class="toolbar-actions">
      {#if aiUpdateNotification}
        <span class="ai-notification">{aiUpdateNotification}</span>
      {/if}
      <button
        class="run-btn"
        onclick={handleRun}
        disabled={isRunning}
      >
        {isRunning ? 'Running...' : 'Run'}
      </button>
    </div>
  </div>

  <div class="editor-body">
    <div class="line-numbers" aria-hidden="true">
      {#each lineNumbers() as num}
        <span class="line-num">{num}</span>
      {/each}
    </div>
    <textarea
      class="code-textarea"
      value={project.code}
      oninput={handleInput}
      onkeydown={handleKeydown}
      spellcheck={false}
      autocomplete="off"
      autocapitalize="off"
    ></textarea>
  </div>

  {#if output}
    <div class="output-panel" class:output-error={outputIsError}>
      <div class="output-header">
        <span class="output-title" class:error-title={outputIsError}>
          {outputIsError ? 'Error' : 'Output'}
        </span>
        <button class="output-close" onclick={() => { output = ''; outputIsError = false; }}>x</button>
      </div>
      <pre class="output-content" class:error-content={outputIsError}>{output}</pre>
    </div>
  {/if}
</div>

<style>
  .code-editor {
    display: flex;
    flex-direction: column;
    height: 100%;
    background: var(--bg-surface);
  }

  .editor-toolbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 6px 12px;
    background: var(--bg-mantle);
    border-bottom: 1px solid var(--border-subtle);
    flex-shrink: 0;
  }

  .editor-title {
    font-size: 12px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    color: var(--text-secondary);
  }

  .toolbar-actions {
    display: flex;
    gap: 8px;
    align-items: center;
  }

  .ai-notification {
    font-size: 11px;
    font-weight: 600;
    color: var(--success, #a6e3a1);
    background: rgba(166, 227, 161, 0.12);
    padding: 2px 8px;
    border-radius: 3px;
    animation: fadeInOut 4s ease-in-out forwards;
  }

  @keyframes fadeInOut {
    0% { opacity: 0; }
    10% { opacity: 1; }
    80% { opacity: 1; }
    100% { opacity: 0; }
  }

  .run-btn {
    background: var(--success);
    color: var(--bg-base);
    border: none;
    border-radius: 3px;
    padding: 3px 12px;
    font-size: 11px;
    font-weight: 600;
    cursor: pointer;
    transition: opacity 0.15s ease;
  }

  .run-btn:hover:not(:disabled) {
    opacity: 0.85;
  }

  .run-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .editor-body {
    flex: 1;
    display: flex;
    overflow: hidden;
    min-height: 0;
  }

  .line-numbers {
    display: flex;
    flex-direction: column;
    padding: 10px 0;
    background: var(--bg-mantle);
    border-right: 1px solid var(--border-subtle);
    overflow: hidden;
    flex-shrink: 0;
    user-select: none;
  }

  .line-num {
    display: block;
    padding: 0 10px;
    text-align: right;
    color: var(--text-muted);
    font-family: var(--font-mono);
    font-size: 12px;
    line-height: 1.6;
    min-width: 40px;
  }

  .code-textarea {
    flex: 1;
    resize: none;
    border: none;
    outline: none;
    background: var(--bg-base);
    color: var(--text-primary);
    font-family: var(--font-mono);
    font-size: 12px;
    line-height: 1.6;
    padding: 10px 12px;
    tab-size: 4;
    white-space: pre;
    overflow: auto;
  }

  .code-textarea::placeholder {
    color: var(--text-muted);
  }

  .output-panel {
    border-top: 1px solid var(--border);
    background: var(--bg-mantle);
    max-height: 30%;
    display: flex;
    flex-direction: column;
    flex-shrink: 0;
  }

  .output-panel.output-error {
    border-top: 2px solid var(--error, #f38ba8);
    background: rgba(243, 139, 168, 0.05);
  }

  .output-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 4px 12px;
    border-bottom: 1px solid var(--border-subtle);
  }

  .output-title {
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    color: var(--text-muted);
  }

  .output-title.error-title {
    color: var(--error, #f38ba8);
  }

  .output-close {
    background: none;
    border: none;
    color: var(--text-muted);
    cursor: pointer;
    font-size: 12px;
    padding: 0 4px;
  }

  .output-close:hover {
    color: var(--text-primary);
  }

  .output-content {
    padding: 8px 12px;
    margin: 0;
    font-family: var(--font-mono);
    font-size: 12px;
    line-height: 1.5;
    color: var(--text-secondary);
    overflow: auto;
    white-space: pre-wrap;
    word-break: break-all;
  }

  .output-content.error-content {
    color: var(--error, #f38ba8);
  }
</style>
