<script lang="ts">
  import { getChatStore } from '$lib/stores/chat.svelte';
  import { getProjectStore } from '$lib/stores/project.svelte';
  import { getViewportStore } from '$lib/stores/viewport.svelte';
  import { generateParallel, generateDesignPlan, generateFromPlan, extractPythonCode, executeCode, autoRetry, sendMessageStreaming, retrySkippedSteps } from '$lib/services/tauri';
  import { getSettingsStore } from '$lib/stores/settings.svelte';
  import ChatMessageComponent from './ChatMessage.svelte';
  import DesignPlanEditor from './DesignPlanEditor.svelte';
  import { PLAN_TEMPLATES } from '$lib/data/plan-templates';
  import type { ChatMessage, RustChatMessage, MultiPartEvent, PartProgress, IterativeStepProgress, SkippedStepInfo, TokenUsageData, DiffLine, DesignPlanResult } from '$lib/types';
  import { onMount, onDestroy } from 'svelte';

  const MAX_RETRIES = 3;

  const chatStore = getChatStore();
  const project = getProjectStore();
  const viewportStore = getViewportStore();
  const settingsStore = getSettingsStore();

  let inputText = $state('');
  let messagesContainer = $state<HTMLElement | null>(null);
  let isRetrying = $state(false);
  let partProgress = $state<PartProgress[]>([]);
  let isMultiPart = $state(false);
  let designPlanText = $state('');
  let tokenUsageSummary = $state<TokenUsageData | null>(null);
  let iterativeSteps = $state<IterativeStepProgress[]>([]);
  let isIterative = $state(false);
  let skippedStepsData = $state<SkippedStepInfo[]>([]);
  let lastDesignPlanText = $state('');
  let lastUserRequest = $state('');
  let diffData = $state<{ diff_lines: DiffLine[]; additions: number; deletions: number } | null>(null);
  let isModification = $state(false);
  let isConsensus = $state(false);
  let consensusProgress = $state<{ label: string; status: string; temperature?: number; hasCode?: boolean; executionSuccess?: boolean }[]>([]);

  // Plan editor state (two-phase flow)
  let showPlanEditor = $state(false);
  let pendingPlan = $state<DesignPlanResult | null>(null);
  let pendingUserRequest = $state('');
  let pendingHistory = $state<RustChatMessage[]>([]);

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
    isMultiPart = false;
    partProgress = [];
    designPlanText = '';
    isIterative = false;
    iterativeSteps = [];
    isModification = false;
    diffData = null;
    isConsensus = false;
    consensusProgress = [];
    showPlanEditor = false;
    pendingPlan = null;
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
      }, (usage) => {
        tokenUsageSummary = usage;
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

  async function handleRetrySkippedSteps() {
    if (chatStore.isStreaming || isRetrying || skippedStepsData.length === 0) return;

    const myGen = chatStore.generationId;
    const currentCode = project.code;
    isIterative = true;
    iterativeSteps = [];

    chatStore.addMessage({
      id: generateId(),
      role: 'system',
      content: `Retrying ${skippedStepsData.length} skipped step(s)...`,
      timestamp: Date.now(),
    });

    const assistantMsg: ChatMessage = {
      id: generateId(),
      role: 'assistant',
      content: '',
      timestamp: Date.now(),
    };
    chatStore.addMessage(assistantMsg);
    chatStore.setStreaming(true);

    let validatedStl: string | null = null;

    try {
      const result = await retrySkippedSteps(
        currentCode,
        skippedStepsData,
        lastDesignPlanText,
        lastUserRequest,
        (event: MultiPartEvent) => {
          if (chatStore.generationId !== myGen) return;

          switch (event.kind) {
            case 'IterativeStart':
              iterativeSteps = event.steps.map((s) => ({
                index: s.index,
                name: s.name,
                description: s.description,
                status: 'pending' as const,
              }));
              chatStore.updateLastMessage(formatIterativeProgress(iterativeSteps));
              break;

            case 'IterativeStepStarted':
              {
                const stepIdx = iterativeSteps.findIndex((s) => s.index === event.step_index);
                if (stepIdx >= 0) {
                  iterativeSteps[stepIdx].status = 'generating';
                  chatStore.updateLastMessage(formatIterativeProgress(iterativeSteps));
                }
              }
              break;

            case 'IterativeStepComplete':
              {
                const stepIdx = iterativeSteps.findIndex((s) => s.index === event.step_index);
                if (stepIdx >= 0) {
                  iterativeSteps[stepIdx].status = event.success ? 'complete' : 'skipped';
                  chatStore.updateLastMessage(formatIterativeProgress(iterativeSteps));
                  if (event.success && event.stl_base64) {
                    viewportStore.setPendingStl(event.stl_base64);
                  }
                }
              }
              break;

            case 'IterativeStepRetry':
              {
                const stepIdx = iterativeSteps.findIndex((s) => s.index === event.step_index);
                if (stepIdx >= 0) {
                  iterativeSteps[stepIdx].status = 'retrying';
                  iterativeSteps[stepIdx].attempt = event.attempt;
                  chatStore.updateLastMessage(formatIterativeProgress(iterativeSteps));
                }
              }
              break;

            case 'IterativeStepSkipped':
              {
                const stepIdx = iterativeSteps.findIndex((s) => s.index === event.step_index);
                if (stepIdx >= 0) {
                  iterativeSteps[stepIdx].status = 'skipped';
                  iterativeSteps[stepIdx].error = event.error;
                  chatStore.updateLastMessage(formatIterativeProgress(iterativeSteps));
                }
              }
              break;

            case 'FinalCode':
              project.setCode(event.code);
              if (event.stl_base64) validatedStl = event.stl_base64;
              break;

            case 'IterativeComplete':
              if (event.skipped_steps.length > 0) {
                skippedStepsData = event.skipped_steps;
              } else {
                skippedStepsData = [];
              }
              break;

            case 'TokenUsage':
              if (event.phase === 'total') {
                tokenUsageSummary = {
                  input_tokens: event.input_tokens,
                  output_tokens: event.output_tokens,
                  total_tokens: event.total_tokens,
                  cost_usd: event.cost_usd,
                };
              }
              break;

            case 'Done':
              break;
          }
        },
      );

      if (chatStore.generationId !== myGen) return;

      if (validatedStl) {
        viewportStore.setPendingStl(validatedStl);
      }

      if (skippedStepsData.length === 0) {
        chatStore.addMessage({
          id: generateId(),
          role: 'system',
          content: 'All previously skipped steps completed successfully!',
          timestamp: Date.now(),
        });
      } else {
        chatStore.addMessage({
          id: generateId(),
          role: 'system',
          content: `${skippedStepsData.length} step(s) still failed. You can try again or modify the request.`,
          timestamp: Date.now(),
          hasSkippedSteps: true,
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
        isIterative = false;
        iterativeSteps = [];
      }
    }
  }

  function formatPartProgress(parts: PartProgress[]): string {
    if (parts.length === 0) return '';
    const lines = parts.map((p) => {
      switch (p.status) {
        case 'pending':
          return `[ ] ${p.name}`;
        case 'generating':
          return `[...] ${p.name}`;
        case 'complete':
          return `[Done] ${p.name}`;
        case 'failed':
          return `[Failed] ${p.name}${p.error ? `: ${p.error}` : ''}`;
      }
    });
    return `Generating ${parts.length} parts in parallel:\n${lines.join('\n')}`;
  }

  function formatConsensusProgress(candidates: typeof consensusProgress): string {
    if (candidates.length === 0) return '';
    const lines = candidates.map((c) => {
      const tempStr = c.temperature != null ? ` (temp ${c.temperature})` : '';
      switch (c.status) {
        case 'pending':
          return `[ ] Candidate ${c.label}${tempStr}`;
        case 'generating':
          return `[...] Candidate ${c.label}${tempStr} generating`;
        case 'generated':
          return `[${c.hasCode ? 'code' : 'no code'}] Candidate ${c.label}${tempStr} generated`;
        case 'executing':
          return `[running] Candidate ${c.label}${tempStr} executing`;
        case 'executed':
          return `[${c.executionSuccess ? 'pass' : 'fail'}] Candidate ${c.label}${tempStr} executed`;
        default:
          return `[${c.status}] Candidate ${c.label}${tempStr}`;
      }
    });
    return `Running consensus (${candidates.length} candidates):\n${lines.join('\n')}`;
  }

  function formatIterativeProgress(steps: IterativeStepProgress[]): string {
    if (steps.length === 0) return '';
    const completed = steps.filter((s) => s.status === 'complete').length;
    const lines = steps.map((s) => {
      switch (s.status) {
        case 'pending':
          return `[ ] Step ${s.index}: ${s.description}`;
        case 'generating':
          return `[...] Step ${s.index}: ${s.description}`;
        case 'retrying':
          return `[Retry ${s.attempt ?? ''}] Step ${s.index}: ${s.description}`;
        case 'complete':
          return `[Done] Step ${s.index}: ${s.description}`;
        case 'skipped':
          return `[Skipped] Step ${s.index}: ${s.description}${s.error ? ` — ${s.error}` : ''}`;
      }
    });
    return `Building step by step (${completed}/${steps.length}):\n${lines.join('\n')}`;
  }

  /**
   * Run code generation from an approved (possibly edited) design plan.
   */
  async function runFromPlan(
    planText: string,
    userRequest: string,
    rustHistory: RustChatMessage[],
    existingCode: string | null,
  ) {
    const myGen = chatStore.generationId;
    chatStore.setStreaming(true);
    let streamingContent = '';
    let validatedStl: string | null = null;
    let backendValidated = false;

    try {
      const result = await generateFromPlan(planText, userRequest, rustHistory, (event: MultiPartEvent) => {
        if (chatStore.generationId !== myGen) return;

        switch (event.kind) {
          case 'PlanStatus':
            {
              chatStore.updateLastMessage(event.message);
            }
            break;

          case 'PlanResult':
            if (event.plan.mode === 'multi' && event.plan.parts.length > 0) {
              isMultiPart = true;
              partProgress = event.plan.parts.map((p) => ({
                name: p.name,
                status: 'pending' as const,
                streamedText: '',
              }));
              const desc = event.plan.description
                ? `${event.plan.description}\n\n`
                : '';
              chatStore.updateLastMessage(`${desc}${formatPartProgress(partProgress)}`);
            }
            break;

          case 'SingleDelta':
            streamingContent += event.delta;
            chatStore.updateLastMessage(streamingContent);
            break;

          case 'SingleDone':
            streamingContent = event.full_response;
            chatStore.updateLastMessage(streamingContent);
            break;

          case 'PartDelta':
            if (partProgress[event.part_index]) {
              partProgress[event.part_index].status = 'generating';
              partProgress[event.part_index].streamedText += event.delta;
              const desc1 = chatStore.messages[chatStore.messages.length - 1]?.content.split('\nGenerating')[0] || '';
              const prefix = desc1.includes('Generating') ? '' : desc1 + '\n';
              chatStore.updateLastMessage(`${prefix}${formatPartProgress(partProgress)}`);
            }
            break;

          case 'PartComplete':
            if (partProgress[event.part_index]) {
              partProgress[event.part_index].status = event.success ? 'complete' : 'failed';
              if (event.error) partProgress[event.part_index].error = event.error;
              const lastContent = chatStore.messages[chatStore.messages.length - 1]?.content || '';
              const descPart = lastContent.split('\nGenerating')[0];
              const prefix2 = descPart.includes('Generating') ? '' : descPart + '\n';
              chatStore.updateLastMessage(`${prefix2}${formatPartProgress(partProgress)}`);
            }
            break;

          case 'AssemblyStatus':
            {
              const lastContent2 = chatStore.messages[chatStore.messages.length - 1]?.content || '';
              chatStore.updateLastMessage(`${lastContent2}\n\n${event.message}`);
            }
            break;

          case 'FinalCode':
            project.setCode(event.code);
            if (event.stl_base64) validatedStl = event.stl_base64;
            break;

          case 'ReviewStatus':
            {
              const lastContent3 = chatStore.messages[chatStore.messages.length - 1]?.content || '';
              chatStore.updateLastMessage(`${lastContent3}\n\n${event.message}`);
            }
            break;

          case 'ReviewComplete':
            {
              const lastContent4 = chatStore.messages[chatStore.messages.length - 1]?.content || '';
              const reviewNote = event.was_modified
                ? `Code corrected by reviewer: ${event.explanation}`
                : `Code approved by reviewer.`;
              chatStore.updateLastMessage(`${lastContent4}\n${reviewNote}`);
            }
            break;

          case 'ValidationAttempt':
            {
              const lastContent5 = chatStore.messages[chatStore.messages.length - 1]?.content || '';
              chatStore.updateLastMessage(`${lastContent5}\n\n${event.message}`);
            }
            break;

          case 'ValidationSuccess':
            {
              const lastContent6 = chatStore.messages[chatStore.messages.length - 1]?.content || '';
              chatStore.updateLastMessage(`${lastContent6}\nCode validated successfully.`);
            }
            break;

          case 'ValidationFailed':
            {
              const lastContent7 = chatStore.messages[chatStore.messages.length - 1]?.content || '';
              const note = event.will_retry
                ? `Execution failed (${event.error_category}), retrying...`
                : `Execution failed: ${event.error_message}`;
              chatStore.updateLastMessage(`${lastContent7}\n${note}`);
            }
            break;

          case 'IterativeStart':
            isIterative = true;
            iterativeSteps = event.steps.map((s) => ({
              index: s.index,
              name: s.name,
              description: s.description,
              status: 'pending' as const,
            }));
            chatStore.updateLastMessage(formatIterativeProgress(iterativeSteps));
            break;

          case 'IterativeStepStarted':
            {
              const stepIdx = iterativeSteps.findIndex((s) => s.index === event.step_index);
              if (stepIdx >= 0) {
                iterativeSteps[stepIdx].status = 'generating';
                chatStore.updateLastMessage(formatIterativeProgress(iterativeSteps));
              }
            }
            break;

          case 'IterativeStepComplete':
            {
              const stepIdx = iterativeSteps.findIndex((s) => s.index === event.step_index);
              if (stepIdx >= 0) {
                iterativeSteps[stepIdx].status = event.success ? 'complete' : 'skipped';
                if (event.stl_base64) iterativeSteps[stepIdx].stl_base64 = event.stl_base64;
                chatStore.updateLastMessage(formatIterativeProgress(iterativeSteps));
                if (event.success && event.stl_base64) viewportStore.setPendingStl(event.stl_base64);
              }
            }
            break;

          case 'IterativeStepRetry':
            {
              const stepIdx = iterativeSteps.findIndex((s) => s.index === event.step_index);
              if (stepIdx >= 0) {
                iterativeSteps[stepIdx].status = 'retrying';
                iterativeSteps[stepIdx].attempt = event.attempt;
                iterativeSteps[stepIdx].error = event.error;
                chatStore.updateLastMessage(formatIterativeProgress(iterativeSteps));
              }
            }
            break;

          case 'IterativeStepSkipped':
            {
              const stepIdx = iterativeSteps.findIndex((s) => s.index === event.step_index);
              if (stepIdx >= 0) {
                iterativeSteps[stepIdx].status = 'skipped';
                iterativeSteps[stepIdx].error = event.error;
                chatStore.updateLastMessage(formatIterativeProgress(iterativeSteps));
              }
            }
            break;

          case 'IterativeComplete':
            if (event.skipped_steps.length > 0) {
              skippedStepsData = event.skipped_steps;
              const skippedNames = event.skipped_steps.map((s) => s.name).join(', ');
              chatStore.addMessage({
                id: generateId(),
                role: 'system',
                content: `Build complete with ${event.skipped_steps.length} skipped step(s): ${skippedNames}. You can retry them below.`,
                timestamp: Date.now(),
                hasSkippedSteps: true,
              });
            }
            break;

          case 'ConsensusStarted':
            isConsensus = true;
            consensusProgress = [
              { label: 'A', status: 'pending' },
              { label: 'B', status: 'pending' },
            ];
            chatStore.updateLastMessage(formatConsensusProgress(consensusProgress));
            break;

          case 'ConsensusCandidate':
            {
              const cidx = consensusProgress.findIndex((c) => c.label === event.label);
              if (cidx >= 0) {
                consensusProgress[cidx].status = event.status;
                consensusProgress[cidx].temperature = event.temperature;
                if (event.has_code != null) consensusProgress[cidx].hasCode = event.has_code;
                if (event.execution_success != null) consensusProgress[cidx].executionSuccess = event.execution_success;
                chatStore.updateLastMessage(formatConsensusProgress(consensusProgress));
              }
            }
            break;

          case 'ConsensusWinner':
            {
              const lastContent8 = chatStore.messages[chatStore.messages.length - 1]?.content || '';
              chatStore.updateLastMessage(
                `${lastContent8}\n\nWinner: Candidate ${event.label} (score ${event.score}) — ${event.reason}`
              );
            }
            break;

          case 'TokenUsage':
            if (event.phase === 'total') {
              tokenUsageSummary = {
                input_tokens: event.input_tokens,
                output_tokens: event.output_tokens,
                total_tokens: event.total_tokens,
                cost_usd: event.cost_usd,
              };
            }
            break;

          case 'Done':
            if (event.validated) backendValidated = true;
            break;
        }
      }, existingCode);

      if (chatStore.generationId !== myGen) return;

      if (isIterative && validatedStl) {
        viewportStore.setPendingStl(validatedStl);
      } else if (validatedStl) {
        viewportStore.setPendingStl(validatedStl);
      } else if (backendValidated) {
        // Backend validated — code already in editor
      } else if (isMultiPart) {
        const assembledCode = result;
        project.setCode(assembledCode);
        chatStore.updateLastMessage(
          chatStore.messages[chatStore.messages.length - 1]?.content +
            '\n\nAssembly complete! Executing code...'
        );
        try {
          const execResult = await executeCode(assembledCode);
          if (chatStore.generationId !== myGen) return;
          if (execResult.success && execResult.stl_base64) {
            viewportStore.setPendingStl(execResult.stl_base64);
          } else if (!execResult.success) {
            const errorInfo = execResult.stderr || 'Code execution failed';
            chatStore.addMessage({
              id: generateId(), role: 'system', content: `Execution error: ${errorInfo}`,
              timestamp: Date.now(), isError: true, failedCode: assembledCode, errorMessage: errorInfo,
            });
            chatStore.setStreaming(false);
            await handleAutoRetry(assembledCode, errorInfo, 1);
            return;
          }
        } catch (execErr) {
          const errMsg = `${execErr}`;
          chatStore.addMessage({
            id: generateId(), role: 'system', content: `Failed to execute code: ${errMsg}`,
            timestamp: Date.now(), isError: true, failedCode: assembledCode, errorMessage: errMsg,
          });
          chatStore.setStreaming(false);
          await handleAutoRetry(assembledCode, errMsg, 1);
          return;
        }
      } else {
        const code = extractPythonCode(result);
        if (code) {
          project.setCode(code);
          try {
            const execResult = await executeCode(code);
            if (chatStore.generationId !== myGen) return;
            if (execResult.success && execResult.stl_base64) {
              viewportStore.setPendingStl(execResult.stl_base64);
            } else if (!execResult.success) {
              const errorInfo = execResult.stderr || 'Code execution failed';
              chatStore.addMessage({
                id: generateId(), role: 'system', content: `Execution error: ${errorInfo}`,
                timestamp: Date.now(), isError: true, failedCode: code, errorMessage: errorInfo,
              });
              chatStore.setStreaming(false);
              await handleAutoRetry(code, errorInfo, 1);
              return;
            }
          } catch (execErr) {
            const errMsg = `${execErr}`;
            chatStore.addMessage({
              id: generateId(), role: 'system', content: `Failed to execute code: ${errMsg}`,
              timestamp: Date.now(), isError: true, failedCode: code, errorMessage: errMsg,
            });
            chatStore.setStreaming(false);
            await handleAutoRetry(code, errMsg, 1);
            return;
          }
        }
      }
    } catch (err) {
      if (streamingContent.length === 0 && !isMultiPart) {
        chatStore.updateLastMessage(`Error: ${err}`);
      } else {
        chatStore.addMessage({
          id: generateId(), role: 'system', content: `Error: ${err}`,
          timestamp: Date.now(), isError: true,
        });
      }
    } finally {
      if (chatStore.generationId === myGen) {
        chatStore.setStreaming(false);
        isMultiPart = false;
        partProgress = [];
        isIterative = false;
        iterativeSteps = [];
        isConsensus = false;
        consensusProgress = [];
      }
    }
  }

  function handlePlanApprove(editedPlanText: string) {
    showPlanEditor = false;
    const req = pendingUserRequest;
    const hist = pendingHistory;
    pendingPlan = null;
    runFromPlan(editedPlanText, req, hist, null);
  }

  function handlePlanReject() {
    showPlanEditor = false;
    pendingPlan = null;
    chatStore.setStreaming(false);
    chatStore.addMessage({
      id: generateId(),
      role: 'system',
      content: 'Plan generation cancelled.',
      timestamp: Date.now(),
    });
  }

  async function handleSend() {
    const text = inputText.trim();
    if (!text || chatStore.isStreaming) return;

    const myGen = chatStore.generationId;
    isMultiPart = false;
    partProgress = [];
    designPlanText = '';
    tokenUsageSummary = null;
    isIterative = false;
    iterativeSteps = [];
    skippedStepsData = [];
    isModification = false;
    diffData = null;
    isConsensus = false;
    consensusProgress = [];
    lastUserRequest = text;
    let validatedStl: string | null = null;
    let backendValidated = false;

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
    const planStartTime = Date.now();
    let planTimerInterval: ReturnType<typeof setInterval> | null = null;

    // Determine if we should send existing code for modification detection
    const DEFAULT_CODE_TEMPLATE = `import cadquery as cq\n\n# Create your 3D model here\nresult = cq.Workplane("XY").box(10, 10, 10)\n`;
    const existingCode = project.code;
    const hasExistingCode = existingCode.trim() !== DEFAULT_CODE_TEMPLATE.trim()
      && existingCode.trim().split('\n').length > 3;

    try {
      if (hasExistingCode) {
        // ── Modification path: call generateParallel directly (no plan editor) ──
        const result = await generateParallel(text, rustHistory, (event: MultiPartEvent) => {
          if (chatStore.generationId !== myGen) return;

          switch (event.kind) {
            case 'DesignPlan':
              designPlanText = event.plan_text;
              lastDesignPlanText = event.plan_text;
              break;

            case 'PlanValidation':
              if (!event.is_valid) {
                const lastContent = chatStore.messages[chatStore.messages.length - 1]?.content || '';
                chatStore.updateLastMessage(
                  `${lastContent}\n\u26A0 Plan risk score: ${event.risk_score}/10 — ${event.rejected_reason ?? 'Re-planning...'}`
                );
              }
              break;

            case 'PlanStatus':
              {
                const elapsed = Math.round((Date.now() - planStartTime) / 1000);
                chatStore.updateLastMessage(`${event.message} (${elapsed}s)`);
                if (!planTimerInterval) {
                  planTimerInterval = setInterval(() => {
                    if (chatStore.generationId !== myGen) {
                      if (planTimerInterval) clearInterval(planTimerInterval);
                      planTimerInterval = null;
                      return;
                    }
                    const el = Math.round((Date.now() - planStartTime) / 1000);
                    chatStore.updateLastMessage(`${event.message} (${el}s)`);
                  }, 1000);
                }
              }
              break;

            case 'PlanResult':
              if (planTimerInterval) {
                clearInterval(planTimerInterval);
                planTimerInterval = null;
              }
              if (event.plan.mode === 'multi' && event.plan.parts.length > 0) {
                isMultiPart = true;
                partProgress = event.plan.parts.map((p) => ({
                  name: p.name,
                  status: 'pending' as const,
                  streamedText: '',
                }));
                const desc = event.plan.description
                  ? `${event.plan.description}\n\n`
                  : '';
                chatStore.updateLastMessage(
                  `${desc}${formatPartProgress(partProgress)}`
                );
              }
              break;

            case 'SingleDelta':
              streamingContent += event.delta;
              chatStore.updateLastMessage(streamingContent);
              break;

            case 'SingleDone':
              streamingContent = event.full_response;
              chatStore.updateLastMessage(streamingContent);
              break;

            case 'PartDelta':
              if (partProgress[event.part_index]) {
                partProgress[event.part_index].status = 'generating';
                partProgress[event.part_index].streamedText += event.delta;
                const desc1 = chatStore.messages[chatStore.messages.length - 1]?.content.split('\nGenerating')[0] || '';
                const prefix = desc1.includes('Generating') ? '' : desc1 + '\n';
                chatStore.updateLastMessage(
                  `${prefix}${formatPartProgress(partProgress)}`
                );
              }
              break;

            case 'PartComplete':
              if (partProgress[event.part_index]) {
                partProgress[event.part_index].status = event.success ? 'complete' : 'failed';
                if (event.error) {
                  partProgress[event.part_index].error = event.error;
                }
                const lastContent = chatStore.messages[chatStore.messages.length - 1]?.content || '';
                const descPart = lastContent.split('\nGenerating')[0];
                const prefix2 = descPart.includes('Generating') ? '' : descPart + '\n';
                chatStore.updateLastMessage(
                  `${prefix2}${formatPartProgress(partProgress)}`
                );
              }
              break;

            case 'AssemblyStatus':
              {
                const lastContent2 = chatStore.messages[chatStore.messages.length - 1]?.content || '';
                chatStore.updateLastMessage(`${lastContent2}\n\n${event.message}`);
              }
              break;

            case 'FinalCode':
              project.setCode(event.code);
              if (event.stl_base64) validatedStl = event.stl_base64;
              break;

            case 'ReviewStatus':
              {
                const lastContent3 = chatStore.messages[chatStore.messages.length - 1]?.content || '';
                chatStore.updateLastMessage(`${lastContent3}\n\n${event.message}`);
              }
              break;

            case 'ReviewComplete':
              {
                const lastContent4 = chatStore.messages[chatStore.messages.length - 1]?.content || '';
                const reviewNote = event.was_modified
                  ? `Code corrected by reviewer: ${event.explanation}`
                  : `Code approved by reviewer.`;
                chatStore.updateLastMessage(`${lastContent4}\n${reviewNote}`);
              }
              break;

            case 'ValidationAttempt':
              {
                const lastContent5 = chatStore.messages[chatStore.messages.length - 1]?.content || '';
                chatStore.updateLastMessage(`${lastContent5}\n\n${event.message}`);
              }
              break;

            case 'ValidationSuccess':
              {
                const lastContent6 = chatStore.messages[chatStore.messages.length - 1]?.content || '';
                chatStore.updateLastMessage(`${lastContent6}\nCode validated successfully.`);
              }
              break;

            case 'ValidationFailed':
              {
                const lastContent7 = chatStore.messages[chatStore.messages.length - 1]?.content || '';
                const note = event.will_retry
                  ? `Execution failed (${event.error_category}), retrying...`
                  : `Execution failed: ${event.error_message}`;
                chatStore.updateLastMessage(`${lastContent7}\n${note}`);
              }
              break;

            case 'IterativeStart':
              isIterative = true;
              iterativeSteps = event.steps.map((s) => ({
                index: s.index,
                name: s.name,
                description: s.description,
                status: 'pending' as const,
              }));
              chatStore.updateLastMessage(formatIterativeProgress(iterativeSteps));
              break;

            case 'IterativeStepStarted':
              {
                const stepIdx = iterativeSteps.findIndex((s) => s.index === event.step_index);
                if (stepIdx >= 0) {
                  iterativeSteps[stepIdx].status = 'generating';
                  chatStore.updateLastMessage(formatIterativeProgress(iterativeSteps));
                }
              }
              break;

            case 'IterativeStepComplete':
              {
                const stepIdx = iterativeSteps.findIndex((s) => s.index === event.step_index);
                if (stepIdx >= 0) {
                  iterativeSteps[stepIdx].status = event.success ? 'complete' : 'skipped';
                  if (event.stl_base64) {
                    iterativeSteps[stepIdx].stl_base64 = event.stl_base64;
                  }
                  chatStore.updateLastMessage(formatIterativeProgress(iterativeSteps));
                  if (event.success && event.stl_base64) {
                    viewportStore.setPendingStl(event.stl_base64);
                  }
                }
              }
              break;

            case 'IterativeStepRetry':
              {
                const stepIdx = iterativeSteps.findIndex((s) => s.index === event.step_index);
                if (stepIdx >= 0) {
                  iterativeSteps[stepIdx].status = 'retrying';
                  iterativeSteps[stepIdx].attempt = event.attempt;
                  iterativeSteps[stepIdx].error = event.error;
                  chatStore.updateLastMessage(formatIterativeProgress(iterativeSteps));
                }
              }
              break;

            case 'IterativeStepSkipped':
              {
                const stepIdx = iterativeSteps.findIndex((s) => s.index === event.step_index);
                if (stepIdx >= 0) {
                  iterativeSteps[stepIdx].status = 'skipped';
                  iterativeSteps[stepIdx].error = event.error;
                  chatStore.updateLastMessage(formatIterativeProgress(iterativeSteps));
                }
              }
              break;

            case 'IterativeComplete':
              if (event.skipped_steps.length > 0) {
                skippedStepsData = event.skipped_steps;
                const skippedNames = event.skipped_steps.map((s) => s.name).join(', ');
                chatStore.addMessage({
                  id: generateId(),
                  role: 'system',
                  content: `Build complete with ${event.skipped_steps.length} skipped step(s): ${skippedNames}. You can retry them below.`,
                  timestamp: Date.now(),
                  hasSkippedSteps: true,
                });
              }
              break;

            case 'TokenUsage':
              if (event.phase === 'total') {
                tokenUsageSummary = {
                  input_tokens: event.input_tokens,
                  output_tokens: event.output_tokens,
                  total_tokens: event.total_tokens,
                  cost_usd: event.cost_usd,
                };
              }
              break;

            case 'ConsensusStarted':
              isConsensus = true;
              consensusProgress = [
                { label: 'A', status: 'pending' },
                { label: 'B', status: 'pending' },
              ];
              chatStore.updateLastMessage(formatConsensusProgress(consensusProgress));
              break;

            case 'ConsensusCandidate':
              {
                const cidx = consensusProgress.findIndex((c) => c.label === event.label);
                if (cidx >= 0) {
                  consensusProgress[cidx].status = event.status;
                  consensusProgress[cidx].temperature = event.temperature;
                  if (event.has_code != null) consensusProgress[cidx].hasCode = event.has_code;
                  if (event.execution_success != null) consensusProgress[cidx].executionSuccess = event.execution_success;
                  chatStore.updateLastMessage(formatConsensusProgress(consensusProgress));
                }
              }
              break;

            case 'ConsensusWinner':
              {
                const lastContent8 = chatStore.messages[chatStore.messages.length - 1]?.content || '';
                chatStore.updateLastMessage(
                  `${lastContent8}\n\nWinner: Candidate ${event.label} (score ${event.score}) — ${event.reason}`
                );
              }
              break;

            case 'ModificationDetected':
              isModification = true;
              break;

            case 'CodeDiff':
              diffData = {
                diff_lines: event.diff_lines,
                additions: event.additions,
                deletions: event.deletions,
              };
              break;

            case 'Done':
              if (event.validated) backendValidated = true;
              break;
          }
        }, existingCode);

        if (chatStore.generationId !== myGen) return;

        // Post-generation execution for modification path
        if (isIterative && validatedStl) {
          viewportStore.setPendingStl(validatedStl);
        } else if (isIterative) {
          // Iterative completed (possibly with skipped steps), code already set via FinalCode
        } else if (validatedStl) {
          viewportStore.setPendingStl(validatedStl);
        } else if (backendValidated) {
          // Backend validated but failed — code is already set in editor via FinalCode event.
        } else if (isMultiPart) {
          const assembledCode = result;
          project.setCode(assembledCode);
          chatStore.updateLastMessage(
            chatStore.messages[chatStore.messages.length - 1]?.content +
              '\n\nAssembly complete! Executing code...'
          );

          try {
            const execResult = await executeCode(assembledCode);
            if (chatStore.generationId !== myGen) return;
            if (execResult.success && execResult.stl_base64) {
              viewportStore.setPendingStl(execResult.stl_base64);
              chatStore.addMessage({
                id: generateId(),
                role: 'system',
                content: 'Assembly executed successfully.',
                timestamp: Date.now(),
              });
            } else if (!execResult.success) {
              const errorInfo = execResult.stderr || 'Code execution failed';
              chatStore.addMessage({
                id: generateId(),
                role: 'system',
                content: `Execution error: ${errorInfo}`,
                timestamp: Date.now(),
                isError: true,
                failedCode: assembledCode,
                errorMessage: errorInfo,
              });

              chatStore.setStreaming(false);
              await handleAutoRetry(assembledCode, errorInfo, 1);
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
              failedCode: assembledCode,
              errorMessage: errMsg,
            });

            chatStore.setStreaming(false);
            await handleAutoRetry(assembledCode, errMsg, 1);
            return;
          }
        } else {
          const code = extractPythonCode(result);
          if (code) {
            project.setCode(code);

            try {
              const execResult = await executeCode(code);
              if (chatStore.generationId !== myGen) return;
              if (execResult.success && execResult.stl_base64) {
                viewportStore.setPendingStl(execResult.stl_base64);
              } else if (!execResult.success) {
                const errorInfo = execResult.stderr || 'Code execution failed';
                chatStore.addMessage({
                  id: generateId(),
                  role: 'system',
                  content: `Execution error: ${errorInfo}`,
                  timestamp: Date.now(),
                  isError: true,
                  failedCode: code,
                  errorMessage: errorInfo,
                });

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

              chatStore.setStreaming(false);
              await handleAutoRetry(code, errMsg, 1);
              return;
            }
          }
        }
      } else {
        // ── New geometry: two-phase plan flow ──
        const planResult = await generateDesignPlan(text, rustHistory, (event: MultiPartEvent) => {
          if (chatStore.generationId !== myGen) return;

          switch (event.kind) {
            case 'PlanStatus':
              {
                const elapsed = Math.round((Date.now() - planStartTime) / 1000);
                chatStore.updateLastMessage(`${event.message} (${elapsed}s)`);
                if (!planTimerInterval) {
                  planTimerInterval = setInterval(() => {
                    if (chatStore.generationId !== myGen) {
                      if (planTimerInterval) clearInterval(planTimerInterval);
                      planTimerInterval = null;
                      return;
                    }
                    const el = Math.round((Date.now() - planStartTime) / 1000);
                    chatStore.updateLastMessage(`${event.message} (${el}s)`);
                  }, 1000);
                }
              }
              break;

            case 'PlanValidation':
              if (!event.is_valid) {
                const lastContent = chatStore.messages[chatStore.messages.length - 1]?.content || '';
                chatStore.updateLastMessage(
                  `${lastContent}\n\u26A0 Plan risk score: ${event.risk_score}/10 — ${event.rejected_reason ?? 'Re-planning...'}`
                );
              }
              break;

            case 'DesignPlan':
              designPlanText = event.plan_text;
              lastDesignPlanText = event.plan_text;
              break;

            case 'TokenUsage':
              if (event.phase === 'total') {
                tokenUsageSummary = {
                  input_tokens: event.input_tokens,
                  output_tokens: event.output_tokens,
                  total_tokens: event.total_tokens,
                  cost_usd: event.cost_usd,
                };
              }
              break;
          }
        });

        if (chatStore.generationId !== myGen) return;
        if (planTimerInterval) {
          clearInterval(planTimerInterval);
          planTimerInterval = null;
        }

        if (settingsStore.config.auto_approve_plan) {
          // Auto-approve: immediately proceed to code generation
          chatStore.updateLastMessage('Plan approved (auto). Generating code...');
          await runFromPlan(planResult.plan_text, text, rustHistory, null);
        } else {
          // Show editor, pause for user approval
          pendingPlan = planResult;
          pendingUserRequest = text;
          pendingHistory = rustHistory;
          showPlanEditor = true;
          chatStore.setStreaming(false);
        }
      }
    } catch (err) {
      if (streamingContent.length === 0 && !isMultiPart) {
        chatStore.updateLastMessage(`Error: ${err}`);
      } else {
        chatStore.addMessage({
          id: generateId(),
          role: 'system',
          content: `Error: ${err}`,
          timestamp: Date.now(),
          isError: true,
        });
      }
    } finally {
      if (planTimerInterval) {
        clearInterval(planTimerInterval);
        planTimerInterval = null;
      }
      if (chatStore.generationId === myGen) {
        chatStore.setStreaming(false);
        isMultiPart = false;
        partProgress = [];
        designPlanText = '';
        isIterative = false;
        iterativeSteps = [];
        isModification = false;
        isConsensus = false;
        consensusProgress = [];
        showPlanEditor = false;
        pendingPlan = null;
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
    {#if showPlanEditor && pendingPlan}
      <DesignPlanEditor
        planText={pendingPlan.plan_text}
        previousPlanText={lastDesignPlanText && lastDesignPlanText !== pendingPlan.plan_text ? lastDesignPlanText : null}
        riskScore={pendingPlan.risk_score}
        warnings={pendingPlan.warnings}
        isValid={pendingPlan.is_valid}
        onApprove={handlePlanApprove}
        onReject={handlePlanReject}
        templates={PLAN_TEMPLATES}
      />
    {:else if designPlanText}
      <details class="design-plan-block">
        <summary class="design-plan-summary">Geometry Design Plan</summary>
        <div class="design-plan-content">{designPlanText}</div>
      </details>
    {/if}
    {#if diffData && !chatStore.isStreaming}
      <details class="diff-block" open>
        <summary class="diff-summary">
          Code Changes (+{diffData.additions} −{diffData.deletions})
        </summary>
        <div class="diff-content">
          {#each diffData.diff_lines as line}
            <div class="diff-line diff-{line.tag}">
              <span class="diff-marker">
                {#if line.tag === 'insert'}+{:else if line.tag === 'delete'}-{:else}&nbsp;{/if}
              </span>
              <span class="diff-text">{line.text}</span>
            </div>
          {/each}
        </div>
      </details>
    {/if}
    {#if skippedStepsData.length > 0 && !chatStore.isStreaming && !isRetrying}
      <div class="retry-skipped-bar">
        <span class="retry-skipped-label">{skippedStepsData.length} step(s) were skipped</span>
        <button class="retry-skipped-btn" onclick={handleRetrySkippedSteps}>
          Retry Skipped Steps
        </button>
      </div>
    {/if}
    {#if tokenUsageSummary && !chatStore.isStreaming && !isRetrying}
      <div class="token-usage-badge">
        <span class="token-count">{tokenUsageSummary.total_tokens.toLocaleString()} tokens</span>
        {#if tokenUsageSummary.cost_usd !== null && tokenUsageSummary.cost_usd > 0}
          <span class="token-cost">/ ${tokenUsageSummary.cost_usd.toFixed(4)}</span>
        {:else if tokenUsageSummary.cost_usd === 0}
          <span class="token-cost">/ free (local)</span>
        {/if}
      </div>
    {/if}
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

  .token-usage-badge {
    display: flex;
    justify-content: flex-end;
    gap: 4px;
    padding: 2px 16px 6px;
    font-size: 10px;
    font-variant-numeric: tabular-nums;
    color: var(--text-muted);
  }

  .token-count {
    opacity: 0.8;
  }

  .token-cost {
    opacity: 0.6;
  }

  .retry-skipped-bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    margin: 4px 12px;
    padding: 6px 10px;
    background: var(--bg-mantle);
    border: 1px solid var(--border-subtle);
    border-radius: 6px;
  }

  .retry-skipped-label {
    font-size: 11px;
    color: var(--text-secondary);
  }

  .retry-skipped-btn {
    background: var(--accent);
    color: var(--bg-base);
    border: none;
    border-radius: 4px;
    padding: 4px 10px;
    font-size: 11px;
    font-weight: 600;
    cursor: pointer;
    white-space: nowrap;
    transition: background-color 0.15s ease;
  }

  .retry-skipped-btn:hover {
    background: var(--accent-hover);
  }

  .design-plan-block {
    margin: 4px 12px;
    border: 1px solid var(--border-subtle);
    border-radius: 6px;
    background: var(--bg-mantle);
    overflow: hidden;
  }

  .design-plan-summary {
    padding: 6px 10px;
    font-size: 11px;
    font-weight: 600;
    color: var(--accent);
    cursor: pointer;
    user-select: none;
    list-style: none;
  }

  .design-plan-summary::before {
    content: '\25B6  ';
    font-size: 9px;
  }

  .design-plan-block[open] .design-plan-summary::before {
    content: '\25BC  ';
  }

  .design-plan-summary::-webkit-details-marker {
    display: none;
  }

  .design-plan-content {
    padding: 8px 12px;
    font-size: 12px;
    line-height: 1.5;
    color: var(--text-secondary);
    white-space: pre-wrap;
    border-top: 1px solid var(--border-subtle);
    max-height: 300px;
    overflow-y: auto;
  }

  .diff-block {
    margin: 4px 12px;
    border: 1px solid var(--border-subtle);
    border-radius: 6px;
    background: var(--bg-mantle);
    overflow: hidden;
  }

  .diff-summary {
    padding: 6px 10px;
    font-size: 11px;
    font-weight: 600;
    color: var(--accent);
    cursor: pointer;
    user-select: none;
    list-style: none;
  }

  .diff-summary::before {
    content: '\25B6  ';
    font-size: 9px;
  }

  .diff-block[open] .diff-summary::before {
    content: '\25BC  ';
  }

  .diff-summary::-webkit-details-marker {
    display: none;
  }

  .diff-content {
    border-top: 1px solid var(--border-subtle);
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
</style>
