use serde::Serialize;
use tauri::State;

use crate::error::AppError;
use crate::python::runner;
use crate::state::AppState;

#[derive(Serialize)]
pub struct DrawingViewResult {
    pub svg_content: String,
    pub width: f64,
    pub height: f64,
}

#[tauri::command]
pub async fn generate_drawing_view(
    code: String,
    proj_x: f64,
    proj_y: f64,
    proj_z: f64,
    show_hidden: bool,
    section_plane: Option<String>,
    section_offset: Option<f64>,
    state: State<'_, AppState>,
) -> Result<DrawingViewResult, AppError> {
    let venv_path = state.venv_path.lock().unwrap().clone();

    let venv_dir = match venv_path {
        Some(p) => p,
        None => {
            return Err(AppError::CadError(
                "Python environment not set up. Click 'Setup Python' in settings.".into(),
            ));
        }
    };

    let script = super::find_python_script("drawing_view.py")?;

    // Write code to temp file
    let temp_dir = std::env::temp_dir().join("cadai-studio");
    std::fs::create_dir_all(&temp_dir)?;
    let input_file = temp_dir.join("drawing_input.py");
    let output_svg = temp_dir.join("drawing_output.svg");
    std::fs::write(&input_file, &code)?;
    let _ = std::fs::remove_file(&output_svg);

    // Build args
    let proj_x_s = proj_x.to_string();
    let proj_y_s = proj_y.to_string();
    let proj_z_s = proj_z.to_string();
    let input_s = input_file.to_string_lossy().to_string();
    let output_s = output_svg.to_string_lossy().to_string();

    let mut args: Vec<&str> = vec![&input_s, &output_s, &proj_x_s, &proj_y_s, &proj_z_s];

    if show_hidden {
        args.push("--hidden");
    }

    let section_offset_s;
    if let Some(ref plane) = section_plane {
        args.push("--section");
        args.push(plane.as_str());
        section_offset_s = section_offset.unwrap_or(0.0).to_string();
        args.push(&section_offset_s);
    }

    let result = runner::execute_python_script(&venv_dir, &script, &args)?;

    if result.exit_code != 0 {
        let error_msg = match result.exit_code {
            2 => format!("Build123d execution error:\n{}", result.stderr),
            3 => "Code must assign final geometry to 'result' variable.".to_string(),
            4 => format!("SVG export error:\n{}", result.stderr),
            5 => format!("Section view error:\n{}", result.stderr),
            _ => format!(
                "Python error (exit code {}):\n{}",
                result.exit_code, result.stderr
            ),
        };
        // Cleanup
        let _ = std::fs::remove_file(&input_file);
        let _ = std::fs::remove_file(&output_svg);
        return Err(AppError::CadError(error_msg));
    }

    // Read the generated SVG
    if !output_svg.exists() {
        let _ = std::fs::remove_file(&input_file);
        return Err(AppError::CadError("SVG file was not generated".into()));
    }

    let svg_content = std::fs::read_to_string(&output_svg)?;

    // Parse dimensions from stdout JSON
    let mut width = 100.0;
    let mut height = 100.0;
    if let Ok(dims) = serde_json::from_str::<serde_json::Value>(&result.stdout.trim()) {
        width = dims["width"].as_f64().unwrap_or(100.0);
        height = dims["height"].as_f64().unwrap_or(100.0);
    }

    // Cleanup
    let _ = std::fs::remove_file(&input_file);
    let _ = std::fs::remove_file(&output_svg);

    Ok(DrawingViewResult {
        svg_content,
        width,
        height,
    })
}

#[tauri::command]
pub async fn export_drawing_pdf(
    svg_content: String,
    output_path: String,
    state: State<'_, AppState>,
) -> Result<String, AppError> {
    let venv_path = state.venv_path.lock().unwrap().clone();
    let venv_dir = match venv_path {
        Some(p) => p,
        None => {
            return Err(AppError::CadError(
                "Python environment not set up.".into(),
            ));
        }
    };

    let script = super::find_python_script("drawing_export.py")?;

    let temp_dir = std::env::temp_dir().join("cadai-studio");
    std::fs::create_dir_all(&temp_dir)?;
    let input_svg = temp_dir.join("export_drawing.svg");
    std::fs::write(&input_svg, &svg_content)?;

    let input_s = input_svg.to_string_lossy().to_string();
    let args: Vec<&str> = vec!["pdf", &input_s, &output_path];

    let result = runner::execute_python_script(&venv_dir, &script, &args)?;

    let _ = std::fs::remove_file(&input_svg);

    if result.exit_code != 0 {
        let msg = match result.exit_code {
            2 => "cairosvg not installed. Run: pip install cairosvg".to_string(),
            3 => format!("PDF conversion error:\n{}", result.stderr),
            _ => format!(
                "Export error (exit code {}):\n{}",
                result.exit_code, result.stderr
            ),
        };
        return Err(AppError::CadError(msg));
    }

    Ok(format!("PDF exported to {}", output_path))
}

#[tauri::command]
pub async fn export_drawing_dxf(
    svg_content: String,
    output_path: String,
    state: State<'_, AppState>,
) -> Result<String, AppError> {
    let venv_path = state.venv_path.lock().unwrap().clone();
    let venv_dir = match venv_path {
        Some(p) => p,
        None => {
            return Err(AppError::CadError(
                "Python environment not set up.".into(),
            ));
        }
    };

    let script = super::find_python_script("drawing_export.py")?;

    let temp_dir = std::env::temp_dir().join("cadai-studio");
    std::fs::create_dir_all(&temp_dir)?;
    let input_svg = temp_dir.join("export_drawing.svg");
    std::fs::write(&input_svg, &svg_content)?;

    let input_s = input_svg.to_string_lossy().to_string();
    let args: Vec<&str> = vec!["dxf", &input_s, &output_path];

    let result = runner::execute_python_script(&venv_dir, &script, &args)?;

    let _ = std::fs::remove_file(&input_svg);

    if result.exit_code != 0 {
        let msg = match result.exit_code {
            2 => "ezdxf not installed. Run: pip install ezdxf".to_string(),
            3 => format!("DXF conversion error:\n{}", result.stderr),
            _ => format!(
                "Export error (exit code {}):\n{}",
                result.exit_code, result.stderr
            ),
        };
        return Err(AppError::CadError(msg));
    }

    Ok(format!("DXF exported to {}", output_path))
}
