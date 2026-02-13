import type { ChatMessage } from '$lib/types';

const DEFAULT_CODE = `from build123d import *

# Create your 3D model here
result = Box(10, 10, 10)
`;

let name = $state('Untitled Project');
let code = $state(DEFAULT_CODE);
let previousCode = $state<string | null>(null);
let messages = $state<ChatMessage[]>([]);
let modified = $state(false);
let filePath = $state<string | null>(null);

export function getProjectStore() {
  return {
    get name() {
      return name;
    },
    get code() {
      return code;
    },
    get messages() {
      return messages;
    },
    get modified() {
      return modified;
    },
    get filePath() {
      return filePath;
    },
    get previousCode() {
      return previousCode;
    },
    setName(val: string) {
      name = val;
      modified = true;
    },
    setCode(val: string) {
      previousCode = code;
      code = val;
      modified = true;
    },
    undoLastCodeChange() {
      if (previousCode !== null) {
        code = previousCode;
        previousCode = null;
        modified = true;
      }
    },
    setFilePath(val: string | null) {
      filePath = val;
    },
    addMessage(msg: ChatMessage) {
      messages = [...messages, msg];
      modified = true;
    },
    setMessages(msgs: ChatMessage[]) {
      messages = msgs;
    },
    setModified(val: boolean) {
      modified = val;
    },
    reset() {
      name = 'Untitled Project';
      code = DEFAULT_CODE;
      previousCode = null;
      messages = [];
      modified = false;
      filePath = null;
    },
  };
}
