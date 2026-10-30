use sthira_core::{
    engines::streaming_engine::{AdaptiveStreamParser, generate_simulated_token_stream},
    llm::streaming::ProgressiveStreamChunk,
    OperationalMode,
};

#[test]
fn test_adaptive_stream_parser_accumulates_plain_tokens() {
    let mut parser = AdaptiveStreamParser::new();
    let mut output = Vec::new();

    parser.feed("Hello ", &mut output);
    parser.feed("world! ", &mut output);
    parser.feed("Streaming plain text tokens.", &mut output);

    let mut final_chunks = Vec::new();
    parser.finish(&mut final_chunks, 120);

    // Verify all chunks are text tokens plus final Complete
    let all_chunks: Vec<_> = output.into_iter().chain(final_chunks.into_iter()).collect();
    assert!(!all_chunks.is_empty());

    let has_complete = all_chunks.iter().any(|c| matches!(c, ProgressiveStreamChunk::Complete { .. }));
    assert!(has_complete, "Stream must emit Complete chunk upon finish");

    if let Some(ProgressiveStreamChunk::Complete { total_tokens, duration_ms, basis_fingerprint }) =
        all_chunks.into_iter().find(|c| matches!(c, ProgressiveStreamChunk::Complete { .. }))
    {
        assert_eq!(total_tokens, 3);
        assert_eq!(duration_ms, 120);
        assert!(!basis_fingerprint.is_empty(), "Basis fingerprint must be calculated via Sha256");
    }
}

#[test]
fn test_adaptive_stream_parser_parses_valid_plan_phase_widget() {
    let mut parser = AdaptiveStreamParser::new();
    let mut output = Vec::new();

    parser.feed("Here is the architectural plan:\n\n", &mut output);

    let plan_json = r#"```widget:plan
{
  "phase_number": 1,
  "title": "Phase 1: Database Contracts",
  "milestones": [
    "Formalize SQLite schema",
    "Define change set invariants"
  ],
  "prerequisites": []
}
```"#;

    parser.feed(plan_json, &mut output);

    let mut final_chunks = Vec::new();
    parser.finish(&mut final_chunks, 150);

    let all_chunks: Vec<_> = output.into_iter().chain(final_chunks.into_iter()).collect();

    // Verify PlanPhase chunk was emitted
    let plan_chunk = all_chunks.iter().find(|c| matches!(c, ProgressiveStreamChunk::PlanPhase { .. }));
    assert!(plan_chunk.is_some(), "Valid plan widget must emit PlanPhase chunk");

    if let Some(ProgressiveStreamChunk::PlanPhase { phase_number, title, milestones, prerequisites }) = plan_chunk {
        assert_eq!(*phase_number, 1);
        assert_eq!(title, "Phase 1: Database Contracts");
        assert_eq!(milestones.len(), 2);
        assert_eq!(milestones[0], "Formalize SQLite schema");
        assert!(prerequisites.is_empty());
    }

    assert_eq!(parser.fallback_count(), 0, "No fallback notices should be emitted on valid JSON");
}

#[test]
fn test_adaptive_stream_parser_parses_comparison_and_decision_widgets() {
    let mut parser = AdaptiveStreamParser::new();
    let mut output = Vec::new();

    let comparison_json = r#"```widget:comparison
{
  "criteria": "Throughput Latency",
  "aspect_a": "In-memory channel",
  "aspect_b": "TCP socket loopback",
  "verdict": "In-memory channel achieves sub-millisecond dispatch"
}
```"#;

    let decision_json = r#"```widget:decision
{
  "id": "ipc_transport",
  "question": "Which IPC protocol should be used for streaming?",
  "options": [
    "Tauri Channel",
    "WebSocket",
    "SSE"
  ],
  "recommended": "Tauri Channel"
}
```"#;

    parser.feed(comparison_json, &mut output);
    parser.feed("\n\n", &mut output);
    parser.feed(decision_json, &mut output);

    let mut final_chunks = Vec::new();
    parser.finish(&mut final_chunks, 200);

    let all_chunks: Vec<_> = output.into_iter().chain(final_chunks.into_iter()).collect();

    let comp_chunk = all_chunks.iter().find(|c| matches!(c, ProgressiveStreamChunk::ComparisonRow { .. }));
    assert!(comp_chunk.is_some(), "Must parse ComparisonRow chunk");

    if let Some(ProgressiveStreamChunk::ComparisonRow { criteria, aspect_a, aspect_b, verdict }) = comp_chunk {
        assert_eq!(criteria, "Throughput Latency");
        assert_eq!(aspect_a, "In-memory channel");
        assert_eq!(aspect_b, "TCP socket loopback");
        assert_eq!(verdict.as_deref(), Some("In-memory channel achieves sub-millisecond dispatch"));
    }

    let dec_chunk = all_chunks.iter().find(|c| matches!(c, ProgressiveStreamChunk::DecisionPoint { .. }));
    assert!(dec_chunk.is_some(), "Must parse DecisionPoint chunk");

    if let Some(ProgressiveStreamChunk::DecisionPoint { id, question, options, recommended }) = dec_chunk {
        assert_eq!(id, "ipc_transport");
        assert_eq!(question, "Which IPC protocol should be used for streaming?");
        assert_eq!(options.len(), 3);
        assert_eq!(recommended.as_deref(), Some("Tauri Channel"));
    }
}

#[test]
fn test_adaptive_stream_fail_safe_parser_gracefully_degrades_on_malformed_json() {
    let mut parser = AdaptiveStreamParser::new();
    let mut output = Vec::new();

    // Malformed JSON missing quotes, curly braces, and trailing commas
    let malformed_widget = "```widget:plan\n{ invalid_json: not_quoted, phase: 123 \n```";

    parser.feed(malformed_widget, &mut output);

    let mut final_chunks = Vec::new();
    parser.finish(&mut final_chunks, 100);

    let all_chunks: Vec<_> = output.into_iter().chain(final_chunks.into_iter()).collect();

    // Invariant: Fail-Safe Parser must NEVER panic or abort.
    // It must emit FallbackNotice and fallback to TextToken containing the raw string.
    assert!(parser.fallback_count() > 0, "Fallback count must increment on malformed JSON");

    let fallback_notice = all_chunks.iter().find(|c| matches!(c, ProgressiveStreamChunk::FallbackNotice { .. }));
    assert!(fallback_notice.is_some(), "Must emit FallbackNotice on malformed widget JSON");

    let has_raw_text = all_chunks.iter().any(|c| match c {
        ProgressiveStreamChunk::TextToken(text) => text.contains("invalid_json"),
        _ => false,
    });
    assert!(has_raw_text, "Raw unparsed text must be preserved as TextToken");
}

#[test]
fn test_generate_simulated_token_stream_plan_and_research_modes() {
    let plan_tokens = generate_simulated_token_stream("Build resilient distributed queue", OperationalMode::Plan);
    assert!(!plan_tokens.is_empty());
    let joined_plan = plan_tokens.join("");
    assert!(joined_plan.contains("Phase 1: Architecture, Invariants & Schemas"));
    assert!(joined_plan.contains("widget:plan"));
    assert!(joined_plan.contains("widget:decision"));

    let research_tokens = generate_simulated_token_stream("Compare SQLite WAL mode vs rollback journal", OperationalMode::Research);
    assert!(!research_tokens.is_empty());
    let joined_research = research_tokens.join("");
    assert!(joined_research.contains("widget:comparison"));
    assert!(joined_research.contains("Synthesized Research & Comparative Evidence"));
}

#[test]
fn test_adaptive_stream_parser_utf8_multibyte_safety() {
    let mut parser = AdaptiveStreamParser::new();
    let mut output = Vec::new();

    // These exact characters caused panics in unaligned byte slicing:
    // '🧪' (4 bytes: F0 9F A6 AA)
    // '\u{202f}' (3 bytes: E2 80 AF)
    // '‑' (3 bytes: E2 80 91)
    let tokens = [
        "Phase 1: 🧪 Research ",
        "and Analysis‑Plan\u{202f}for ",
        "the trip planner ",
        "architecture.\n\n",
        "Key highlights:\n",
        "- Point 1: 🚀 Fast and reliable\n",
        "- Point 2: ⚡ Sub-5ms resumption\n",
        "- Point 3: 🌐 Deep search & quotes\n",
    ];

    for tok in tokens {
        parser.feed(tok, &mut output);
    }

    let mut final_chunks = Vec::new();
    parser.finish(&mut final_chunks, 100);

    let all_chunks: Vec<_> = output.into_iter().chain(final_chunks.into_iter()).collect();
    assert!(!all_chunks.is_empty());

    let accumulated = parser.accumulated_text();
    assert!(accumulated.contains("🧪"));
    assert!(accumulated.contains("Analysis‑Plan"));
    assert!(accumulated.contains("\u{202f}"));
    assert!(accumulated.contains("🚀"));
    assert!(accumulated.contains("⚡"));
}

#[test]
fn test_adaptive_stream_markdown_auto_hydration_and_explicit_widgets() {
    // 1. Test Standard Markdown auto-hydration (fallback for small models like llama3.2)
    let mut parser = AdaptiveStreamParser::new();
    let mut output = Vec::new();

    let standard_markdown = r#"# Distributed System Plan

## Phase 1: Architecture & Data Contracts
- [ ] Setup DB schema and SQLite migrations
- [ ] Define strict zero-unsolicited-writes invariants

## Phase 2: Core Progressive Streaming Engine
- [ ] Implement fail-safe token buffer parser
- [ ] Wire Webview IPC sender stream

| Model | Speed | Latency |
| :--- | :--- | :--- |
| llama3.2 | Fast | 45ms |
| deepseek-r1 | Deep | 320ms |

## Decisions
Which database engine should be used?
- SQLite WAL (recommended)
- Remote Postgres
"#;

    parser.feed(standard_markdown, &mut output);

    let mut final_chunks = Vec::new();
    parser.finish(&mut final_chunks, 150);

    let all_chunks: Vec<_> = output.into_iter().chain(final_chunks.into_iter()).collect();

    // Verify PlanPhase chunks auto-hydrated from Markdown headings & checklists
    let plan_phases: Vec<_> = all_chunks.iter().filter_map(|c| match c {
        ProgressiveStreamChunk::PlanPhase { phase_number, title, milestones, .. } => {
            Some((*phase_number, title.clone(), milestones.clone()))
        }
        _ => None,
    }).collect();

    assert_eq!(plan_phases.len(), 2, "Must extract 2 PlanPhase chunks from Markdown headings");
    assert_eq!(plan_phases[0].0, 1);
    assert!(plan_phases[0].1.contains("Architecture & Data Contracts"));
    assert_eq!(plan_phases[0].2.len(), 2);
    assert!(plan_phases[0].2[0].contains("Setup DB schema"));

    assert_eq!(plan_phases[1].0, 2);
    assert!(plan_phases[1].1.contains("Core Progressive Streaming Engine"));

    // Verify ComparisonRow chunks auto-hydrated from Markdown table
    let comparison_rows: Vec<_> = all_chunks.iter().filter_map(|c| match c {
        ProgressiveStreamChunk::ComparisonRow { criteria, aspect_a, aspect_b, .. } => {
            Some((criteria.clone(), aspect_a.clone(), aspect_b.clone()))
        }
        _ => None,
    }).collect();

    assert_eq!(comparison_rows.len(), 2, "Must extract 2 ComparisonRow chunks from Markdown table");
    assert_eq!(comparison_rows[0].0, "llama3.2");
    assert_eq!(comparison_rows[0].1, "Fast");
    assert_eq!(comparison_rows[0].2, "45ms");
    assert_eq!(comparison_rows[1].0, "deepseek-r1");
    assert_eq!(comparison_rows[1].1, "Deep");
    assert_eq!(comparison_rows[1].2, "320ms");

    // Verify DecisionPoint chunk auto-hydrated from ## Decisions
    let decision_points: Vec<_> = all_chunks.iter().filter_map(|c| match c {
        ProgressiveStreamChunk::DecisionPoint { question, options, recommended, .. } => {
            Some((question.clone(), options.clone(), recommended.clone()))
        }
        _ => None,
    }).collect();

    assert!(!decision_points.is_empty(), "Must extract DecisionPoint chunk from ## Decisions");
    assert!(decision_points[0].1.len() >= 2);
    assert_eq!(decision_points[0].2.as_deref(), Some("SQLite WAL (recommended)"));

    // 2. Test explicit widget blocks produce PlanPhase and ComparisonRow directly
    let mut explicit_parser = AdaptiveStreamParser::new();
    let mut explicit_output = Vec::new();

    let explicit_plan = r#"```widget:plan
{
  "phase_number": 1,
  "title": "Phase 1: Architecture",
  "milestones": ["Setup DB"],
  "prerequisites": []
}
```"#;
    let explicit_comparison = r#"```widget:comparison
{
  "criteria": "Speed",
  "aspect_a": "Fast",
  "aspect_b": "Slow",
  "verdict": "Fast is better"
}
```"#;

    explicit_parser.feed(explicit_plan, &mut explicit_output);
    explicit_parser.feed("\n\n", &mut explicit_output);
    explicit_parser.feed(explicit_comparison, &mut explicit_output);

    let mut explicit_final = Vec::new();
    explicit_parser.finish(&mut explicit_final, 80);

    let explicit_all: Vec<_> = explicit_output.into_iter().chain(explicit_final.into_iter()).collect();
    assert!(explicit_all.iter().any(|c| matches!(c, ProgressiveStreamChunk::PlanPhase { .. })));
    assert!(explicit_all.iter().any(|c| matches!(c, ProgressiveStreamChunk::ComparisonRow { .. })));
}

#[test]
fn test_adaptive_stream_real_world_weight_loss_plan_hydration() {
    let raw_weight_loss_markdown = r#"As an expert systems architect, I will treat your weight loss goal as a **Biological Optimization Project**. To move from 81kg to a leaner state while maintaining muscle mass (via 80g of protein) on a vegetarian diet, we need a phased deployment plan.
System Analysis (The Baseline)
•
**Current State:** 81kg | 175cm (5'9")
•
**Target Metric:** Weight loss (Caloric Deficit)
•
**Constraint A:** Vegetarian (No meat/fish)
•
**Constraint B:** $\ge 80g$ Protein/day (to prevent muscle atrophy)
---
Phase 1: Caloric Budgeting & Macro Schema
Before selecting foods, we must define the "Energy Budget." For your height and weight, your Maintenance Calories (TDEE) are roughly 2,200–2,400 kcal (assuming sedentary/light activity). To lose weight safely, we will implement a **500 kcal deficit**.
**Architectural Invariant:** The "Protein Floor." No matter the caloric fluctuation, protein must remain $\ge 80g$. This ensures you lose fat, not muscle.
---
Phase 2: Protein Source Engineering
The primary challenge in vegetarian diets is the "Protein-to-Calorie Ratio." We need "high-density" proteins that don't blow the calorie budget.
**The "Protein Stack" for your plan:**
1. **Greek Yogurt/Paneer (Low Fat):** High bioavailability.
2. **Tofu/Tempeh:** Complete plant proteins.
3. **Lentils/Chickpeas:** Fiber + Protein (Careful with carb count).
4. **Whey/Pea Protein:** Efficient "top-off" to hit 80g without excess calories.
---
Phase 3: The Daily Deployment (Meal Plan)
We will distribute the 1,800 kcal and 80g protein across four "execution windows."
#### The Proposed Blueprint:
Meal	Component	Est. Calories	Est. Protein
**Breakfast**	Greek Yogurt (200g) + Berries + 10g Almonds	300 kcal	22g
**Lunch**	100g Tofu/Paneer + 1 cup Sautéed Spinach/Broccoli + 1/2 cup Brown Rice	500 kcal	25g
**Snack**	1 Apple + 1 Scoop Whey/Pea Protein (with water)	250 kcal	25g
**Dinner**	1 cup Lentil Soup (Dal) + Large Mixed Green Salad (Lemon dressing)	450 kcal	15g
**Total**		**~1,500 - 1,700 kcal**	**~87g**
---
Phase 4: Monitoring & Iteration (The Feedback Loop)
Weight loss is not linear. We must implement a "Telemetry" system to adjust the plan based on actual biological response.
**Trade-offs & Decisions:**
•
**The Carbohydrate Trade-off:** To keep protein high and calories low, we must limit "empty" carbs (white bread, sugar). We trade these for "complex" carbs (quinoa, oats) to maintain insulin stability.
Final System Summary
1. **Calorie Target:** $\approx 1,700$ kcal.
2. **Protein Target:** $80g - 90g$ (Prioritize Tofu, Greek Yogurt, and Protein Powder).
3. **Water Invariant:** Minimum 3L per day to assist kidney function during high protein intake.
4. **Activity Modifier:** If you add weight training, increase calories by 200 (via complex carbs) to support recovery."#;

    let mut parser = AdaptiveStreamParser::new();
    let mut output = Vec::new();

    parser.feed(raw_weight_loss_markdown, &mut output);

    let mut final_chunks = Vec::new();
    parser.finish(&mut final_chunks, 120);

    let all_chunks: Vec<_> = output.into_iter().chain(final_chunks.into_iter()).collect();

    let phases: Vec<_> = all_chunks.iter().filter_map(|c| match c {
        ProgressiveStreamChunk::PlanPhase { phase_number, title, milestones, .. } => {
            Some((*phase_number, title.clone(), milestones.clone()))
        }
        _ => None,
    }).collect();

    assert!(phases.len() >= 4, "Must extract at least 4 phases from real-world plan, got: {}", phases.len());
    assert!(phases[0].1.contains("Caloric Budgeting"));
    assert!(phases[1].1.contains("Protein Source Engineering"));
    assert!(phases[2].1.contains("Daily Deployment"));
    assert!(phases[3].1.contains("Monitoring & Iteration"));

    // Verify numbered milestones under Phase 2 are captured
    assert!(!phases[1].2.is_empty(), "Phase 2 must capture numbered protein items as milestones");
    assert!(phases[1].2.iter().any(|m| m.contains("Greek Yogurt") || m.contains("Tofu")));

    // Verify meal table was extracted into ComparisonRow chunks
    let comparison_rows: Vec<_> = all_chunks.iter().filter_map(|c| match c {
        ProgressiveStreamChunk::ComparisonRow { criteria, aspect_a, .. } => {
            Some((criteria.clone(), aspect_a.clone()))
        }
        _ => None,
    }).collect();

    assert!(comparison_rows.len() >= 3, "Must extract comparison rows from meal plan table, got: {}", comparison_rows.len());

    // Verify decision point was extracted
    let decisions: Vec<_> = all_chunks.iter().filter_map(|c| match c {
        ProgressiveStreamChunk::DecisionPoint { question, .. } => {
            Some(question.clone())
        }
        _ => None,
    }).collect();

    eprintln!("DECISIONS FOUND: {:?}", decisions);
    eprintln!("ALL CHUNKS: {:?}", all_chunks.iter().map(|c| match c {
        ProgressiveStreamChunk::PlanPhase { title, .. } => format!("Phase: {}", title),
        ProgressiveStreamChunk::ComparisonRow { criteria, .. } => format!("Comp: {}", criteria),
        ProgressiveStreamChunk::DecisionPoint { question, .. } => format!("Dec: {}", question),
        ProgressiveStreamChunk::TextToken(t) => format!("Text: {} bytes", t.len()),
        ProgressiveStreamChunk::Complete { .. } => "Complete".to_string(),
        ProgressiveStreamChunk::FallbackNotice { reason: f } => format!("Fallback: {}", f),
    }).collect::<Vec<_>>());

    assert!(!decisions.is_empty(), "Must extract decision point from Trade-offs section");
}

