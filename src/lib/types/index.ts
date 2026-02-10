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
  ollama_base_url: string | null;
  openai_base_url: string | null;
  agent_rules_preset: string | null;
  enable_code_review: boolean;
  display_units: 'mm' | 'inch';
  grid_size: number;
  grid_spacing: number;
  snap_translate: number | null;
  snap_rotation: number | null;
  snap_sketch: number | null;
}

export interface ModelInfo {
  id: string;
  display_name: string;
}

export interface ProviderInfo {
  id: string;
  display_name: string;
  requires_api_key: boolean;
  base_url: string | null;
  models: ModelInfo[];
  allows_custom_model: boolean;
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
  event_type?: string;
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

// Multi-part parallel generation types

export interface GenerationPlan {
  mode: 'single' | 'multi';
  description?: string;
  parts: PartSpec[];
}

export interface PartSpec {
  name: string;
  description: string;
  position: [number, number, number];
  constraints: string[];
}

export type MultiPartEvent =
  | { kind: 'DesignPlan'; plan_text: string }
  | { kind: 'PlanStatus'; message: string }
  | { kind: 'PlanResult'; plan: GenerationPlan }
  | { kind: 'SingleDelta'; delta: string; done: boolean }
  | { kind: 'SingleDone'; full_response: string }
  | { kind: 'PartDelta'; part_index: number; part_name: string; delta: string }
  | { kind: 'PartComplete'; part_index: number; part_name: string; success: boolean; error?: string }
  | { kind: 'AssemblyStatus'; message: string }
  | { kind: 'FinalCode'; code: string }
  | { kind: 'ReviewStatus'; message: string }
  | { kind: 'ReviewComplete'; was_modified: boolean; explanation: string }
  | { kind: 'Done'; success: boolean; error?: string };

export interface PartProgress {
  name: string;
  status: 'pending' | 'generating' | 'complete' | 'failed';
  streamedText: string;
  error?: string;
}

export interface ProjectFile {
  name: string;
  code: string;
  messages: RustChatMessage[];
  version: number;
  scene?: {
    objects: import('$lib/types/cad').SceneObject[];
    codeMode: import('$lib/types/cad').CodeMode;
    camera: import('$lib/types/cad').CameraState;
    sketches?: import('$lib/types/cad').Sketch[];
  };
}
