use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;
use sthira_core::{
    ArtifactType, Authorship, Clock, CoreError, Engine, Locality, ModelProvider,
};

struct SystemClock;
impl Clock for SystemClock {
    fn now_ms(&self) -> i64 {
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64
    }
}

struct CliModel;
#[async_trait::async_trait]
impl ModelProvider for CliModel {
    fn id(&self) -> &str {
        "sthira-cli-local"
    }

    fn locality(&self) -> Locality {
        Locality::Local
    }

    async fn generate_json(
        &self,
        _schema: &serde_json::Value,
        _system_prompt: &str,
        _user_prompt: &str,
    ) -> Result<String, CoreError> {
        Ok("{}".to_string())
    }

    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, CoreError> {
        let dim = 128;
        let mut results = Vec::with_capacity(texts.len());
        for text in texts {
            let mut vec = vec![0.0f32; dim];
            for (i, b) in text.as_bytes().iter().enumerate() {
                vec[i % dim] += *b as f32 / 255.0;
            }
            let norm: f32 = vec.iter().map(|v| v * v).sum::<f32>().sqrt();
            if norm > 0.0 {
                for v in &mut vec {
                    *v /= norm;
                }
            }
            results.push(vec);
        }
        Ok(results)
    }
}

fn print_usage() {
    println!(
        r#"Sthira Headless Core Intelligence & Local-First Storage Engine CLI

USAGE:
    sthira-cli [OPTIONS] <COMMAND>

COMMANDS:
    home                            View active project threads, loose notes, and pending proposals
    capture <text>                  Capture a spontaneous loose thought
    route <query>                   Classify input intent using the read-only router
    project list                    List all active project threads
    project new <name> [purpose]    Create a new project thread with optional purpose
    resume <project_id> [level]     Display resumption brief and pane hint (level: 1 or 2)
    history <project_id>            Display project history milestones and evolution
    reconcile                       Reconcile filesystem Markdown files with SQLite database
    audit                           Run live multi-model benchmark against Ollama endpoints
    help                            Display this help message

OPTIONS:
    --db <path>                     Path to SQLite database (default: ./sthira.db)
    --vault <path>                  Path to vault directory (default: ./.sthira_vault/artifacts)
"#
    );
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 || args[1] == "help" || args[1] == "--help" || args[1] == "-h" {
        print_usage();
        return Ok(());
    }

    let mut db_path = PathBuf::from("sthira.db");
    let mut vault_dir = PathBuf::from(".sthira_vault").join("artifacts");

    let mut cmd_idx = 1;
    while cmd_idx < args.len() {
        if args[cmd_idx] == "--db" && cmd_idx + 1 < args.len() {
            db_path = PathBuf::from(&args[cmd_idx + 1]);
            cmd_idx += 2;
        } else if args[cmd_idx] == "--vault" && cmd_idx + 1 < args.len() {
            vault_dir = PathBuf::from(&args[cmd_idx + 1]);
            cmd_idx += 2;
        } else {
            break;
        }
    }

    if cmd_idx >= args.len() {
        print_usage();
        return Ok(());
    }

    let clock = Arc::new(SystemClock);
    let model = Arc::new(CliModel);
    let engine = Engine::open(&db_path, &vault_dir, clock.clone(), model)?;

    let command = &args[cmd_idx];
    match command.as_str() {
        "home" => {
            let state = engine.get_home_state()?;
            println!("=== Sthira Home State ===");
            println!("Active Project Threads ({}):", state.threads.len());
            for t in &state.threads {
                println!(
                    "  - [{}] {} (Last active: {})",
                    &t.project_id[..8.min(t.project_id.len())],
                    t.name,
                    t.last_substantive_at
                );
                println!("    Purpose: {}", t.purpose);
            }
            println!("\nLoose Captures ({}):", state.loose_captures.len());
            for c in &state.loose_captures {
                let title = c.title.as_deref().unwrap_or("Untitled Thought");
                let preview: String = c.content_md.chars().take(60).collect();
                println!(
                    "  - [{}] {}: {}...",
                    &c.id[..8.min(c.id.len())],
                    title,
                    preview.replace('\n', " ")
                );
            }
            println!("\nPending AI Proposals: {}", state.pending_proposals_count);
        }

        "capture" => {
            if cmd_idx + 1 >= args.len() {
                eprintln!("Error: capture requires note text\nUsage: sthira-cli capture <text>");
                std::process::exit(1);
            }
            let text = args[cmd_idx + 1..].join(" ");
            let artifact = engine.create_artifact(
                ArtifactType::Note,
                Authorship::User,
                Some("Quick Capture"),
                &text,
                None,
            )?;
            println!("Captured thought [{}]: {}", &artifact.id[..8], text);
        }

        "route" => {
            if cmd_idx + 1 >= args.len() {
                eprintln!("Error: route requires an input query\nUsage: sthira-cli route <query>");
                std::process::exit(1);
            }
            let query = args[cmd_idx + 1..].join(" ");
            let plan = engine.route_input(&query).await?;
            println!("=== Router Plan ===");
            println!("Intent: {:?}", plan.intent_type);
            println!("Confidence: {:.2}", plan.confidence);
            println!("Mutates Immediately: {}", plan.mutates_immediately());
            if let Some(ref target) = plan.target_project_id {
                println!("Target Project ID: {}", target);
            }
            println!(
                "Payload: {}",
                serde_json::to_string_pretty(&plan.payload)?
            );
        }

        "project" => {
            if cmd_idx + 1 >= args.len() {
                eprintln!("Error: project command requires a subcommand (new | list)");
                std::process::exit(1);
            }
            let subcmd = &args[cmd_idx + 1];
            match subcmd.as_str() {
                "list" => {
                    let state = engine.get_home_state()?;
                    println!("=== Active Projects ===");
                    for t in &state.threads {
                        println!("- ID: {}", t.project_id);
                        println!("  Name: {}", t.name);
                        println!("  Purpose: {}", t.purpose);
                    }
                }
                "new" => {
                    if cmd_idx + 2 >= args.len() {
                        eprintln!("Error: project new requires a name\nUsage: sthira-cli project new <name> [purpose]");
                        std::process::exit(1);
                    }
                    let name = &args[cmd_idx + 2];
                    let purpose = if cmd_idx + 3 < args.len() {
                        Some(args[cmd_idx + 3..].join(" "))
                    } else {
                        None
                    };

                    let proj = engine.create_project(name, None)?;
                    if let Some(ref p) = purpose {
                        engine.set_project_purpose(&proj.id, p)?;
                    }
                    println!("Created project [{}] '{}'", proj.id, proj.name);
                }
                other => {
                    eprintln!("Unknown project subcommand: {}", other);
                    std::process::exit(1);
                }
            }
        }

        "resume" => {
            if cmd_idx + 1 >= args.len() {
                eprintln!("Error: resume requires a project_id\nUsage: sthira-cli resume <project_id> [level]");
                std::process::exit(1);
            }
            let project_id = &args[cmd_idx + 1];
            let level: i64 = if cmd_idx + 2 < args.len() {
                args[cmd_idx + 2].parse().unwrap_or(1)
            } else {
                1
            };

            let hint = engine.get_resume_pane_hint(project_id)?;
            let brief = engine.get_resume_brief(project_id, level).await?;

            println!("=== Resume Brief (Level {}) ===", brief.level);
            println!("Breadcrumb: {}", brief.active_working_breadcrumb);
            println!("Pane Hint: {:?}", hint);
            println!("Purpose: {}", brief.purpose);
            if let Some(ref cp) = brief.continuation_point {
                println!("Continuation Point: {}", cp);
            }
            println!("\nRecent Progress (Claims):");
            for claim in &brief.recent_progress {
                println!("  * [{:?}] {}", claim.origin, claim.text);
            }
            if !brief.conflicting_decisions.is_empty() {
                println!("\nConflicting Decisions:");
                for d in &brief.conflicting_decisions {
                    println!("  ! {}", d.text);
                }
            }
        }

        "history" => {
            if cmd_idx + 1 >= args.len() {
                eprintln!("Error: history requires a project_id\nUsage: sthira-cli history <project_id>");
                std::process::exit(1);
            }
            let project_id = &args[cmd_idx + 1];
            let hist = engine.get_project_history(project_id)?;

            println!("=== Project History Milestones ===");
            for m in &hist.milestones {
                println!(
                    "[{}] {:?} ({}): {}",
                    m.attribution.label(),
                    m.milestone_type,
                    m.title,
                    m.description
                );
            }
            println!("\n=== Narrative Segments ===");
            for s in &hist.segments {
                println!("[{}] {}", s.attribution.label(), s.text);
            }
        }

        "reconcile" => {
            println!("Reconciling vault files in {}...", vault_dir.display());
            let result = sthira_core::vault::reconcile_vault(
                engine.db(),
                engine.vault_dir(),
                clock.now_ms(),
                |_conn, _pid, _aid, _actor, _etype, _mat, _delta, _ts| Ok(()),
            )?;
            println!("Reconciled vault successfully:");
            println!("  Files imported: {}", result.files_imported);
            println!("  Files exported: {}", result.files_exported);
            println!("  Conflicts detected: {}", result.conflicts_detected);
            println!("  Renames detected: {}", result.renames_detected);
        }

        "audit" => {
            let status = std::process::Command::new("cargo")
                .args(["run", "--bin", "live_model_audit"])
                .status()?;
            if !status.success() {
                std::process::exit(1);
            }
        }

        other => {
            eprintln!("Unknown command: {}\n", other);
            print_usage();
            std::process::exit(1);
        }
    }

    Ok(())
}
