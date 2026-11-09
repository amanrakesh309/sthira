//! Reproducible real-model comparison using the production Resume adapter/schema.
//! Usage: resume_model_eval <runtime> <weights> <model-label> <output.json>
use serde::{Deserialize, Serialize};
use sthira_core::{
    llm::local_resume::LocalResumeModel,
    resume::{output_schema, validate_output, ResumeSource, RESUME_SYSTEM_PROMPT},
    ModelProvider,
};
#[derive(Deserialize)]
struct Case {
    id: String,
    purpose: String,
    sources: Vec<ResumeSource>,
    required: Vec<String>,
    #[serde(default)]
    one_of: Vec<String>,
    forbidden: Vec<String>,
}
#[derive(Serialize)]
struct ResultRow {
    id: String,
    elapsed_ms: u128,
    grounding_valid: bool,
    content_checks_passed: bool,
    raw: String,
    error: Option<String>,
}
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 5 {
        return Err(
            "Usage: resume_model_eval <runtime> <weights> <model-label> <output.json>".into(),
        );
    }
    let model = LocalResumeModel {
        executable: args[1].clone().into(),
        weights: args[2].clone().into(),
        model_name: args[3].clone(),
    };
    let cases: Vec<Case> =
        serde_json::from_str(include_str!("../../../../fixtures/resume/cases.json"))?;
    let mut results = Vec::new();
    for case in cases {
        let start = std::time::Instant::now();
        let result = if case.sources.is_empty() {
            Ok("{\"items\":[]}".to_string())
        } else {
            tokio::time::timeout(
                std::time::Duration::from_secs(90),
                model.generate_json(
                    &output_schema(),
                    RESUME_SYSTEM_PROMPT,
                    &serde_json::json!({"purpose":case.purpose,"sources":case.sources}).to_string(),
                ),
            )
            .await
            .map_err(|_| "Evaluation timed out".to_string())
            .and_then(|result| result.map_err(|error| error.to_string()))
        };
        let (raw, error) = match result {
            Ok(raw) => (raw, None),
            Err(error) => (String::new(), Some(error)),
        };
        let validation = validate_output(&raw, &case.sources);
        let text = validation
            .as_ref()
            .map(|claims| {
                claims
                    .iter()
                    .map(|claim| claim.text.to_lowercase())
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .unwrap_or_default();
        let passed = validation.is_ok()
            && case
                .required
                .iter()
                .all(|word| text.contains(&word.to_lowercase()))
            && (case.one_of.is_empty()
                || case
                    .one_of
                    .iter()
                    .any(|word| text.contains(&word.to_lowercase())))
            && case
                .forbidden
                .iter()
                .all(|word| !text.contains(&word.to_lowercase()));
        println!(
            "{} {} grounding={} checks={}",
            args[3],
            case.id,
            validation.is_ok(),
            passed
        );
        results.push(ResultRow {
            id: case.id,
            elapsed_ms: start.elapsed().as_millis(),
            grounding_valid: validation.is_ok(),
            content_checks_passed: passed,
            raw,
            error: error.or_else(|| validation.err().map(|error| error.to_string())),
        });
        std::fs::write(
            &args[4],
            serde_json::to_string_pretty(
                &serde_json::json!({"model":args[3],"runtime":args[1],"weights":args[2],"threads":2,"context":8192,"quantization":"Q8_0","generation":"on demand; no thinking; seed 42; temperature 0.2","results":results}),
            )?,
        )?;
    }
    Ok(())
}
