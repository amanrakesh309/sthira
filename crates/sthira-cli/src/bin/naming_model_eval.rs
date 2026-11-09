//! Real-model naming comparison using the production naming prompt and adapter.
use serde::Deserialize;
use sthira_core::{llm::local_resume::LocalResumeModel, naming};
#[derive(Deserialize)]
struct Case {
    id: String,
    content: String,
    topic: Vec<String>,
    qualifiers: Vec<String>,
    forbidden: Vec<String>,
}
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 5 {
        return Err("Usage: naming_model_eval RUNTIME WEIGHTS LABEL OUTPUT.json".into());
    }
    let model = LocalResumeModel {
        executable: args[1].clone().into(),
        weights: args[2].clone().into(),
        model_name: args[3].clone(),
    };
    let cases: Vec<Case> =
        serde_json::from_str(include_str!("../../../../fixtures/naming/cases.json"))?;
    let mut rows = Vec::new();
    for case in cases {
        let start = std::time::Instant::now();
        let result = naming::suggest(&model, &case.content).await;
        let valid = result.is_ok();
        let titles = result.as_ref().cloned().unwrap_or_default();
        let checks = if case.content.is_empty() {
            result.is_err()
        } else {
            valid
                && titles.iter().all(|title| {
                    let lower = title.to_lowercase();
                    case.topic
                        .iter()
                        .any(|term| lower.contains(&term.to_lowercase()))
                        && (case.qualifiers.is_empty()
                            || case
                                .qualifiers
                                .iter()
                                .any(|term| lower.contains(&term.to_lowercase())))
                        && case
                            .forbidden
                            .iter()
                            .all(|term| !lower.contains(&term.to_lowercase()))
                })
        };
        println!("{} {} checks={} {:?}", args[3], case.id, checks, titles);
        rows.push(serde_json::json!({"id":case.id,"elapsed_ms":start.elapsed().as_millis(),"format_valid":valid,"content_checks_passed":checks,"titles":titles,"error":result.err().map(|e|e.to_string())}));
        std::fs::write(
            &args[4],
            serde_json::to_string_pretty(
                &serde_json::json!({"model":args[3],"threads":2,"context":8192,"quantization":"Q8_0","results":rows}),
            )?,
        )?;
    }
    Ok(())
}
