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
  hasSkippedSteps?: boolean; // triggers retry button for iterative build skipped steps
}

export interface AppConfig {
  ai_provider: string;
  api_key: string | null;
  model: string;
  python_path: string | null;
  theme: string;
  ollama_base_url: string | null;
  openai_base_url: string | null;
  runpod_base_url: string | null;
  agent_rules_preset: string | null;
  enable_code_review: boolean;
  display_units: 'mm' | 'inch';
  grid_size: number;
  grid_spacing: number;
  snap_translate: number | null;
  snap_rotation: number | null;
  snap_sketch: number | null;
  enable_consensus: boolean;
  auto_approve_plan: boolean;
  retrieval_enabled: boolean;
  retrieval_token_budget: number;
  telemetry_enabled: boolean;
  max_validation_attempts: number;
  generation_reliability_profile: 'reliability_first' | 'balanced' | 'fidelity_first';
  preview_on_partial_failure: boolean;
  max_generation_runtime_seconds: number;
  semantic_contract_strict: boolean;
  reviewer_mode: 'advisory_only' | 'rewrite_allowed';
  quality_gates_strict: boolean;
  allow_euler_override: boolean;
  semantic_bbox_mode: 'semantic_aware' | 'legacy';
  mechanisms_enabled: boolean;
  mechanism_import_enabled: boolean;
  mechanism_cache_max_mb: number;
  allowed_spdx_licenses: string[];
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

export type ExecuteResult = import('$lib/types/execution').CadExecutionResult;
export type ExecutionRequest = import('$lib/types/execution').ExecutionRequest;
export type ExecutionTiming = import('$lib/types/execution').ExecutionTiming;
export type ExecutionArtifacts = import('$lib/types/execution').ExecutionArtifacts;

export interface PythonStatus {
  python_found: boolean;
  python_version: string | null;
  python_path: string | null;
  venv_ready: boolean;
  build123d_installed: boolean;
  build123d_version: string | null;
}

export interface StreamEvent {
  delta: string;
  done: boolean;
  event_type?: string;
  token_usage?: TokenUsageData;
}

export interface TokenUsageData {
  input_tokens: number;
  output_tokens: number;
  total_tokens: number;
  cost_usd: number | null;
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
  category: { kind: string; sub_kind?: string };
  failing_operation: string | null;
  context: { source_line: string | null; failing_parameters: string | null } | null;
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

export interface PendingAssemblyPart {
  part_key: string;
  name: string;
  stl_base64: string;
  position: [number, number, number];
}

export type MultiPartEvent =
  | { kind: 'DesignPlan'; plan_text: string }
  | {
      kind: 'PlanValidation';
      risk_score: number;
      warnings: string[];
      is_valid: boolean;
      rejected_reason: string | null;
      fatal_combo: boolean;
      negation_conflict: boolean;
      repair_sensitive_ops: string[];
    }
  | { kind: 'ConfidenceAssessment'; level: 'high' | 'medium' | 'low'; score: number; cookbook_matches: string[]; warnings: string[]; message: string }
  | { kind: 'PlanStatus'; message: string }
  | { kind: 'PlanResult'; plan: GenerationPlan }
  | { kind: 'SingleDelta'; delta: string; done: boolean }
  | { kind: 'SingleDone'; full_response: string }
  | { kind: 'PartDelta'; part_index: number; part_name: string; delta: string }
  | { kind: 'PartComplete'; part_index: number; part_name: string; success: boolean; error?: string }
  | { kind: 'PartCodeExtracted'; part_index: number; part_name: string; code: string }
  | { kind: 'PartStlReady'; part_index: number; part_name: string; stl_base64: string }
  | { kind: 'PartStlFailed'; part_index: number; part_name: string; error: string }
  | { kind: 'AssemblyStatus'; message: string }
  | { kind: 'FinalCode'; code: string; stl_base64?: string }
  | { kind: 'ReviewStatus'; message: string }
  | { kind: 'ReviewComplete'; was_modified: boolean; explanation: string }
  | { kind: 'ValidationAttempt'; attempt: number; max_attempts: number; message: string }
  | { kind: 'StaticValidationReport'; passed: boolean; findings: string[] }
  | { kind: 'ValidationSuccess'; attempt: number; message: string }
  | { kind: 'ValidationFailed'; attempt: number; error_category: string; error_message: string; will_retry: boolean }
  | {
      kind: 'PostGeometryValidationReport';
      report: {
        watertight: boolean;
        manifold: boolean;
        degenerate_faces: number;
        euler_number: number;
        triangle_count: number;
        component_count: number;
        bounds_min: [number, number, number];
        bounds_max: [number, number, number];
        volume: number;
        bbox_ok: boolean;
        warnings: string[];
      };
    }
  | { kind: 'PostGeometryValidationWarning'; message: string }
  | { kind: 'SemanticValidationReport'; part_name: string; passed: boolean; findings: string[] }
  | { kind: 'RetrievalStatus'; message: string; items: { source: string; id: string; title: string; score: number }[]; used_embeddings: boolean; lexical_fallback: boolean }
  | { kind: 'IterativeStart'; total_steps: number; steps: { index: number; name: string; description: string; operations: string[] }[] }
  | { kind: 'IterativeStepStarted'; step_index: number; step_name: string; description: string }
  | { kind: 'IterativeStepComplete'; step_index: number; success: boolean; stl_base64?: string }
  | { kind: 'IterativeStepRetry'; step_index: number; attempt: number; error: string }
  | { kind: 'IterativeStepSkipped'; step_index: number; name: string; error: string }
  | { kind: 'IterativeComplete'; final_code: string; stl_base64?: string; skipped_steps: SkippedStepInfo[] }
  | { kind: 'ModificationDetected'; intent_summary: string }
  | { kind: 'CodeDiff'; diff_lines: DiffLine[]; old_line_count: number; new_line_count: number; additions: number; deletions: number }
  | { kind: 'ConsensusStarted'; candidate_count: number }
  | { kind: 'ConsensusCandidate'; label: string; temperature: number; status: string; has_code?: boolean; execution_success?: boolean }
  | { kind: 'ConsensusWinner'; label: string; score: number; reason: string }
  | { kind: 'ClarificationNeeded'; questions: string[] }
  | { kind: 'TokenUsage'; phase: string; input_tokens: number; output_tokens: number; total_tokens: number; cost_usd: number | null }
  | { kind: 'Done'; success: boolean; error?: string; validated?: boolean };

export interface DiffLine {
  tag: 'equal' | 'insert' | 'delete';
  text: string;
}

export interface PartProgress {
  name: string;
  status: 'pending' | 'generating' | 'complete' | 'failed';
  streamedText: string;
  error?: string;
  description: string;
  constraints: string[];
  position: [number, number, number];
  code?: string;
  stl_base64?: string;
}

export interface IterativeStepProgress {
  index: number;
  name: string;
  description: string;
  status: 'pending' | 'generating' | 'retrying' | 'complete' | 'skipped';
  stl_base64?: string;
  error?: string;
  attempt?: number;
}

export interface SkippedStepInfo {
  step_index: number;
  name: string;
  description: string;
  error: string;
}

export interface DesignPlanResult {
  plan_text: string;
  risk_score: number;
  warnings: string[];
  is_valid: boolean;
  clarification_questions?: string[];
}

export interface GenerationEntry {
  id: string;
  timestamp: number;
  userPrompt: string;
  code: string;
  stl_base64?: string;
  success: boolean;
  error?: string;
  provider: string;
  model: string;
  durationMs: number;
  tokenUsage?: TokenUsageData;
  confidenceScore?: number;
  confidenceLevel?: 'high' | 'medium' | 'low';
  generationType: 'single' | 'multi-part' | 'iterative' | 'modification';
  retryCount: number;
  pinned: boolean;
}

export interface PlanTemplate {
  id: string;
  name: string;
  description: string;
  plan_text: string;
}

export interface MechanismParameter {
  name: string;
  default_value: string;
  description: string;
  unit?: string | null;
}

export interface MechanismItem {
  package_id: string;
  package_name: string;
  package_version: string;
  id: string;
  title: string;
  summary: string;
  category: string;
  keywords: string[];
  prompt_block: string;
  license?: string | null;
  source_url?: string | null;
  preview_url?: string | null;
  parameters: MechanismParameter[];
}

export interface MechanismPackage {
  package_id: string;
  name: string;
  version: string;
  license: string;
  source?: string | null;
  homepage?: string | null;
  mechanism_count: number;
  is_imported: boolean;
}

export interface MechanismListResponse {
  packages: MechanismPackage[];
  mechanisms: MechanismItem[];
}

export interface MechanismImportReport {
  package_id: string;
  package_name: string;
  installed_count: number;
  source_url: string;
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
    featureTree?: import('$lib/stores/feature-tree.svelte').FeatureTreeSnapshot;
    datumPlanes?: import('$lib/types/cad').DatumPlane[];
    datumAxes?: import('$lib/types/cad').DatumAxis[];
    displayMode?: import('$lib/types/cad').DisplayMode;
    components?: import('$lib/types/cad').Component[];
    componentNameCounter?: number;
    mates?: import('$lib/types/cad').AssemblyMate[];
    drawings?: import('$lib/types/drawing').Drawing[];
  };
}
