export interface ChatMessage {
  id: string;
  role: 'user' | 'assistant' | 'system';
  content: string;
  timestamp: number;
  code?: string; // extracted code block
  isError?: boolean; // whether this is an error message
  failedCode?: string; // the code that caused the error (for retry)
  errorMessage?: string; // the raw error text (for retry)
  retryAttempt?: number; // which retry attempt this is (1-3)
  codeUpdatedByAi?: boolean; // whether code was updated via auto-retry
}

export interface AppConfig {
  ai_provider: string;
  api_key: string | null;
  model: string;
  python_path: string | null;
  theme: string;
}

export interface ViewportState {
  isLoading: boolean;
  hasModel: boolean;
  error: string | null;
}

export interface ProjectState {
  name: string;
  code: string;
  messages: ChatMessage[];
  modified: boolean;
}

export interface ExecuteResult {
  stl_base64: string | null;
  stdout: string;
  stderr: string;
  success: boolean;
}

export interface PythonStatus {
  python_found: boolean;
  python_version: string | null;
  python_path: string | null;
  venv_ready: boolean;
  cadquery_installed: boolean;
}

export interface StreamEvent {
  delta: string;
  done: boolean;
}

export interface RustChatMessage {
  role: string;
  content: string;
}

export interface AutoRetryResult {
  new_code: string | null;
  ai_response: string;
  attempt: number;
  success: boolean;
}

export interface StructuredError {
  error_type: string;
  message: string;
  line_number: number | null;
  suggestion: string | null;
}

export interface ProjectFile {
  name: string;
  code: string;
  messages: RustChatMessage[];
  version: number;
}
