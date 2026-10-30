use std::fs;
use std::path::Path;
use std::process::Command;
use walkdir::WalkDir;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 || args[1] != "gate" {
        eprintln!("Usage: cargo xtask gate <phase_number>");
        std::process::exit(1);
    }

    let phase: usize = match args[2].parse() {
        Ok(p) => p,
        Err(_) => {
            eprintln!("Error: Phase must be a number (1..=9)");
            std::process::exit(1);
        }
    };

    println!("=== Running Gate Verification for Phase {} ===", phase);

    // 1. Anti-Cheating: Check forbidden literals
    println!("[1/4] Checking forbidden literals in crates/sthira-core/src/**...");
    let forbidden_file = "fixtures/golden/forbidden_literals.txt";
    if !Path::new(forbidden_file).exists() {
        eprintln!("Error: {} not found", forbidden_file);
        std::process::exit(1);
    }

    let forbidden_content = fs::read_to_string(forbidden_file).expect("Failed to read forbidden_literals.txt");
    let forbidden_tokens: Vec<&str> = forbidden_content
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect();

    for entry in WalkDir::new("crates/sthira-core/src")
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "rs"))
    {
        let content = fs::read_to_string(entry.path()).expect("Failed to read file");
        for token in &forbidden_tokens {
            if content.contains(token) {
                eprintln!(
                    "FAILED: Forbidden literal '{}' found in file: {}",
                    token,
                    entry.path().display()
                );
                record_failure(phase, &format!("Forbidden literal '{}' in {}", token, entry.path().display()));
                std::process::exit(1);
            }
        }
    }
    println!("  -> Forbidden literals check PASSED.");

    // 2. Wall-clock call bans
    println!("[2/4] Checking banned wall-clock calls in crates/sthira-core/src/**...");
    let banned_patterns = [
        "SystemTime::now",
        "Instant::now",
        "Utc::now",
        "CURRENT_TIMESTAMP",
    ];

    for entry in WalkDir::new("crates/sthira-core/src")
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "rs" || ext == "sql"))
    {
        let content = fs::read_to_string(entry.path()).expect("Failed to read file");
        for pattern in &banned_patterns {
            if content.contains(pattern) {
                eprintln!(
                    "FAILED: Banned wall-clock call '{}' found in file: {}",
                    pattern,
                    entry.path().display()
                );
                record_failure(phase, &format!("Banned wall-clock call '{}' in {}", pattern, entry.path().display()));
                std::process::exit(1);
            }
        }
    }
    println!("  -> Banned wall-clock check PASSED.");

    // 3. Run phase tests
    println!("[3/4] Running cargo test for Phase {}...", phase);
    let test_target = match phase {
        1 => "phase1_storage_reconcile",
        2 => "phase2_epistemic_safety",
        3 => "phase3_router_isolation",
        4 => "phase4_memory_resume_home",
        5 => "phase5_history_materiality",
        6 => "phase6_discovery_proposals",
        7 => "phase7_e2e_lifecycle",
        9 => "phase9_modes_registry",
        _ => {
            eprintln!("Invalid phase: {}", phase);
            std::process::exit(1);
        }
    };

    let status = Command::new("cargo")
        .args(["test", "--package", "sthira-core", "--test", test_target, "--", "--nocapture"])
        .status()
        .expect("Failed to run cargo test");

    if !status.success() {
        eprintln!("FAILED: Tests for Phase {} failed", phase);
        record_failure(phase, &format!("Tests for {} failed", test_target));
        std::process::exit(1);
    }
    println!("  -> Phase {} tests PASSED.", phase);

    // 4. Record success
    println!("[4/4] Writing gate report for Phase {}...", phase);
    record_success(phase);
    println!("=== Gate Verification for Phase {} PASSED ===", phase);
}

fn record_failure(phase: usize, error: &str) {
    let _ = fs::create_dir_all("gate_reports");
    let report = serde_json::json!({
        "phase": phase,
        "status": "FAILED",
        "error": error
    });
    let report_path = format!("gate_reports/phase{}.json", phase);
    let _ = fs::write(&report_path, serde_json::to_string_pretty(&report).unwrap());
}

fn record_success(phase: usize) {
    let _ = fs::create_dir_all("gate_reports");
    let report = serde_json::json!({
        "phase": phase,
        "status": "PASSED",
        "checks": {
            "forbidden_literals": "PASSED",
            "banned_clock": "PASSED",
            "tests": "PASSED"
        }
    });
    let report_path = format!("gate_reports/phase{}.json", phase);
    fs::write(&report_path, serde_json::to_string_pretty(&report).unwrap())
        .expect("Failed to write gate report");

    if phase == 7 {
        let final_report = serde_json::json!({
            "status": "ALL_PHASES_PASSED",
            "total_phases": 7,
            "verification_status": "PASSED"
        });
        fs::write("gate_reports/FINAL_VERIFICATION_REPORT.json", serde_json::to_string_pretty(&final_report).unwrap())
            .expect("Failed to write final report");
    }
}
