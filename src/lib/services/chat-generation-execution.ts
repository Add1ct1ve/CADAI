import { executeCode, extractPythonCode } from '$lib/services/tauri';

export interface ResolvedExecutionSuccess {
  success: true;
  stlBase64: string | null;
}

export interface ResolvedExecutionFailure {
  success: false;
  error: string;
}

export type ResolvedExecutionResult = ResolvedExecutionSuccess | ResolvedExecutionFailure;

export function resolveGeneratedCode(
  generationResult: string,
  isMultiPart: boolean,
): string | null {
  return isMultiPart ? generationResult : extractPythonCode(generationResult);
}

export async function executeGeneratedCode(
  code: string,
): Promise<ResolvedExecutionResult> {
  try {
    const result = await executeCode(code);
    if (result.success) {
      return {
        success: true,
        stlBase64: result.stl_base64,
      };
    }
    return {
      success: false,
      error: result.error ?? (result.stderr || 'Code execution failed'),
    };
  } catch (err) {
    return {
      success: false,
      error: `${err}`,
    };
  }
}
