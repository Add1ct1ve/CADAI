export interface ExecutionRequest {
  code: string;
  timeoutMs?: number;
}

export interface ExecutionArtifacts {
  stl_base64: string | null;
}

export interface ExecutionTiming {
  duration_ms: number;
  timeout_ms: number;
}

export interface CadExecutionResult {
  success: boolean;
  artifacts: ExecutionArtifacts;
  stdout: string;
  stderr: string;
  logs: string[];
  timing: ExecutionTiming;
  error: string | null;
  // Backward compatibility for existing callers while migrating to artifacts.
  stl_base64: string | null;
}

