/// Only the two official setup destinations can be opened by this command.
pub fn ai_app_download_url(app: &str) -> Result<&'static str, String> {
    match app {
        "ollama" => Ok("https://ollama.com/download"),
        "lmstudio" => Ok("https://lmstudio.ai/download"),
        _ => Err("Unknown AI app.".into()),
    }
}

#[tauri::command]
pub fn cmd_open_ai_app_download(app: String) -> Result<(), String> {
    let url = ai_app_download_url(&app)?;
    #[cfg(target_os = "linux")]
    let mut command = std::process::Command::new("xdg-open");
    #[cfg(target_os = "macos")]
    let mut command = std::process::Command::new("open");
    #[cfg(target_os = "windows")]
    let mut command = {
        let mut command = std::process::Command::new("rundll32.exe");
        command.arg("url.dll,FileProtocolHandler");
        command
    };
    let mut child = command.arg(url).spawn().map_err(|_| {
        "Could not open your browser. Visit the app’s official download page.".to_string()
    })?;
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}
