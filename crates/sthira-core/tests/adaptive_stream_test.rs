use sthira_core::{
    engines::streaming_engine::{generate_simulated_token_stream, AdaptiveStreamParser},
    llm::ProgressiveStreamChunk,
    OperationalMode,
};

#[test]
fn streams_every_character_without_hiding_prose_or_generating_ui() {
    let mut parser = AdaptiveStreamParser::new();
    let mut chunks = Vec::new();
    let input =
        "# Plan\n\n- [ ] Read evidence\n\n| A | B |\n|---|---|\n| One | Two |\n\nChoice: A or B?";
    for character in input.chars() {
        parser.feed(&character.to_string(), &mut chunks);
    }
    parser.finish(&mut chunks, 90);
    let text = chunks
        .iter()
        .filter_map(|chunk| {
            if let ProgressiveStreamChunk::TextToken(text) = chunk {
                Some(text.as_str())
            } else {
                None
            }
        })
        .collect::<String>();
    assert_eq!(text, input);
    assert_eq!(parser.accumulated_text(), input);
    assert_eq!(chunks.len(), input.chars().count() + 1);
    assert!(matches!(
        chunks.last(),
        Some(ProgressiveStreamChunk::Complete {
            duration_ms: 90,
            ..
        })
    ));
}
#[test]
fn legacy_and_malformed_fences_remain_literal_copyable_stream_content() {
    for input in [
        "Before\n```widget:plan\n{bad JSON}\n```\nAfter",
        "<widget:decision>\n未完成 / Café 🧠\n",
        "````text\n```widget:plan\n{}\n```\n````",
    ] {
        let mut parser = AdaptiveStreamParser::new();
        let mut chunks = vec![];
        for character in input.chars() {
            parser.feed(&character.to_string(), &mut chunks);
        }
        parser.finish(&mut chunks, 10);
        assert_eq!(parser.accumulated_text(), input);
        assert_eq!(parser.fallback_count(), 0);
        assert!(chunks.iter().all(|chunk| matches!(
            chunk,
            ProgressiveStreamChunk::TextToken(_) | ProgressiveStreamChunk::Complete { .. }
        )));
    }
}
#[test]
fn simulated_fixtures_use_markdown_without_ui_instructions() {
    for mode in [OperationalMode::Plan, OperationalMode::Research] {
        let text = generate_simulated_token_stream("Example", mode).concat();
        assert!(text.contains("Example"));
        assert!(!text.contains("widget:"));
    }
}
