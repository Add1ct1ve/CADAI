<script lang="ts">
  import { getChatStore } from '$lib/stores/chat.svelte';
  import { getProjectStore } from '$lib/stores/project.svelte';
  import { getViewportStore } from '$lib/stores/viewport.svelte';
  import { sendMessageStreaming, extractPythonCode, executeCode, autoRetry } from '$lib/services/tauri';
  import ChatMessageComponent from './ChatMessage.svelte';
  import type { ChatMessage, RustChatMessage } from '$lib/types';
  import { onMount, onDestroy } from 'svelte';

  const MAX_RETRIES = 3;

  const chatStore = getChatStore();
  const project = getProjectStore();
  const viewportStore = getViewportStore();

  let inputText = $state('');
  let messagesContainer = $state<HTMLElement | null>(null);
  let isRetrying = $state(false);

  function generateId(): string {
    return Date.now().toString(36) + Math.random().toString(36).slice(2, 7);
  }

  /**
   * Convert frontend ChatMessages to the Rust backend format (role + content only).
   */
  function toRustHistory(messages: ChatMessage[]): RustChatMessage[] {
    return messages
      .filter((m) => m.role === 'user' || m.role === 'assistant')
      .map((m) => ({ role: m.role, content: m.content }));
  }

  /**
   * Handle auto-retry when code execution fails.
   * Sends the error back to the AI for a fix, then re-executes.
   */
  function handleStop() {
    chatStore.cancelGeneration();
    isRetrying = false;
  }

  async function handleAutoRetry(failedCode: string, errorMessage: string, attempt: number) {
    if (attempt > MAX_RETRIES) return;

    const myGen = chatStore.generationId;
    isRetrying = true;

    // Add a system message indicating the retry attempt.
    chatStore.addMessage({
      id: generateId(),
      role: 'system',
      content: `Retrying... (attempt ${attempt}/${MAX_RETRIES})`,
      timestamp: Date.now(),
      retryAttempt: attempt,
    });

    // Add an empty assistant message for streaming the retry response.
    const retryAssistantMsg: ChatMessage = {
      id: generateId(),
      role: 'assistant',
      content: '',
      timestamp: Date.now(),
    };
    chatStore.addMessage(retryAssistantMsg);
    chatStore.setStreaming(true);

    let streamingContent = '';
    const rustHistory = toRustHistory(
      chatStore.messages.filter((m) => m.content.length > 0),
    );

    try {
      const result = await autoRetry(
        failedCode,
        errorMessage,
        rustHistory,
        attempt,
        (delta, _done) => {
          streamingContent += delta;
          chatStore.updateLastMessage(streamingContent);
        },
      );
      if (chatStore.generationId !== myGen) return;

      // Ensure the final message matches the full response.
      chatStore.updateLastMessage(result.ai_response);

      if (result.success && result.new_code) {
        // Update the code editor with the fixed code.
        project.setCode(result.new_code);

        // Add a notification that code was updated.
        chatStore.addMessage({
          id: generateId(),
          role: 'system',
          content: `Code updated by AI (attempt ${attempt}/${MAX_RETRIES}).`,
          timestamp: Date.now(),
          codeUpdatedByAi: true,
        });

        // Try executing the new code.
        try {
          const execResult = await executeCode(result.new_code);
          if (chatStore.generationId !== myGen) return;
          if (execResult.success && execResult.stl_base64) {
            viewportStore.setPendingStl(execResult.stl_base64);
            chatStore.addMessage({
              id: generateId(),
              role: 'system',
              content: 'Code executed successfully after retry.',
              timestamp: Date.now(),
            });
          } else if (!execResult.success) {
            const newError = execResult.stderr || 'Code execution failed';
            chatStore.addMessage({
              id: generateId(),
              role: 'system',
              content: `Execution error: ${newError}`,
              timestamp: Date.now(),
              isError: true,
              failedCode: result.new_code,
              errorMessage: newError,
              retryAttempt: attempt,
            });

            // Auto-retry again if we have attempts left.
            if (attempt < MAX_RETRIES) {
              chatStore.setStreaming(false);
              isRetrying = false;
              await handleAutoRetry(result.new_code, newError, attempt + 1);
              return;
            }
          }
        } catch (execErr) {
          const errMsg = `${execErr}`;
          chatStore.addMessage({
            id: generateId(),
            role: 'system',
            content: `Failed to execute code: ${errMsg}`,
            timestamp: Date.now(),
            isError: true,
            failedCode: result.new_code,
            errorMessage: errMsg,
            retryAttempt: attempt,
          });
        }
      } else {
        // AI didn't produce valid code.
        chatStore.addMessage({
          id: generateId(),
          role: 'system',
          content: 'AI could not produce valid code. Please try describing the fix manually.',
          timestamp: Date.now(),
          isError: true,
        });
      }
    } catch (err) {
      chatStore.addMessage({
        id: generateId(),
        role: 'system',
        content: `Retry failed: ${err}`,
        timestamp: Date.now(),
        isError: true,
      });
    } finally {
      if (chatStore.generationId === myGen) {
        chatStore.setStreaming(false);
        isRetrying = false;
      }
    }
  }

  /**
   * Handle the "Explain Error" action: send a message asking the AI to explain the error.
   */
  async function handleExplainError(errorMessage: string, failedCode: string) {
    const myGen = chatStore.generationId;
    const text = `Please explain this error and suggest how to fix it:\n\nCode:\n\`\`\`python\n${failedCode}\n\`\`\`\n\nError:\n\`\`\`\n${errorMessage}\n\`\`\``;

    const userMsg: ChatMessage = {
      id: generateId(),
      role: 'user',
      content: text,
      timestamp: Date.now(),
    };
    chatStore.addMessage(userMsg);

    const assistantMsg: ChatMessage = {
      id: generateId(),
      role: 'assistant',
      content: '',
      timestamp: Date.now(),
    };
    chatStore.addMessage(assistantMsg);

    const rustHistory = toRustHistory(
      chatStore.messages.filter((m) => m.content.length > 0),
    );

    chatStore.setStreaming(true);
    let streamingContent = '';

    try {
      const fullResponse = await sendMessageStreaming(text, rustHistory, (delta, _done) => {
        streamingContent += delta;
        chatStore.updateLastMessage(streamingContent);
      });
      if (chatStore.generationId !== myGen) return;

      chatStore.updateLastMessage(fullResponse);

      // Extract Python code if the AI provided a fix in the explanation.
      const code = extractPythonCode(fullResponse);
      if (code) {
        project.setCode(code);

        try {
          const result = await executeCode(code);
          if (chatStore.generationId !== myGen) return;
          if (result.success && result.stl_base64) {
            viewportStore.setPendingStl(result.stl_base64);
          } else if (!result.success) {
            const errorInfo = result.stderr || 'Code execution failed';
            chatStore.addMessage({
              id: generateId(),
              role: 'system',
              content: `Execution error: ${errorInfo}`,
              timestamp: Date.now(),
              isError: true,
              failedCode: code,
              errorMessage: errorInfo,
            });
          }
        } catch (execErr) {
          chatStore.addMessage({
            id: generateId(),
            role: 'system',
            content: `Failed to execute code: ${execErr}`,
            timestamp: Date.now(),
            isError: true,
          });
        }
      }
    } catch (err) {
      if (streamingContent.length === 0) {
        chatStore.updateLastMessage(`Error: ${err}`);
      }
      chatStore.addMessage({
        id: generateId(),
        role: 'system',
        content: `Error: ${err}`,
        timestamp: Date.now(),
        isError: true,
      });
    } finally {
      if (chatStore.generationId === myGen) {
        chatStore.setStreaming(false);
      }
    }
  }

  /**
   * Handle the manual "Retry" button click from a chat message.
   */
  function handleRetryFromMessage(failedCode: string, errorMessage: string) {
    if (chatStore.isStreaming || isRetrying) return;
    handleAutoRetry(failedCode, errorMessage, 1);
  }

  /**
   * Handle the "Explain Error" button click from a chat message.
   */
  function handleExplainFromMessage(errorMessage: string, failedCode: string) {
    if (chatStore.isStreaming || isRetrying) return;
    handleExplainError(errorMessage, failedCode);
  }

  async function handleSend() {
    const text = inputText.trim();
    if (!text || chatStore.isStreaming) return;

    const myGen = chatStore.generationId;

    // Add user message
    const userMsg: ChatMessage = {
      id: generateId(),
      role: 'user',
      content: text,
      timestamp: Date.now(),
    };
    chatStore.addMessage(userMsg);
    inputText = '';

    // Add empty assistant message for streaming
    const assistantMsg: ChatMessage = {
      id: generateId(),
      role: 'assistant',
      content: '',
      timestamp: Date.now(),
    };
    chatStore.addMessage(assistantMsg);

    // Build history from existing messages (excluding the empty assistant placeholder)
    const rustHistory = toRustHistory(
      chatStore.messages.filter((m) => m.content.length > 0),
    );

    chatStore.setStreaming(true);
    let streamingContent = '';

    try {
      // Stream the response
      const fullResponse = await sendMessageStreaming(text, rustHistory, (delta, _done) => {
        streamingContent += delta;
        chatStore.updateLastMessage(streamingContent);
      });
      if (chatStore.generationId !== myGen) return;

      // Ensure the final message matches the full response
      chatStore.updateLastMessage(fullResponse);

      // Extract Python code from the response
      const code = extractPythonCode(fullResponse);
      if (code) {
        // Update the code editor
        project.setCode(code);

        // Auto-execute the code
        try {
          const result = await executeCode(code);
          if (chatStore.generationId !== myGen) return;
          if (result.success && result.stl_base64) {
            viewportStore.setPendingStl(result.stl_base64);
          } else if (!result.success) {
            const errorInfo = result.stderr || 'Code execution failed';
            chatStore.addMessage({
              id: generateId(),
              role: 'system',
              content: `Execution error: ${errorInfo}`,
              timestamp: Date.now(),
              isError: true,
              failedCode: code,
              errorMessage: errorInfo,
            });

            // Auto-retry: send the error back to the AI for a fix.
            chatStore.setStreaming(false);
            await handleAutoRetry(code, errorInfo, 1);
            return;
          }
        } catch (execErr) {
          const errMsg = `${execErr}`;
          chatStore.addMessage({
            id: generateId(),
            role: 'system',
            content: `Failed to execute code: ${errMsg}`,
            timestamp: Date.now(),
            isError: true,
            failedCode: code,
            errorMessage: errMsg,
          });

          // Auto-retry on execution exception too.
          chatStore.setStreaming(false);
          await handleAutoRetry(code, errMsg, 1);
          return;
        }
      }
    } catch (err) {
      // If streaming failed and we have an empty assistant message, update it with the error
      if (streamingContent.length === 0) {
        // Remove the empty assistant message
        chatStore.updateLastMessage(`Error: ${err}`);
      }
      chatStore.addMessage({
        id: generateId(),
        role: 'system',
        content: `Error: ${err}`,
        timestamp: Date.now(),
        isError: true,
      });
    } finally {
      if (chatStore.generationId === myGen) {
        chatStore.setStreaming(false);
      }
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape' && (chatStore.isStreaming || isRetrying)) {
      handleStop();
      return;
    }
    if (e.key === 'Enter' && (e.ctrlKey || e.metaKey || !e.shiftKey)) {
      e.preventDefault();
      handleSend();
    }
  }

  // Auto-scroll to bottom when messages change
  $effect(() => {
    const _ = chatStore.messages.length;
    // Also track last message content for streaming scroll
    const lastMsg = chatStore.messages[chatStore.messages.length - 1];
    const __ = lastMsg?.content;
    if (messagesContainer) {
      // Use setTimeout to wait for DOM update
      setTimeout(() => {
        if (messagesContainer) {
          messagesContainer.scrollTop = messagesContainer.scrollHeight;
        }
      }, 0);
    }
  });

  function handleWindowKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape' && (chatStore.isStreaming || isRetrying)) {
      handleStop();
    }
  }

  onMount(() => {
    window.addEventListener('keydown', handleWindowKeydown);

    // Add welcome message
    chatStore.addMessage({
      id: generateId(),
      role: 'system',
      content: 'Welcome to CAD AI Studio. Describe what you want to build and I will generate CadQuery code for you.',
      timestamp: Date.now(),
    });
  });

  onDestroy(() => {
    window.removeEventListener('keydown', handleWindowKeydown);
  });
</script>

<div class="chat-panel">
  <div class="chat-header">
    <span class="chat-title">Chat</span>
    <button class="clear-btn" onclick={() => chatStore.clear()} title="Clear chat">
      Clear
    </button>
  </div>

  <div class="messages-list" bind:this={messagesContainer}>
    {#each chatStore.messages as message (message.id)}
      <ChatMessageComponent
        {message}
        onRetry={handleRetryFromMessage}
        onExplainError={handleExplainFromMessage}
        disableActions={chatStore.isStreaming || isRetrying}
      />
    {/each}
    {#if chatStore.isStreaming || isRetrying}
      <div class="streaming-indicator">
        <span class="dot"></span>
        <span class="dot"></span>
        <span class="dot"></span>
        {#if isRetrying}
          <span class="retry-label">AI is fixing the code...</span>
        {/if}
      </div>
    {/if}
  </div>

  <div class="chat-input-area">
    <textarea
      class="chat-input"
      placeholder="Describe what you want to build..."
      bind:value={inputText}
      onkeydown={handleKeydown}
      rows={2}
      disabled={chatStore.isStreaming || isRetrying}
    ></textarea>
    {#if chatStore.isStreaming || isRetrying}
      <button
        class="stop-btn"
        onclick={handleStop}
        title="Stop generation (Escape)"
      >
        Stop
      </button>
    {:else}
      <button
        class="send-btn"
        onclick={handleSend}
        disabled={!inputText.trim()}
        title="Send message"
      >
        Send
      </button>
    {/if}
  </div>
</div>

<style>
  .chat-panel {
    display: flex;
    flex-direction: column;
    height: 100%;
    background: var(--bg-surface);
  }

  .chat-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 6px 12px;
    background: var(--bg-mantle);
    border-bottom: 1px solid var(--border-subtle);
    flex-shrink: 0;
  }

  .chat-title {
    font-size: 12px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    color: var(--text-secondary);
  }

  .clear-btn {
    background: none;
    border: none;
    color: var(--text-muted);
    cursor: pointer;
    font-size: 11px;
    padding: 2px 6px;
    border-radius: 3px;
  }

  .clear-btn:hover {
    color: var(--text-primary);
    background: var(--bg-overlay);
  }

  .messages-list {
    flex: 1;
    overflow-y: auto;
    min-height: 0;
  }

  .streaming-indicator {
    display: flex;
    gap: 4px;
    padding: 12px 16px;
    align-items: center;
  }

  .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--accent);
    animation: pulse 1.4s ease-in-out infinite;
  }

  .dot:nth-child(2) {
    animation-delay: 0.2s;
  }

  .dot:nth-child(3) {
    animation-delay: 0.4s;
  }

  .retry-label {
    font-size: 11px;
    color: var(--text-muted);
    margin-left: 8px;
    font-style: italic;
  }

  @keyframes pulse {
    0%, 80%, 100% {
      opacity: 0.3;
      transform: scale(0.8);
    }
    40% {
      opacity: 1;
      transform: scale(1);
    }
  }

  .chat-input-area {
    display: flex;
    gap: 6px;
    padding: 8px;
    border-top: 1px solid var(--border-subtle);
    background: var(--bg-mantle);
    flex-shrink: 0;
  }

  .chat-input {
    flex: 1;
    resize: none;
    background: var(--bg-base);
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 6px 10px;
    color: var(--text-primary);
    font-size: 13px;
    font-family: var(--font-sans);
    line-height: 1.4;
  }

  .chat-input::placeholder {
    color: var(--text-muted);
  }

  .chat-input:focus {
    outline: none;
    border-color: var(--accent);
  }

  .chat-input:disabled {
    opacity: 0.6;
  }

  .send-btn {
    align-self: flex-end;
    background: var(--accent);
    color: var(--bg-base);
    border: none;
    border-radius: 4px;
    padding: 6px 14px;
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
    transition: background-color 0.15s ease;
  }

  .send-btn:hover:not(:disabled) {
    background: var(--accent-hover);
  }

  .send-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .stop-btn {
    align-self: flex-end;
    background: #e64553;
    color: #fff;
    border: none;
    border-radius: 4px;
    padding: 6px 14px;
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
    transition: background-color 0.15s ease;
  }

  .stop-btn:hover {
    background: #d13344;
  }
</style>
