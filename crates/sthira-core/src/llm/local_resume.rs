use crate::{CoreError, Locality, ModelProvider};
use std::{path::PathBuf, process::Stdio};

/// A bounded, on-demand child runtime. It is killed when a request completes,
/// times out, errors, or is cancelled; it never shares the user's AI app service.
pub struct LocalResumeModel {
    pub executable: PathBuf,
    pub weights: PathBuf,
    pub model_name: String,
}

#[async_trait::async_trait]
impl ModelProvider for LocalResumeModel {
    fn id(&self) -> &str {
        &self.model_name
    }
    fn locality(&self) -> Locality {
        Locality::Local
    }
    async fn generate_json(
        &self,
        schema: &serde_json::Value,
        system_prompt: &str,
        user_prompt: &str,
    ) -> Result<String, CoreError> {
        static GENERATION: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
        let _guard = GENERATION.lock().await;
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .map_err(|_| CoreError::Model("Could not reserve a local Resume port.".into()))?
            .local_addr()
            .map_err(|_| CoreError::Model("Could not read the local Resume port.".into()))?
            .port();
        let token = uuid::Uuid::new_v4().to_string();
        let mut child = tokio::process::Command::new(&self.executable)
            .arg("--model")
            .arg(&self.weights)
            .args([
                "--host",
                "127.0.0.1",
                "--port",
                &port.to_string(),
                "--api-key",
                &token,
                "--ctx-size",
                "8192",
                "--threads",
                "2",
                "--parallel",
                "1",
                "--n-gpu-layers",
                "0",
                "--no-warmup",
                "--no-webui",
                "--no-slots",
                "--reasoning",
                "off",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .map_err(|_| {
                CoreError::Model(
                    "The local Resume runtime could not start. Reinstall its download pack.".into(),
                )
            })?;
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(45))
            .build()
            .map_err(|error| CoreError::Model(error.to_string()))?;
        let base = format!("http://127.0.0.1:{port}");
        let result=async {
            let ready=tokio::time::timeout(std::time::Duration::from_secs(20),async {
                loop {
                    if child.try_wait().map_err(|_|CoreError::Model("Local Resume runtime stopped.".into()))?.is_some() {return Err(CoreError::Model("The local Resume runtime is incompatible with this computer or model pack.".into()));}
                    if client.get(format!("{base}/health")).timeout(std::time::Duration::from_secs(1)).send().await.is_ok_and(|response|response.status().is_success()) {return Ok(());}
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                }
            }).await.map_err(|_|CoreError::Model("Local Resume model loading timed out.".into()))?;
            ready?;
            let response=client.post(format!("{base}/v1/chat/completions")).bearer_auth(&token).json(&serde_json::json!({"model":"local-resume","messages":[{"role":"system","content":system_prompt},{"role":"user","content":user_prompt}],"temperature":0.2,"seed":42,"max_tokens":768,"chat_template_kwargs":{"enable_thinking":false},"response_format":{"type":"json_schema","json_schema":{"name":"resume","strict":true,"schema":schema}}})).send().await.map_err(|_|CoreError::Model("Local Resume generation failed.".into()))?;
            if !response.status().is_success() {return Err(CoreError::Model("The local Resume runtime rejected this request.".into()));}
            let body:serde_json::Value=response.json().await.map_err(|_|CoreError::Model("Local Resume returned an invalid response.".into()))?;
            body["choices"][0]["message"]["content"].as_str().map(str::to_string).ok_or_else(||CoreError::Model("Local Resume returned no text.".into()))
        }.await;
        let _ = child.kill().await;
        let _ = child.wait().await;
        result
    }
    async fn embed(&self, _texts: &[String]) -> Result<Vec<Vec<f32>>, CoreError> {
        Err(CoreError::Model(
            "The built-in Resume model does not provide embeddings.".into(),
        ))
    }
}
