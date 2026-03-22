use std::process::Command;
use tauri::Manager;
use tauri::Emitter;

// ── Verifica se ffmpeg e ffprobe estão instalados ──────────────────────────
#[tauri::command]
fn check_dependencies() -> serde_json::Value {
    let ffmpeg = which("ffmpeg");
    let ffprobe = which("ffprobe");

    serde_json::json!({
        "ffmpeg": ffmpeg,
        "ffprobe": ffprobe,
        "ok": ffmpeg && ffprobe
    })
}

fn which(bin: &str) -> bool {
    Command::new("which")
        .arg(bin)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

// ── Retorna o caminho do binário easybrawto embutido ───────────────────────
fn easybrawto_path(_app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    let target = std::env::consts::ARCH;
    let os = std::env::consts::OS;

    let suffix = match (os, target) {
        ("macos", "aarch64") => "aarch64-apple-darwin",
        ("macos", "x86_64")  => "x86_64-apple-darwin",
        ("linux", "x86_64")  => "x86_64-unknown-linux-gnu",
        _ => return Err(format!("Plataforma não suportada: {}-{}", os, target)),
    };

    let bin_name = format!("easybrawto-{}", suffix);

    // Dev: busca em src-tauri/binaries/
    #[cfg(debug_assertions)]
    {
        let dev_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("binaries")
            .join(&bin_name);

        if dev_path.exists() {
            return Ok(dev_path);
        }
        return Err(format!("Binário não encontrado: {}", dev_path.display()));
    }

    // Release: usa Resource dir do bundle
    #[cfg(not(debug_assertions))]
    {
        _app.path()
            .resolve(format!("binaries/{}", bin_name), tauri::path::BaseDirectory::Resource)
            .map_err(|e| e.to_string())
    }
}

// ── Comando principal: captura screenshot + gera vídeo ────────────────────
#[tauri::command]
async fn record(
    app: tauri::AppHandle,
    url: String,
    output: String,
    speed: f64,
    fps: u32,
) -> Result<serde_json::Value, String> {
    let eb_path = easybrawto_path(&app)?;

    let screenshot_path = format!("/tmp/easyscrollrec_{}.png", rand_id());

    emit_progress(&app, "screenshot", "Capturando página...");

    let eb_status = Command::new(&eb_path)
        .args(["fullscreenshot", &url, &screenshot_path])
        .status()
        .map_err(|e| format!("Erro ao executar easybrawto: {}", e))?;

    if !eb_status.success() {
        return Err("easybrawto falhou ao capturar a página".to_string());
    }

    emit_progress(&app, "ffprobe", "Lendo dimensões da imagem...");

    let ffprobe_out = Command::new("ffprobe")
        .args([
            "-v", "error",
            "-show_entries", "stream=height",
            "-of", "csv=p=0",
            &screenshot_path,
        ])
        .output()
        .map_err(|e| format!("Erro ao executar ffprobe: {}", e))?;

    let height_str = String::from_utf8_lossy(&ffprobe_out.stdout)
        .trim()
        .to_string();
    let height: f64 = height_str
        .lines()
        .next()
        .unwrap_or("1080")
        .parse()
        .unwrap_or(1080.0);

    let viewport: f64 = 1080.0;
    let scroll = (height - viewport).max(0.0);
    let duration = scroll / speed;
    let total = duration + 2.0;

    let y_expr = format!(
        "if(gte(t,1),min({scroll},((t-1)*{speed})),0)",
        scroll = scroll,
        speed = speed
    );

    emit_progress(&app, "ffmpeg", "Gerando vídeo...");

    let ffmpeg_status = Command::new("ffmpeg")
        .args([
            "-loop", "1",
            "-framerate", &fps.to_string(),
            "-i", &screenshot_path,
            "-vf", &format!(
                "crop=1920:{}:0:'{}',format=yuv420p",
                viewport as u32,
                y_expr
            ),
            "-t", &format!("{:.2}", total),
            "-c:v", "libx264",
            "-crf", "18",
            "-preset", "fast",
            "-y",
            &output,
        ])
        .status()
        .map_err(|e| format!("Erro ao executar ffmpeg: {}", e))?;

    let _ = std::fs::remove_file(&screenshot_path);

    if !ffmpeg_status.success() {
        return Err("ffmpeg falhou ao gerar o vídeo".to_string());
    }

    emit_progress(&app, "done", "Concluído!");

    Ok(serde_json::json!({
        "ok": true,
        "output": output,
        "height": height,
        "duration": total
    }))
}

// ── Abre pasta do arquivo no Finder/Files ─────────────────────────────────
#[tauri::command]
fn reveal_in_finder(path: String) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    Command::new("open")
        .args(["-R", &path])
        .spawn()
        .map_err(|e| e.to_string())?;

    #[cfg(target_os = "linux")]
    Command::new("xdg-open")
        .arg(
            std::path::Path::new(&path)
                .parent()
                .unwrap_or(std::path::Path::new("/")),
        )
        .spawn()
        .map_err(|e| e.to_string())?;

    Ok(())
}

// ── Helpers ───────────────────────────────────────────────────────────────
fn emit_progress(app: &tauri::AppHandle, step: &str, message: &str) {
    let _ = app.emit("progress", serde_json::json!({
        "step": step,
        "message": message
    }));
}

fn rand_id() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

// ── Entry point ───────────────────────────────────────────────────────────
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_log::Builder::new().build())
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            check_dependencies,
            record,
            reveal_in_finder,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}