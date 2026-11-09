//! Explicit optional download of a pinned model/runtime pack, isolated from providers.
use crate::{resume::ResumeSelection, state::AppState};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};
use sthira_core::{llm::local_resume::LocalResumeModel, resume::ResumeModelIdentity, Engine};
use tauri::State;

#[derive(Clone, Deserialize)]
pub struct Artifact {
    pub url: String,
    pub sha256: String,
    pub bytes: u64,
}
#[derive(Clone, Deserialize)]
pub struct Manifest {
    pub id: String,
    pub label: String,
    pub model: Artifact,
    pub runtime_tag: String,
    pub runtimes: BTreeMap<String, Artifact>,
}
pub fn manifest() -> Result<Manifest, String> {
    serde_json::from_str(include_str!("../resources/resume/pack.json"))
        .map_err(|_| "The Resume pack manifest is invalid.".into())
}
static INSTALLING: AtomicBool = AtomicBool::new(false);
struct DownloadGuard;
impl Drop for DownloadGuard {
    fn drop(&mut self) {
        INSTALLING.store(false, Ordering::SeqCst);
    }
}
#[derive(Clone, Debug, Serialize)]
pub struct PackStatus {
    pub label: String,
    pub ready: bool,
    pub downloading: bool,
    pub supported: bool,
    pub download_bytes: u64,
    pub notice: Option<String>,
}
fn platform() -> String {
    format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH)
}
fn root(engine: &Engine, manifest: &Manifest) -> PathBuf {
    engine
        .vault_dir()
        .join(".sthira_meta/resume_models")
        .join(&manifest.id)
}
fn executable(root: &Path) -> Option<PathBuf> {
    let expected = if cfg!(windows) {
        "llama-server.exe"
    } else {
        "llama-server"
    };
    walkdir::WalkDir::new(root.join("runtime"))
        .max_depth(4)
        .into_iter()
        .filter_map(Result::ok)
        .find(|entry| entry.file_type().is_file() && entry.file_name() == expected)
        .map(|entry| entry.into_path())
}
pub fn status(engine: &Engine) -> Result<PackStatus, String> {
    let manifest = manifest()?;
    let runtime = manifest.runtimes.get(&platform());
    let root = root(engine, &manifest);
    Ok(PackStatus {label:manifest.label,ready:root.join("complete.json").is_file()&&root.join("model.gguf").metadata().is_ok_and(|meta|meta.len()==manifest.model.bytes)&&executable(&root).is_some(),downloading:INSTALLING.load(Ordering::SeqCst),supported:runtime.is_some(),download_bytes:manifest.model.bytes+runtime.map_or(0,|runtime|runtime.bytes),notice:runtime.is_none().then(||"The local Resume pack is not available for this platform yet. Workspace models and factual Resume remain available.".into())})
}
pub fn hash_file(path: &Path) -> Result<String, String> {
    let mut file = std::fs::File::open(path)
        .map_err(|_| "Could not open the Resume pack file.".to_string())?;
    let mut hash = Sha256::new();
    let mut buffer = [0; 65536];
    loop {
        let size = file
            .read(&mut buffer)
            .map_err(|_| "Could not read the Resume pack file.".to_string())?;
        if size == 0 {
            break;
        }
        hash.update(&buffer[..size]);
    }
    Ok(format!("{:x}", hash.finalize()))
}
pub fn selected_model(engine: &Engine) -> Result<ResumeSelection, String> {
    let manifest = manifest()?;
    if !status(engine)?.ready {
        return Err("Download the local Resume pack in Settings → Workflows → Resume, or use a workspace model.".into());
    }
    let root = root(engine, &manifest);
    let executable = executable(&root).ok_or("Local Resume runtime is missing.")?;
    Ok(ResumeSelection {
        model: std::sync::Arc::new(VerifiedLocalModel {
            executable,
            weights: root.join("model.gguf"),
            expected_sha256: manifest.model.sha256.clone(),
            label: manifest.label.clone(),
        }),
        identity: ResumeModelIdentity {
            id: manifest.id,
            name: manifest.label,
            provider_name: "Sthira local pack".into(),
            cache_key: format!("{}:{}", manifest.model.sha256, manifest.runtime_tag),
        },
    })
}
struct VerifiedLocalModel {
    executable: PathBuf,
    weights: PathBuf,
    expected_sha256: String,
    label: String,
}
#[async_trait::async_trait]
impl sthira_core::ModelProvider for VerifiedLocalModel {
    fn id(&self) -> &str {
        &self.label
    }
    fn locality(&self) -> sthira_core::Locality {
        sthira_core::Locality::Local
    }
    async fn generate_json(
        &self,
        schema: &serde_json::Value,
        system: &str,
        prompt: &str,
    ) -> Result<String, sthira_core::CoreError> {
        let path = self.weights.clone();
        let expected = self.expected_sha256.clone();
        tokio::task::spawn_blocking(move||{if hash_file(&path)?!=expected{return Err("The local Resume model failed its integrity check. Remove and download the pack again.".into());}Ok(())}).await.map_err(|_|sthira_core::CoreError::Model("Resume integrity check could not finish.".into()))?.map_err(sthira_core::CoreError::Model)?;
        LocalResumeModel {
            executable: self.executable.clone(),
            weights: self.weights.clone(),
            model_name: self.label.clone(),
        }
        .generate_json(schema, system, prompt)
        .await
    }
    async fn embed(&self, _texts: &[String]) -> Result<Vec<Vec<f32>>, sthira_core::CoreError> {
        Err(sthira_core::CoreError::Model(
            "Resume does not provide embeddings.".into(),
        ))
    }
}

async fn download(
    client: &reqwest::Client,
    artifact: &Artifact,
    path: &Path,
) -> Result<(), String> {
    let mut response = client
        .get(&artifact.url)
        .send()
        .await
        .map_err(|_| {
            "Could not reach the official Resume download. Check your connection and retry."
                .to_string()
        })?
        .error_for_status()
        .map_err(|_| "The official Resume download is unavailable. Try again later.".to_string())?;
    let mut file = std::fs::File::create(path).map_err(|_| {
        "Could not create the model download file. Check disk space and vault permissions."
            .to_string()
    })?;
    let mut hash = Sha256::new();
    let mut bytes = 0u64;
    while let Some(chunk) = response.chunk().await.map_err(|_| {
        "The Resume download was interrupted. Retry to download it again.".to_string()
    })? {
        bytes += chunk.len() as u64;
        if bytes > artifact.bytes {
            return Err("The downloaded file size does not match the pinned Resume pack.".into());
        }
        file.write_all(&chunk)
            .map_err(|_| "Could not save the download. Check disk space.".to_string())?;
        hash.update(&chunk);
    }
    file.sync_all()
        .map_err(|_| "Could not finish saving the Resume download.".to_string())?;
    if bytes != artifact.bytes || format!("{:x}", hash.finalize()) != artifact.sha256 {
        return Err("Resume download verification failed. Retry the download.".into());
    }
    Ok(())
}
pub async fn install(engine: &Engine) -> Result<PackStatus, String> {
    if INSTALLING
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Err("A Resume pack download is already running.".into());
    }
    let _guard = DownloadGuard;
    let manifest = manifest()?;
    let runtime = manifest
        .runtimes
        .get(&platform())
        .ok_or("The Resume runtime is unavailable for this platform.")?;
    if status(engine)?.ready {
        drop(_guard);
        return status(engine);
    }
    let target = root(engine, &manifest);
    let parent = target.parent().ok_or("Invalid Resume pack path.")?;
    std::fs::create_dir_all(parent)
        .map_err(|_| "Could not create the Resume pack directory.".to_string())?;
    let stage = parent.join(format!(".download-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&stage)
        .map_err(|_| "Could not start the Resume pack download.".to_string())?;
    let result:Result<(),String>=async {
        let client=reqwest::Client::builder().timeout(std::time::Duration::from_secs(900)).build().map_err(|_|"Could not create the Resume downloader.".to_string())?;
        download(&client,runtime,&stage.join("runtime.archive")).await?;download(&client,&manifest.model,&stage.join("model.gguf")).await?;
        let runtime_dir=stage.join("runtime");std::fs::create_dir(&runtime_dir).map_err(|_|"Could not prepare the runtime directory.".to_string())?;
        let listing=tokio::process::Command::new("tar").arg("-tf").arg(stage.join("runtime.archive")).kill_on_drop(true).output().await.map_err(|_|"The system archive tool (tar) is needed to install the local Resume runtime.".to_string())?;
        if !listing.status.success() || String::from_utf8_lossy(&listing.stdout).lines().any(|line|Path::new(line).is_absolute()||Path::new(line).components().any(|part|matches!(part,std::path::Component::ParentDir|std::path::Component::Prefix(_)))){return Err("The Resume runtime archive has invalid paths.".into());}
        let extracted=tokio::process::Command::new("tar").arg("-xf").arg(stage.join("runtime.archive")).arg("-C").arg(&runtime_dir).kill_on_drop(true).output().await.map_err(|_|"Could not unpack the local Resume runtime.".to_string())?;
        if !extracted.status.success()||executable(&stage).is_none(){return Err("The local Resume runtime could not be installed.".into());}
        std::fs::write(stage.join("MODEL_LICENSE.txt"),include_str!("../resources/resume/MODEL_LICENSE.txt")).map_err(|_|"Could not save the model license.".to_string())?;
        std::fs::write(stage.join("complete.json"),serde_json::json!({"model_sha256":manifest.model.sha256,"runtime_sha256":runtime.sha256}).to_string()).map_err(|_|"Could not finish the Resume installation.".to_string())?;
        // Only replace this app-owned pack, never provider models or user documents.
        if target.exists(){std::fs::remove_dir_all(&target).map_err(|_|"Remove the incomplete Resume pack and retry.".to_string())?;}
        std::fs::rename(&stage,&target).map_err(|_|"Could not activate the Resume pack.".to_string())?;Ok(())
    }.await;
    if result.is_err() {
        let _ = std::fs::remove_dir_all(&stage);
    }
    result?;
    drop(_guard);
    status(engine)
}
pub fn remove(engine: &Engine) -> Result<PackStatus, String> {
    if INSTALLING.load(Ordering::SeqCst) {
        return Err("Wait for the Resume download to finish before removing it.".into());
    }
    let manifest = manifest()?;
    let root = root(engine, &manifest);
    if root.exists() {
        std::fs::remove_dir_all(root).map_err(|_| {
            "Could not remove the Resume pack. Close an active Resume request and retry."
                .to_string()
        })?;
    }
    status(engine)
}
#[tauri::command]
pub fn cmd_get_resume_pack_status(state: State<'_, AppState>) -> Result<PackStatus, String> {
    status(&state.engine)
}
#[tauri::command]
pub async fn cmd_install_resume_pack(state: State<'_, AppState>) -> Result<PackStatus, String> {
    install(&state.engine).await
}
#[tauri::command]
pub fn cmd_remove_resume_pack(state: State<'_, AppState>) -> Result<PackStatus, String> {
    remove(&state.engine)
}
