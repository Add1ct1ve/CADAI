import type { ChatMessage } from '$lib/types';

let messages = $state<ChatMessage[]>([]);
let isStreaming = $state(false);
let generationId = $state(0);

export function getChatStore() {
  return {
    get messages() {
      return messages;
    },
    get isStreaming() {
      return isStreaming;
    },
    get generationId() {
      return generationId;
    },
    addMessage(msg: ChatMessage) {
      messages = [...messages, msg];
    },
    setStreaming(val: boolean) {
      isStreaming = val;
    },
    updateLastMessage(content: string) {
      if (messages.length > 0) {
        const last = messages[messages.length - 1];
        messages = [...messages.slice(0, -1), { ...last, content }];
      }
    },
    cancelGeneration() {
      generationId++;
      isStreaming = false;
    },
    clear() {
      messages = [];
      isStreaming = false;
      generationId++;
    },
  };
}
