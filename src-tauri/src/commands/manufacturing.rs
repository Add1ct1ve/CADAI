use serde::{Deserialize, Serialize};
use tauri::State;

use crate::error::AppError;
use crate::python::runner;
use crate::state::AppState;

#[derive(Serialize)]
pub struct Export3mfResult {
    pub path: String,
    pub triangles: u64,
}

#[derive(Serialize)]
pub struct MeshCheckResult {
    pub watertight: bool,
    pub winding_consistent: bool,
    pub degenerate_faces: u64,
    pub euler_number: i64,
    pub volume: f64,
    pub triangle_count: u64,
    pub issues: Vec<String>,
}

#[derive(Serialize)]
pub struct OrientResult {
    pub rotation: [f64; 3],
    pub height: f64,
    pub overhang_pct: f64,
    pub base_area: f64,
    pub candidates_evaluated: u32,
}

#[derive(Serialize)]
pub struct UnfoldResult {
    pub path: String,
    pub face_count: u32,
    pub bend_count: u32,
    pub flat_width: f64,
    pub flat_height: f64,
}

#[derive(Serialize, Deserialize)]
pub struct ColorInfo {
    pub r: f64,
    pub g: f64,
    pub b: f64,
    pub a: f64,
}

#[tauri::command]
pub async fn export_3mf(
    code: String,
    output_path: String,
    colors: Option<Vec<ColorInfo>>,
    state: State<'_, AppState>,
) -> Result<Export3mfResult, AppError> {
    let venv_path = state.venv_path.lock().unwrap().clone();
    let venv_dir = match venv_path {
        Some(p) => p,
        None => {
            return Err(AppError::CadError(
                "Python environment not set up. Click 'Setup Python' in settings.".into(),
            ));
        }
    };

    let script = super::find_python_script("manufacturing.py")?;

    let temp_dir = std::env::temp_dir().join("cadai-studio");
    std::fs::create_dir_all(&temp_dir)?;
    let code_file = temp_dir.join("mfg_code.py");
    std::fs::write(&code_file, &code)?;

    let code_file_s = code_file.to_string_lossy().to_string();
    let mut args: Vec<String> = vec![
        "export_3mf".into(),
        code_file_s.clone(),
        output_path.clone(),
    ];

    // Write colors to temp file if provided
    let colors_file = temp_dir.join("mfg_colors.json");
    if let Some(ref color_list) = colors {
        let colors_json = serde_json::to_string(color_list)?;
        std::fs::write(&colors_file, &colors_json)?;
        let colors_file_s = colors_file.to_string_lossy().to_string();
        args.push("--colors".into());
        args.push(colors_file_s);
    }

    let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let result = runner::execute_python_script(&venv_dir, &script, &arg_refs)?;

    // Cleanup
    let _ = std::fs::remove_file(&code_file);
    let _ = std::fs::remove_file(&colors_file);

    if result.exit_code != 0 {
        let msg = match result.exit_code {
            2 => format!("Build123d execution error:\n{}", result.stderr),
            3 => "Code must assign final geometry to 'result' variable.".to_string(),
            4 => format!("Export error:\n{}", result.stderr),
            5 => "Missing dependency (trimesh). Will auto-install on next attempt.".to_string(),
            _ => format!(
                "Manufacturing error (exit code {}):\n{}",
                result.exit_code, result.stderr
            ),
        };
        return Err(AppError::CadError(msg));
    }

    // Parse JSON from stdout
    let parsed: serde_json::Value = serde_json::from_str(result.stdout.trim())
        .map_err(|e| AppError::CadError(format!("Failed to parse result: {}", e)))?;

    Ok(Export3mfResult {
        path: parsed["path"].as_str().unwrap_or(&output_path).to_string(),
        triangles: parsed["triangles"].as_u64().unwrap_or(0),
    })
}

#[tauri::command]
pub async fn mesh_check(
    code: String,
    state: State<'_, AppState>,
) -> Result<MeshCheckResult, AppError> {
    let venv_path = state.venv_path.lock().unwrap().clone();
    let venv_dir = match venv_path {
        Some(p) => p,
        None => {
            return Err(AppError::CadError(
                "Python environment not set up. Click 'Setup Python' in settings.".into(),
            ));
        }
    };

    let script = super::find_python_script("manufacturing.py")?;

    let temp_dir = std::env::temp_dir().join("cadai-studio");
    std::fs::create_dir_all(&temp_dir)?;
    let code_file = temp_dir.join("mfg_check_code.py");
    std::fs::write(&code_file, &code)?;

    let code_file_s = code_file.to_string_lossy().to_string();
    let args: Vec<&str> = vec!["mesh_check", &code_file_s];

    let result = runner::execute_python_script(&venv_dir, &script, &args)?;

    let _ = std::fs::remove_file(&code_file);

    if result.exit_code != 0 {
        let msg = match result.exit_code {
            2 => format!("Build123d execution error:\n{}", result.stderr),
            3 => "Code must assign final geometry to 'result' variable.".to_string(),
            4 => format!("Mesh check error:\n{}", result.stderr),
            5 => "Missing dependency (trimesh). Will auto-install on next attempt.".to_string(),
            _ => format!(
                "Manufacturing error (exit code {}):\n{}",
                result.exit_code, result.stderr
            ),
        };
        return Err(AppError::CadError(msg));
    }

    let parsed: serde_json::Value = serde_json::from_str(result.stdout.trim())
        .map_err(|e| AppError::CadError(format!("Failed to parse result: {}", e)))?;

    Ok(MeshCheckResult {
        watertight: parsed["watertight"].as_bool().unwrap_or(false),
        winding_consistent: parsed["winding_consistent"].as_bool().unwrap_or(false),
        degenerate_faces: parsed["degenerate_faces"].as_u64().unwrap_or(0),
        euler_number: parsed["euler_number"].as_i64().unwrap_or(0),
        volume: parsed["volume"].as_f64().unwrap_or(0.0),
        triangle_count: parsed["triangle_count"].as_u64().unwrap_or(0),
        issues: parsed["issues"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default(),
    })
}

#[tauri::command]
pub async fn orient_for_print(
    code: String,
    state: State<'_, AppState>,
) -> Result<OrientResult, AppError> {
    let venv_path = state.venv_path.lock().unwrap().clone();
    let venv_dir = match venv_path {
        Some(p) => p,
        None => {
            return Err(AppError::CadError(
                "Python environment not set up. Click 'Setup Python' in settings.".into(),
            ));
        }
    };

    let script = super::find_python_script("manufacturing.py")?;

    let temp_dir = std::env::temp_dir().join("cadai-studio");
    std::fs::create_dir_all(&temp_dir)?;
    let code_file = temp_dir.join("mfg_orient_code.py");
    std::fs::write(&code_file, &code)?;

    let code_file_s = code_file.to_string_lossy().to_string();
    let args: Vec<&str> = vec!["orient", &code_file_s];

    let result = runner::execute_python_script(&venv_dir, &script, &args)?;

    let _ = std::fs::remove_file(&code_file);

    if result.exit_code != 0 {
        let msg = match result.exit_code {
            2 => format!("Build123d execution error:\n{}", result.stderr),
            3 => "Code must assign final geometry to 'result' variable.".to_string(),
            4 => format!("Orientation analysis error:\n{}", result.stderr),
            5 => {
                "Missing dependency (trimesh/scipy). Will auto-install on next attempt.".to_string()
            }
            _ => format!(
                "Manufacturing error (exit code {}):\n{}",
                result.exit_code, result.stderr
            ),
        };
        return Err(AppError::CadError(msg));
    }

    let parsed: serde_json::Value = serde_json::from_str(result.stdout.trim())
        .map_err(|e| AppError::CadError(format!("Failed to parse result: {}", e)))?;

    let rotation = parsed["rotation"]
        .as_array()
        .map(|arr| {
            let mut r = [0.0f64; 3];
            for (i, v) in arr.iter().enumerate().take(3) {
                r[i] = v.as_f64().unwrap_or(0.0);
            }
            r
        })
        .unwrap_or([0.0, 0.0, 0.0]);

    Ok(OrientResult {
        rotation,
        height: parsed["height"].as_f64().unwrap_or(0.0),
        overhang_pct: parsed["overhang_pct"].as_f64().unwrap_or(0.0),
        base_area: parsed["base_area"].as_f64().unwrap_or(0.0),
        candidates_evaluated: parsed["candidates_evaluated"].as_u64().unwrap_or(0) as u32,
    })
}

#[tauri::command]
pub async fn sheet_metal_unfold(
    code: String,
    output_path: String,
    thickness: Option<f64>,
    state: State<'_, AppState>,
) -> Result<UnfoldResult, AppError> {
    let venv_path = state.venv_path.lock().unwrap().clone();
    let venv_dir = match venv_path {
        Some(p) => p,
        None => {
            return Err(AppError::CadError(
                "Python environment not set up. Click 'Setup Python' in settings.".into(),
            ));
        }
    };

    let script = super::find_python_script("manufacturing.py")?;

    let temp_dir = std::env::temp_dir().join("cadai-studio");
    std::fs::create_dir_all(&temp_dir)?;
    let code_file = temp_dir.join("mfg_unfold_code.py");
    std::fs::write(&code_file, &code)?;

    let code_file_s = code_file.to_string_lossy().to_string();
    let thickness_s = thickness.unwrap_or(1.0).to_string();
    let mut args: Vec<String> = vec!["unfold".into(), code_file_s, output_path.clone()];
    if thickness.is_some() {
        args.push("--thickness".into());
        args.push(thickness_s);
    }

    let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let result = runner::execute_python_script(&venv_dir, &script, &arg_refs)?;

    let _ = std::fs::remove_file(&code_file);

    if result.exit_code != 0 {
        let msg = match result.exit_code {
            2 => format!("Build123d execution error:\n{}", result.stderr),
            3 => "Code must assign final geometry to 'result' variable.".to_string(),
            4 => format!("Unfold error:\n{}", result.stderr),
            5 => "Missing dependency (ezdxf). Will auto-install on next attempt.".to_string(),
            _ => format!(
                "Manufacturing error (exit code {}):\n{}",
                result.exit_code, result.stderr
            ),
        };
        return Err(AppError::CadError(msg));
    }

    let parsed: serde_json::Value = serde_json::from_str(result.stdout.trim())
        .map_err(|e| AppError::CadError(format!("Failed to parse result: {}", e)))?;

    let success = parsed["success"].as_bool().unwrap_or(false);
    if !success {
        let error = parsed["error"]
            .as_str()
            .unwrap_or("Unknown unfold error")
            .to_string();
        return Err(AppError::CadError(error));
    }

    Ok(UnfoldResult {
        path: parsed["path"].as_str().unwrap_or(&output_path).to_string(),
        face_count: parsed["face_count"].as_u64().unwrap_or(0) as u32,
        bend_count: parsed["bend_count"].as_u64().unwrap_or(0) as u32,
        flat_width: parsed["flat_width"].as_f64().unwrap_or(0.0),
        flat_height: parsed["flat_height"].as_f64().unwrap_or(0.0),
    })
}
