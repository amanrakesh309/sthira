# Workflow configuration

## Product decisions

Capture preserves the user's words and works without a model. Research investigates questions, Plan produces steps, and Draft produces a deliverable. The existing `act` protocol value remains compatible; its visible label is Draft. Resume belongs to existing work, not the composer.

AI workflows have separate enabled and configured states. Fresh settings leave Research, Plan, Draft and document chat off until enabled by the user. Existing documents and conversation history remain readable. Disabling a workflow does not delete content or cancel an already dispatched request. Model choice resolves explicit task/document selection, then workflow preference, then workspace default. An unavailable explicit choice is an error.

Research/Plan template controls require the workflow and its template option to be enabled. Turning templates off uses a general output structure; it must not silently apply the saved specialized template. Resume supports explicitly requested model-selected excerpts, dedicated source/model settings, a shared cache, and a factual fallback. Automatic summaries and new background jobs remain off. The local Resume pack is an optional explicit download; external providers remain separate. Existing saved tracker activity is not silently deleted or reconfigured.

## Implementation sequence

1. Settings foundation: persisted workflow switches/model defaults, honest setup states, one settings navigation, immediate-save feedback, Draft naming and generation guards. Preserve provider configuration and stored conversations.
2. Template library: native create/edit/duplicate/delete/default APIs, rich structure preview, general built-ins and one manager linked from settings and workflow selectors. Changing templates only affects future runs; preserve old documents.
3. Resume: implemented as described in [Resume](resume.md), with source-validated excerpts, current factual state, on-demand generation, one shared cache, source/model/settings fingerprints, and optional local inference.
4. Optional automation: explicit background/cloud permission, batching, resource limits and per-job scheduling. No continuously resident model by default.

Each phase requires relevant UI/native integration checks and fresh isolated browser evidence. Live provider and native GUI checks must be reported separately.
