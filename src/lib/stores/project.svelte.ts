import type { ChatMessage } from '$lib/types';

const DEFAULT_CODE = `import cadquery as cq

# Create your 3D model here
result = cq.Workplane("XY").box(10, 10, 10)
`;

let name = $state('Untitled Project');
let code = $state(DEFAULT_CODE);
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
    setName(val: string) {
      name = val;
      modified = true;
    },
    setCode(val: string) {
      code = val;
      modified = true;
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
      messages = [];
      modified = false;
      filePath = null;
    },
  };
}
