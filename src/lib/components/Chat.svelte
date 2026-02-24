<script lang="ts">
  import { getChatStore } from '$lib/stores/chat.svelte';
  import { getProjectStore } from '$lib/stores/project.svelte';
  import { getViewportStore } from '$lib/stores/viewport.svelte';
  import { generateParallel, generateDesignPlan, generateFromPlan, extractPythonCode, executeCode, autoRetry, sendMessageStreaming, retrySkippedSteps, retryPart } from '$lib/services/tauri';
  import { executeGeneratedCode, resolveGeneratedCode } from '$lib/services/chat-generation-execution';
  import { getSettingsStore } from '$lib/stores/settings.svelte';
  import ChatMessageComponent from './ChatMessage.svelte';
  import ConfidenceBadge from './ConfidenceBadge.svelte';
  import DesignPlanEditor from './DesignPlanEditor.svelte';
  import MultiPartProgress from './MultiPartProgress.svelte';
  import { PLAN_TEMPLATES } from '$lib/data/plan-templates';
  import type { ChatMessage, RustChatMessage, MultiPartEvent, PartProgress, PartSpec, IterativeStepProgress, SkippedStepInfo, TokenUsageData, DiffLine, DesignPlanResult, GenerationEntry, PendingAssemblyPart } from '$lib/types';
  import { getGenerationHistoryStore } from '$lib/stores/generationHistory.svelte';
  import { onMount, onDestroy } from 'svelte';

  const MAX_RETRIES = 3;

  const chatStore = getChatStore();
  const project = getProjectStore();
  const viewportStore = getViewportStore();
  const settingsStore = getSettingsStore();
  const generationHistoryStore = getGenerationHistoryStore();

  let generationStartTime = $state(0);
  let generationType = $state<GenerationEntry['generationType']>('single');
  let retryCountForEntry = $state(0);
  let lastGeneratedCode = $state('');
  let lastGeneratedStl = $state<string | undefined>(undefined);
  let lastGenerationSuccess = $state(true);
  let lastGenerationError = $state<string | undefined>(undefined);

  let inputText = $state('');
  let messagesContainer = $state<HTMLElement | null>(null);
  let userHasScrolledUp = $state(false);
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
  let assemblyStl = $state<string | null>(null);
  let multiPartPlanParts = $state<PartSpec[]>([]);
  let multipartImportQueued = $state(false);
  let diffData = $state<{ diff_lines: DiffLine[]; additions: number; deletions: number } | null>(null);
  let isModification = $state(false);
  let isConsensus = $state(false);
  let consensusProgress = $state<{ label: string; status: string; temperature?: number; hasCode?: boolean; executionSuccess?: boolean }[]>([]);
  let confidenceData = $state<{
    level: 'high' | 'medium' | 'low';
    score: number;
    message: string;
    cookbookMatches: string[];
  } | null>(null);

  // Plan editor state (two-phase flow)
  let showPlanEditor = $state(false);
  let pendingPlan = $state<DesignPlanResult | null>(null);
  let pendingUserRequest = $state('');
  let pendingHistory = $state<RustChatMessage[]>([]);

  // Prompt triage clarification state
  let awaitingClarification = $state(false);
  let clarificationOriginalPrompt = $state('');

  function generateId(): string {
    return Date.now().toString(36) + Math.random().toString(36).slice(2, 7);
  }

  function recordGeneration(opts: {
    code: string; stl_base64?: string; success: boolean; error?: string;
  }) {
    if (!opts.code && !opts.error) return;
    generationHistoryStore.addEntry({
      id: generateId(),
      timestamp: Date.now(),
      userPrompt: lastUserRequest,
      code: opts.code,
      stl_base64: opts.stl_base64,
      success: opts.success,
      error: opts.error,
      provider: settingsStore.config.ai_provider,
      model: settingsStore.config.model,
      durationMs: Date.now() - generationStartTime,
      tokenUsage: tokenUsageSummary ?? undefined,
      confidenceScore: confidenceData?.score,
      confidenceLevel: confidenceData?.level,
      generationType,
      retryCount: retryCountForEntry,
      pinned: false,
    });
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
   * Client-side assembly fallback: mirrors Rust's assemble_parts() logic.
   * Used when the backend times out after parts complete but before FinalCode is emitted.
   */
  function assemblePartsClientSide(
    parts: PartProgress[],
    planParts: PartSpec[],
  ): string {
    let code = 'from build123d import *\n\n';
    for (const part of parts) {
      const varName = `part_${part.name}`;
      const cleaned = part.code!
        .split('\n')
        .filter((line) => {
          const t = line.trim();
          return !t.startsWith('from build123d') && !t.startsWith('import build123d');
        })
        .join('\n')
        .replace(/\bresult\b/g, varName);
      code += `# --- ${part.name} ---\n${cleaned}\n\n`;
    }
    code += '# --- Assembly ---\n_parts = []\n';
    for (const part of parts) {
      const varName = `part_${part.name}`;
      const pos = planParts.find((p) => p.name === part.name)?.position ?? [0, 0, 0];
      code += `_parts.append(${varName}.move(Location((${pos[0]}, ${pos[1]}, ${pos[2]}))))\n`;
    }
    code += 'result = Compound(children=_parts)\n';
    return code;
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
    confidenceData = null;
    showPlanEditor = false;
    pendingPlan = null;
    assemblyStl = null;
    multiPartPlanParts = [];
    multipartImportQueued = false;
  }

  async function handleAutoRetry(failedCode: string, errorMessage: string, attempt: number) {
    if (attempt > MAX_RETRIES) return;

    retryCountForEntry = attempt;
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

  async function executeAndHandleGeneratedCode(
    code: string,
    generationId: number,
    successMessage?: string,
  ): Promise<boolean> {
    const result = await executeGeneratedCode(code);
    if (chatStore.generationId !== generationId) return false;

    if (result.success) {
      if (result.stlBase64) {
        viewportStore.setPendingStl(result.stlBase64);
        lastGeneratedStl = result.stlBase64;
      }
      if (successMessage) {
        chatStore.addMessage({
          id: generateId(),
          role: 'system',
          content: successMessage,
          timestamp: Date.now(),
        });
      }
      return true;
    }

    lastGenerationSuccess = false;
    lastGenerationError = result.error;
    chatStore.addMessage({
      id: generateId(),
      role: 'system',
      content: `Execution error: ${result.error}`,
      timestamp: Date.now(),
      isError: true,
      failedCode: code,
      errorMessage: result.error,
    });
    chatStore.setStreaming(false);
    await handleAutoRetry(code, result.error, 1);
    return false;
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

  function normalizePartKey(name: string, index: number): string {
    const normalized = name.trim().toLowerCase().replace(/[^a-z0-9]+/g, '_').replace(/^_+|_+$/g, '');
    return normalized || `part_${index}`;
  }

  function tryQueueMultipartAssemblyImport(requireAllParts = true): boolean {
    if (!isMultiPart || multipartImportQueued) return false;
    if (partProgress.length === 0) return false;
    const readyParts = partProgress
      .map((p, index) => ({ p, index }))
      .filter(({ p }) => !!p.stl_base64);
    if (requireAllParts && readyParts.length !== partProgress.length) return false;
    if (readyParts.length === 0) return false;

    const parts: PendingAssemblyPart[] = readyParts.map(({ p, index }) => ({
      part_key: normalizePartKey(multiPartPlanParts[index]?.name ?? p.name, index),
      name: p.name,
      stl_base64: p.stl_base64!,
      position: p.position,
    }));

    viewportStore.setPendingAssemblyParts(parts);
    multipartImportQueued = true;
    return true;
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

  function updateConfidence(
    signal: { reviewModified?: boolean; validationSuccess?: boolean; validationAttempt?: number }
  ) {
    if (!confidenceData) return;
    let adj = 0;
    let reason = confidenceData.message;

    if (signal.reviewModified === false) {
      adj += 10;
      reason = 'Code approved by reviewer';
    } else if (signal.reviewModified === true) {
      adj -= 5;
      reason = 'Code required reviewer corrections';
    }

    if (signal.validationSuccess === true && signal.validationAttempt === 1) {
      adj += 15;
      reason = 'Validated on first attempt';
    } else if (signal.validationSuccess === true && (signal.validationAttempt ?? 0) > 1) {
      adj += 5;
      reason = `Validated after ${signal.validationAttempt} attempts`;
    } else if (signal.validationSuccess === false) {
      adj -= 20;
      reason = 'Validation failed';
    }

    const s = Math.max(0, Math.min(100, confidenceData.score + adj));
    confidenceData = {
      ...confidenceData,
      score: s,
      level: s >= 70 ? 'high' : s >= 40 ? 'medium' : 'low',
      message: reason,
    };
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
    multipartImportQueued = false;
    chatStore.setStreaming(true);
    let streamingContent = '';
    let validatedStl: string | null = null;
    let backendValidationFinished = false;
    let backendValidationSucceeded = false;
    let backendDoneError: string | null = null;

    try {
      const result = await generateFromPlan(planText, userRequest, rustHistory, (event: MultiPartEvent) => {
        if (chatStore.generationId !== myGen) return;

        switch (event.kind) {
          case 'PlanStatus':
            {
              chatStore.updateLastMessage(event.message);
            }
            break;

          case 'RetrievalStatus':
            {
              const detail = event.items.length > 0
                ? `${event.message} (${event.items.length} snippets)`
                : event.message;
              const last = chatStore.messages[chatStore.messages.length - 1]?.content || '';
              chatStore.updateLastMessage(`${last}\n${detail}`);
            }
            break;

          case 'ConfidenceAssessment':
            confidenceData = {
              level: event.level,
              score: event.score,
              message: event.message,
              cookbookMatches: event.cookbook_matches,
            };
            break;

          case 'PlanResult':
            if (event.plan.mode === 'multi' && event.plan.parts.length > 0) {
              isMultiPart = true;
              generationType = 'multi-part';
              multiPartPlanParts = event.plan.parts;
              partProgress = event.plan.parts.map((p) => ({
                name: p.name,
                status: 'pending' as const,
                streamedText: '',
                description: p.description,
                constraints: p.constraints,
                position: p.position,
              }));
              const desc = event.plan.description
                ? `${event.plan.description}`
                : 'Generating parts...';
              chatStore.updateLastMessage(desc);
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
            }
            break;

          case 'PartCodeExtracted':
            if (partProgress[event.part_index]) {
              partProgress[event.part_index].code = event.code;
            }
            break;

          case 'PartComplete':
            if (partProgress[event.part_index]) {
              partProgress[event.part_index].status = event.success ? 'complete' : 'failed';
              if (event.error) partProgress[event.part_index].error = event.error;
            }
            break;

          case 'PartStlReady':
            if (partProgress[event.part_index]) {
              partProgress[event.part_index].stl_base64 = event.stl_base64;
              tryQueueMultipartAssemblyImport();
            }
            break;

          case 'PartStlFailed':
            if (partProgress[event.part_index]) {
              partProgress[event.part_index].error = `Part preview failed: ${event.error}`;
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
            lastGeneratedCode = event.code;
            if (event.stl_base64) {
              validatedStl = event.stl_base64;
              lastGeneratedStl = event.stl_base64;
              if (isMultiPart) assemblyStl = event.stl_base64;
            }
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
              updateConfidence({ reviewModified: event.was_modified });
            }
            break;

          case 'ValidationAttempt':
            {
              const lastContent5 = chatStore.messages[chatStore.messages.length - 1]?.content || '';
              chatStore.updateLastMessage(`${lastContent5}\n\n${event.message}`);
            }
            break;

          case 'StaticValidationReport':
            {
              const last = chatStore.messages[chatStore.messages.length - 1]?.content || '';
              if (!event.passed && event.findings.length > 0) {
                chatStore.updateLastMessage(`${last}\nStatic validation: ${event.findings[0]}`);
              }
            }
            break;

          case 'ValidationSuccess':
            {
              const lastContent6 = chatStore.messages[chatStore.messages.length - 1]?.content || '';
              chatStore.updateLastMessage(`${lastContent6}\nExecution validation passed.`);
              updateConfidence({ validationSuccess: true, validationAttempt: event.attempt });
            }
            break;

          case 'ValidationFailed':
            {
              const lastContent7 = chatStore.messages[chatStore.messages.length - 1]?.content || '';
              const note = event.will_retry
                ? `Execution failed (${event.error_category}), retrying...`
                : `Execution failed: ${event.error_message}`;
              chatStore.updateLastMessage(`${lastContent7}\n${note}`);
              if (!event.will_retry) {
                updateConfidence({ validationSuccess: false });
              }
            }
            break;

          case 'PostGeometryValidationReport':
            {
              const last = chatStore.messages[chatStore.messages.length - 1]?.content || '';
              const summary = event.report.manifold && event.report.bbox_ok
                ? `Geometry checks passed (components: ${event.report.component_count}, triangles: ${event.report.triangle_count}).`
                : `Geometry checks flagged issues: ${event.report.warnings.join('; ')}`;
              chatStore.updateLastMessage(`${last}\n${summary}`);
            }
            break;

          case 'PostGeometryValidationWarning':
            {
              const last = chatStore.messages[chatStore.messages.length - 1]?.content || '';
              chatStore.updateLastMessage(`${last}\n${event.message}`);
            }
            break;

          case 'SemanticValidationReport':
            {
              const last = chatStore.messages[chatStore.messages.length - 1]?.content || '';
              const details = event.findings.length > 0 ? `: ${event.findings.join('; ')}` : '';
              const msg = event.passed
                ? `Semantic validation passed for ${event.part_name}${details}`
                : `Semantic validation failed for ${event.part_name}${details}`;
              chatStore.updateLastMessage(`${last}\n${msg}`);
            }
            break;

          case 'IterativeStart':
            isIterative = true;
            generationType = 'iterative';
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
            if (event.validated) {
              backendValidationFinished = true;
              backendValidationSucceeded = event.success;
              backendDoneError = event.error ?? null;
            }

            if (isMultiPart && event.error) {
              let changed = false;
              partProgress = partProgress.map((p) => {
                if (p.status === 'pending' || p.status === 'generating') {
                  changed = true;
                  return {
                    ...p,
                    status: 'failed',
                    error: p.error ?? event.error ?? 'Generation aborted before part completion.',
                  };
                }
                return p;
              });
              if (changed) {
                chatStore.updateLastMessage(formatPartProgress(partProgress));
              }
            }

            tryQueueMultipartAssemblyImport();
            break;
        }
      }, existingCode);

      if (chatStore.generationId !== myGen) return;

      const importedMultipart = isMultiPart && (multipartImportQueued || tryQueueMultipartAssemblyImport(true));
      if (importedMultipart) {
        chatStore.addMessage({
          id: generateId(),
          role: 'system',
          content: 'Imported multipart result as editable assembly components.',
          timestamp: Date.now(),
        });
      } else if (isIterative && validatedStl) {
        viewportStore.setPendingStl(validatedStl);
      } else if (validatedStl) {
        viewportStore.setPendingStl(validatedStl);
      } else if (isMultiPart && tryQueueMultipartAssemblyImport(false)) {
        chatStore.addMessage({
          id: generateId(),
          role: 'system',
          content: 'Imported accepted parts as partial editable assembly.',
          timestamp: Date.now(),
        });
      } else if (isMultiPart && previewFirstAvailablePart()) {
        chatStore.addMessage({
          id: generateId(),
          role: 'system',
          content: 'Full assembly preview unavailable. Showing first available part preview.',
          timestamp: Date.now(),
        });
      } else if (backendValidationFinished && !backendValidationSucceeded) {
        chatStore.addMessage({
          id: generateId(),
          role: 'system',
          content: backendDoneError
            ? `Generation finished with validation failure: ${backendDoneError}`
            : 'Generation finished with validation failure. No preview artifact available.',
          timestamp: Date.now(),
          isError: true,
        });
      } else if (backendValidationSucceeded) {
        // Validation succeeded but no STL artifact was returned.
      } else if (isMultiPart) {
        const assembledCode = resolveGeneratedCode(result, true);
        if (!assembledCode) return;
        project.setCode(assembledCode);
        lastGeneratedCode = assembledCode;
        chatStore.updateLastMessage(
          chatStore.messages[chatStore.messages.length - 1]?.content +
            '\n\nAssembly complete! Executing code...'
        );
        const executed = await executeAndHandleGeneratedCode(assembledCode, myGen);
        if (!executed) return;
      } else {
        const code = resolveGeneratedCode(result, false);
        if (code) {
          project.setCode(code);
          lastGeneratedCode = code;
          const executed = await executeAndHandleGeneratedCode(code, myGen);
          if (!executed) return;
        }
      }
    } catch (err) {
      lastGenerationSuccess = false;
      lastGenerationError = `${err}`;

      // Timeout recovery: if parts completed, assemble what we have
      const isTimeout = `${err}`.includes('runtime exceeded');
      if (isTimeout && isMultiPart) {
        const completedParts = partProgress.filter((p) => p.status === 'complete' && p.code);
        if (completedParts.length > 0 && !lastGeneratedCode) {
          const assembled = assemblePartsClientSide(completedParts, multiPartPlanParts);
          project.setCode(assembled);
          lastGeneratedCode = assembled;
          lastGenerationSuccess = true;
          lastGenerationError = undefined;
          chatStore.updateLastMessage(
            'Generation timed out during validation, but parts were assembled successfully. ' +
            'The code may not be fully validated — review before use.',
          );
          await executeAndHandleGeneratedCode(assembled, myGen, 'Assembly executed (unvalidated).');
          return;
        }
      }

      if (isMultiPart) {
        let changed = false;
        partProgress = partProgress.map((p) => {
          if (p.status === 'pending' || p.status === 'generating') {
            changed = true;
            return {
              ...p,
              status: 'failed',
              error: p.error ?? `${err}`,
            };
          }
          return p;
        });
        if (changed) {
          chatStore.updateLastMessage(formatPartProgress(partProgress));
        }
      }

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
        // Record generation to history
        if (lastGeneratedCode || lastGenerationError) {
          recordGeneration({
            code: lastGeneratedCode,
            stl_base64: lastGeneratedStl,
            success: lastGenerationSuccess,
            error: lastGenerationError,
          });
        }
        chatStore.setStreaming(false);
        // Do NOT reset isMultiPart/partProgress — cards persist for user interaction
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

  function handlePreviewPart(index: number) {
    if (partProgress[index]?.stl_base64) {
      viewportStore.setPendingStl(partProgress[index].stl_base64!);
    }
  }

  function handleShowAssembly() {
    if (assemblyStl) {
      viewportStore.setPendingStl(assemblyStl);
    }
  }

  function previewFirstAvailablePart(): boolean {
    const firstWithStl = partProgress.find((p) => !!p.stl_base64);
    if (!firstWithStl?.stl_base64) return false;
    viewportStore.setPendingStl(firstWithStl.stl_base64);
    return true;
  }

  async function handleRetryPart(index: number) {
    if (chatStore.isStreaming || isRetrying) return;
    const partSpec = multiPartPlanParts[index];
    if (!partSpec) return;

    const myGen = chatStore.generationId;
    isRetrying = true;

    // Reset part state to generating
    partProgress[index] = {
      ...partProgress[index],
      status: 'generating',
      streamedText: '',
      error: undefined,
      code: undefined,
      stl_base64: undefined,
    };

    try {
      await retryPart(
        index,
        partSpec,
        lastDesignPlanText,
        lastUserRequest,
        (event: MultiPartEvent) => {
          if (chatStore.generationId !== myGen) return;

          switch (event.kind) {
            case 'PartDelta':
              if (partProgress[event.part_index]) {
                partProgress[event.part_index].status = 'generating';
                partProgress[event.part_index].streamedText += event.delta;
              }
              break;
            case 'PartCodeExtracted':
              if (partProgress[event.part_index]) {
                partProgress[event.part_index].code = event.code;
              }
              break;
            case 'PartComplete':
              if (partProgress[event.part_index]) {
                partProgress[event.part_index].status = event.success ? 'complete' : 'failed';
                if (event.error) partProgress[event.part_index].error = event.error;
              }
              break;
            case 'PartStlReady':
              if (partProgress[event.part_index]) {
                partProgress[event.part_index].stl_base64 = event.stl_base64;
              }
              break;
            case 'PartStlFailed':
              if (partProgress[event.part_index]) {
                partProgress[event.part_index].error = `Part preview failed: ${event.error}`;
              }
              break;
            case 'Done':
              break;
          }
        },
      );
    } catch (err) {
      partProgress[index] = {
        ...partProgress[index],
        status: 'failed',
        error: `Retry failed: ${err}`,
      };
    } finally {
      if (chatStore.generationId === myGen) {
        isRetrying = false;
      }
    }
  }

  async function handleSend() {
    let text = inputText.trim();
    if (!text || chatStore.isStreaming) return;

    userHasScrolledUp = false;

    // If we're awaiting clarification, combine original prompt with user's answers
    if (awaitingClarification && clarificationOriginalPrompt) {
      text = `${clarificationOriginalPrompt}\n\nUser clarifications:\n${text}`;
      awaitingClarification = false;
      clarificationOriginalPrompt = '';
    }

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
    confidenceData = null;
    assemblyStl = null;
    multiPartPlanParts = [];
    multipartImportQueued = false;
    lastUserRequest = text;
    generationStartTime = Date.now();
    generationType = 'single';
    retryCountForEntry = 0;
    lastGeneratedCode = '';
    lastGeneratedStl = undefined;
    lastGenerationSuccess = true;
    lastGenerationError = undefined;
    let validatedStl: string | null = null;
    let backendValidationFinished = false;
    let backendValidationSucceeded = false;
    let backendDoneError: string | null = null;

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
    const DEFAULT_CODE_TEMPLATE = `from build123d import *\n\n# Create your 3D model here\nwith BuildPart() as part:\n    Box(10, 10, 10)\nresult = part.part\n`;
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
                const extras = [
                  event.fatal_combo ? 'fatal combo' : null,
                  event.negation_conflict ? 'negation conflict' : null,
                ].filter(Boolean).join(', ');
                chatStore.updateLastMessage(
                  `${lastContent}\n\u26A0 Plan risk score: ${event.risk_score}/10 — ${event.rejected_reason ?? 'Re-planning...'}${extras ? ` [${extras}]` : ''}`
                );
              }
              break;

            case 'ConfidenceAssessment':
              confidenceData = {
                level: event.level,
                score: event.score,
                message: event.message,
                cookbookMatches: event.cookbook_matches,
              };
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

            case 'RetrievalStatus':
              {
                const last = chatStore.messages[chatStore.messages.length - 1]?.content || '';
                const detail = event.items.length > 0
                  ? `${event.message} (${event.items.length} snippets)`
                  : event.message;
                chatStore.updateLastMessage(`${last}\n${detail}`);
              }
              break;

            case 'PlanResult':
              if (planTimerInterval) {
                clearInterval(planTimerInterval);
                planTimerInterval = null;
              }
              if (event.plan.mode === 'multi' && event.plan.parts.length > 0) {
                isMultiPart = true;
                generationType = 'multi-part';
                multiPartPlanParts = event.plan.parts;
                partProgress = event.plan.parts.map((p) => ({
                  name: p.name,
                  status: 'pending' as const,
                  streamedText: '',
                  description: p.description,
                  constraints: p.constraints,
                  position: p.position,
                }));
                const desc = event.plan.description
                  ? `${event.plan.description}`
                  : 'Generating parts...';
                chatStore.updateLastMessage(desc);
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
              }
              break;

            case 'PartCodeExtracted':
              if (partProgress[event.part_index]) {
                partProgress[event.part_index].code = event.code;
              }
              break;

            case 'PartComplete':
              if (partProgress[event.part_index]) {
                partProgress[event.part_index].status = event.success ? 'complete' : 'failed';
                if (event.error) {
                  partProgress[event.part_index].error = event.error;
                }
              }
              break;

            case 'PartStlReady':
              if (partProgress[event.part_index]) {
                partProgress[event.part_index].stl_base64 = event.stl_base64;
                tryQueueMultipartAssemblyImport();
              }
              break;

            case 'PartStlFailed':
              if (partProgress[event.part_index]) {
                partProgress[event.part_index].error = `Part preview failed: ${event.error}`;
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
              lastGeneratedCode = event.code;
              if (event.stl_base64) {
                validatedStl = event.stl_base64;
                lastGeneratedStl = event.stl_base64;
                if (isMultiPart) assemblyStl = event.stl_base64;
              }
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
                updateConfidence({ reviewModified: event.was_modified });
              }
              break;

            case 'ValidationAttempt':
              {
                const lastContent5 = chatStore.messages[chatStore.messages.length - 1]?.content || '';
                chatStore.updateLastMessage(`${lastContent5}\n\n${event.message}`);
              }
              break;

            case 'StaticValidationReport':
              {
                const last = chatStore.messages[chatStore.messages.length - 1]?.content || '';
                if (!event.passed && event.findings.length > 0) {
                  chatStore.updateLastMessage(`${last}\nStatic validation: ${event.findings[0]}`);
                }
              }
              break;

            case 'ValidationSuccess':
              {
                const lastContent6 = chatStore.messages[chatStore.messages.length - 1]?.content || '';
                chatStore.updateLastMessage(`${lastContent6}\nExecution validation passed.`);
                updateConfidence({ validationSuccess: true, validationAttempt: event.attempt });
              }
              break;

            case 'ValidationFailed':
              {
                const lastContent7 = chatStore.messages[chatStore.messages.length - 1]?.content || '';
                const note = event.will_retry
                  ? `Execution failed (${event.error_category}), retrying...`
                  : `Execution failed: ${event.error_message}`;
                chatStore.updateLastMessage(`${lastContent7}\n${note}`);
                if (!event.will_retry) {
                  updateConfidence({ validationSuccess: false });
                }
              }
              break;

            case 'PostGeometryValidationReport':
              {
                const last = chatStore.messages[chatStore.messages.length - 1]?.content || '';
                const summary = event.report.manifold && event.report.bbox_ok
                  ? `Geometry checks passed (components: ${event.report.component_count}, triangles: ${event.report.triangle_count}).`
                  : `Geometry checks flagged issues: ${event.report.warnings.join('; ')}`;
                chatStore.updateLastMessage(`${last}\n${summary}`);
              }
              break;

            case 'PostGeometryValidationWarning':
              {
                const last = chatStore.messages[chatStore.messages.length - 1]?.content || '';
                chatStore.updateLastMessage(`${last}\n${event.message}`);
              }
              break;

            case 'SemanticValidationReport':
              {
                const last = chatStore.messages[chatStore.messages.length - 1]?.content || '';
                const details = event.findings.length > 0 ? `: ${event.findings.join('; ')}` : '';
                const msg = event.passed
                  ? `Semantic validation passed for ${event.part_name}${details}`
                  : `Semantic validation failed for ${event.part_name}${details}`;
                chatStore.updateLastMessage(`${last}\n${msg}`);
              }
              break;

            case 'IterativeStart':
              isIterative = true;
              generationType = 'iterative';
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
              generationType = 'modification';
              break;

            case 'CodeDiff':
              diffData = {
                diff_lines: event.diff_lines,
                additions: event.additions,
                deletions: event.deletions,
              };
              break;

            case 'Done':
              if (event.validated) {
                backendValidationFinished = true;
                backendValidationSucceeded = event.success;
                backendDoneError = event.error ?? null;
              }
              tryQueueMultipartAssemblyImport();
              break;
          }
        }, existingCode);

        if (chatStore.generationId !== myGen) return;

        // Post-generation execution for modification path
        const importedMultipart = isMultiPart && (
          multipartImportQueued ||
          tryQueueMultipartAssemblyImport(true) ||
          tryQueueMultipartAssemblyImport(false)  // Try partial import too
        );
        if (importedMultipart) {
          chatStore.addMessage({
            id: generateId(),
            role: 'system',
            content: 'Imported multipart result as editable assembly components.',
            timestamp: Date.now(),
          });
        } else if (isIterative && validatedStl) {
          viewportStore.setPendingStl(validatedStl);
        } else if (isIterative) {
          // Iterative completed (possibly with skipped steps), code already set via FinalCode
        } else if (validatedStl) {
          viewportStore.setPendingStl(validatedStl);
        } else if (isMultiPart && previewFirstAvailablePart()) {
          chatStore.addMessage({
            id: generateId(),
            role: 'system',
            content: 'Full assembly preview unavailable. Showing first available part preview.',
            timestamp: Date.now(),
          });
        } else if (backendValidationFinished && !backendValidationSucceeded) {
          chatStore.addMessage({
            id: generateId(),
            role: 'system',
            content: backendDoneError
              ? `Generation finished with validation failure: ${backendDoneError}`
              : 'Generation finished with validation failure. No preview artifact available.',
            timestamp: Date.now(),
            isError: true,
          });
        } else if (backendValidationSucceeded) {
          // Validation succeeded but no STL artifact was returned.
        } else if (isMultiPart) {
          const assembledCode = resolveGeneratedCode(result, true);
          if (!assembledCode) return;
          project.setCode(assembledCode);
          lastGeneratedCode = assembledCode;
          chatStore.updateLastMessage(
            chatStore.messages[chatStore.messages.length - 1]?.content +
              '\n\nAssembly complete! Executing code...'
          );
          const executed = await executeAndHandleGeneratedCode(
            assembledCode,
            myGen,
            'Assembly executed successfully.',
          );
          if (!executed) return;
        } else {
          const code = resolveGeneratedCode(result, false);
          if (code) {
            project.setCode(code);
            lastGeneratedCode = code;
            const executed = await executeAndHandleGeneratedCode(code, myGen);
            if (!executed) return;
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
                const extras = [
                  event.fatal_combo ? 'fatal combo' : null,
                  event.negation_conflict ? 'negation conflict' : null,
                ].filter(Boolean).join(', ');
                chatStore.updateLastMessage(
                  `${lastContent}\n\u26A0 Plan risk score: ${event.risk_score}/10 — ${event.rejected_reason ?? 'Re-planning...'}${extras ? ` [${extras}]` : ''}`
                );
              }
              break;

            case 'DesignPlan':
              designPlanText = event.plan_text;
              lastDesignPlanText = event.plan_text;
              break;

            case 'ConfidenceAssessment':
              confidenceData = {
                level: event.level,
                score: event.score,
                message: event.message,
                cookbookMatches: event.cookbook_matches,
              };
              break;

            case 'ClarificationNeeded':
              // Will be handled after await returns
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

        // Check if triage needs clarification before showing plan editor
        if (planResult.clarification_questions?.length) {
          const questionsText = planResult.clarification_questions
            .map((q: string, i: number) => `${i + 1}. ${q}`)
            .join('\n');
          chatStore.updateLastMessage(
            `I need a few more details before designing this:\n\n${questionsText}\n\nPlease answer and I'll generate the design plan.`
          );
          chatStore.setStreaming(false);
          awaitingClarification = true;
          clarificationOriginalPrompt = text;
          return;
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
      lastGenerationSuccess = false;
      lastGenerationError = `${err}`;

      // Timeout recovery: if parts completed, assemble what we have
      const isTimeout = `${err}`.includes('runtime exceeded');
      if (isTimeout && isMultiPart) {
        const completedParts = partProgress.filter((p) => p.status === 'complete' && p.code);
        if (completedParts.length > 0 && !lastGeneratedCode) {
          const assembled = assemblePartsClientSide(completedParts, multiPartPlanParts);
          project.setCode(assembled);
          lastGeneratedCode = assembled;
          lastGenerationSuccess = true;
          lastGenerationError = undefined;
          chatStore.updateLastMessage(
            'Generation timed out during validation, but parts were assembled successfully. ' +
            'The code may not be fully validated — review before use.',
          );
          await executeAndHandleGeneratedCode(assembled, myGen, 'Assembly executed (unvalidated).');
          return;
        }
      }

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
        // Record generation to history (only if we actually generated something)
        if (lastGeneratedCode || lastGenerationError) {
          recordGeneration({
            code: lastGeneratedCode,
            stl_base64: lastGeneratedStl,
            success: lastGenerationSuccess,
            error: lastGenerationError,
          });
        }
        chatStore.setStreaming(false);
        // Do NOT reset isMultiPart/partProgress — cards persist for user interaction
        designPlanText = '';
        isIterative = false;
        iterativeSteps = [];
        isModification = false;
        isConsensus = false;
        consensusProgress = [];
        if (!pendingPlan) {
          showPlanEditor = false;
        }
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

  function handleMessagesScroll() {
    if (!messagesContainer) return;
    const { scrollTop, scrollHeight, clientHeight } = messagesContainer;
    userHasScrolledUp = scrollHeight - scrollTop - clientHeight > 80;
  }

  // Auto-scroll to bottom when messages change (unless user scrolled up)
  $effect(() => {
    const _ = chatStore.messages.length;
    // Also track last message content for streaming scroll
    const lastMsg = chatStore.messages[chatStore.messages.length - 1];
    const __ = lastMsg?.content;
    if (messagesContainer && !userHasScrolledUp) {
      // Use setTimeout to wait for DOM update
      setTimeout(() => {
        if (messagesContainer && !userHasScrolledUp) {
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

  function handleHistoryRestore(e: Event) {
    const entry = (e as CustomEvent<GenerationEntry>).detail;
    project.setCode(entry.code);
    if (entry.stl_base64) {
      viewportStore.setPendingStl(entry.stl_base64);
    }
  }

  function handleMechanismInsert(e: Event) {
    const detail = (e as CustomEvent<{ prompt?: string; mechanismId?: string }>).detail;
    if (!detail?.prompt) return;
    inputText = inputText.trim()
      ? `${inputText.trim()}\n\n${detail.prompt}`
      : detail.prompt;
  }

  onMount(() => {
    window.addEventListener('keydown', handleWindowKeydown);
    window.addEventListener('generation-history:restore', handleHistoryRestore);
    window.addEventListener('mechanism:insert-prompt', handleMechanismInsert);

    // Add welcome message
    chatStore.addMessage({
      id: generateId(),
      role: 'system',
      content: 'Welcome to CAD AI Studio. Describe what you want to build and I will generate Build123d code for you.',
      timestamp: Date.now(),
    });
  });

  onDestroy(() => {
    window.removeEventListener('keydown', handleWindowKeydown);
    window.removeEventListener('generation-history:restore', handleHistoryRestore);
    window.removeEventListener('mechanism:insert-prompt', handleMechanismInsert);
  });
</script>

<div class="chat-panel">
  <div class="chat-header">
    <span class="chat-title">Chat</span>
    <button class="clear-btn" onclick={() => chatStore.clear()} title="Clear chat">
      Clear
    </button>
  </div>

  <div class="messages-list" bind:this={messagesContainer} onscroll={handleMessagesScroll}>
    {#each chatStore.messages as message (message.id)}
      <ChatMessageComponent
        {message}
        onRetry={handleRetryFromMessage}
        onExplainError={handleExplainFromMessage}
        disableActions={chatStore.isStreaming || isRetrying}
      />
    {/each}
    {#if isMultiPart && partProgress.length > 0}
      <MultiPartProgress
        parts={partProgress}
        {assemblyStl}
        isGenerating={chatStore.isStreaming}
        onPreviewPart={handlePreviewPart}
        onRetryPart={handleRetryPart}
        onShowAssembly={handleShowAssembly}
        disableActions={chatStore.isStreaming || isRetrying}
      />
    {/if}
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
        confidenceLevel={confidenceData?.level}
        confidenceScore={confidenceData?.score}
        confidenceMessage={confidenceData?.message}
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
    {#if confidenceData && !chatStore.isStreaming && !isRetrying}
      <ConfidenceBadge level={confidenceData.level} score={confidenceData.score}
        message={confidenceData.message} cookbookMatches={confidenceData.cookbookMatches} />
    {/if}
    {#if chatStore.isStreaming || isRetrying}
      <div class="streaming-indicator">
        <span class="dot"></span>
        <span class="dot"></span>
        <span class="dot"></span>
        {#if isRetrying}
          <span class="retry-label">AI is fixing the code...</span>
        {/if}
        {#if confidenceData}
          <ConfidenceBadge level={confidenceData.level} score={confidenceData.score}
            message={confidenceData.message} compact />
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
