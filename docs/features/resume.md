# Resume

Resume is an explicit action in a project overview and in project-document headers. Opening a project or document does not invoke it. Reading a Resume shows a valid cached selection or current factual working state; Summarize / Refresh summary is the only model dispatch. Compact and expanded views use the same cache.

Factual Resume needs no model, opt-in, network or download. It shows saved document references, unfinished checklist entries, verified current recorded decisions/questions, and an exact continuation document. It never infers that two different decisions conflict, creates a proposal, changes knowledge, or edits a note. The editor saves a dirty draft before opening Resume; save failure keeps that draft and blocks the transition.

The Resume pane exposes its configured model and connection before generation, plus an inline selector. Generated/cached output retains its producing model/provider label independently of the current selection.

Settings → Workflows → Resume controls model summaries, model choice, structured details, document excerpts, and the maximum number of documents. Model choice starts with the enabled workspace default. A specific override, local pack, and factual-only mode are available. The optional local fallback applies when no workspace default is configured, not as an invisible substitution for an unreachable explicitly chosen model. A requested cloud model receives the bounded excerpts; there is no automatic background generation.

The source packet contains current project members only, with stable artifact IDs, a bounded number of documents and a bounded excerpt budget. Source text is untrusted context. Model output selects short source-verbatim passages with exact quote evidence and source IDs. Validation rejects missing/unknown citations, changed wording, new numbers and malformed output; it does not claim semantic completeness. Model-selected passages retain AI attribution and source links. Saved factual state remains separately inspectable even after successful generation.

The cache is derived SQLite data in `resume_brief_cache`, separate from documents, proposals and legacy memory mirrors. Its basis includes source identities/content, valid node evidence, project purpose, included details, generator version and selected provider/model identity. Simple access timestamps are not progress. Reads update factual continuation information without another inference request. Changed sources during inference reject the result; changed settings/model choices prevent delivery of an old selection. Failure leaves current facts or a valid saved selection visible with an explanatory state. Settings persist in `resume_preferences` and the existing Resume workflow preference, migrated additively in schema version 18.

## Local packaging decision

Offer SmolLM2-360M Q8 as an optional **local preview pack**, not a mandatory addition to the installer. The download is approximately 410 MB including the Linux x86_64 CPU runtime; installed runtime files require additional disk space. It is independent of Ollama/LM Studio and does not populate the provider catalog. The pack works offline after installation and the runtime is started on demand with two CPU threads, an 8,192-token context, and a bounded response. The child exits after the request, including cancellation/failure paths.

The model and runtime URLs, revisions, byte sizes and SHA-256 digests are pinned in `crates/sthira-desktop/resources/resume/pack.json`. Installation verifies both downloads, unpacks only the app-owned stage, preserves runtime/model licenses and activates a complete pack. Interrupted or invalid downloads leave the existing workspace intact and can be retried. Removal affects this pack only. System `tar` is required. Runtime artifacts are listed for Linux, macOS and Windows architectures; only Linux x86_64 was executed during this implementation. No binary weights are committed to the repository.

## Why this candidate

The reproducible comparison uses `fixtures/resume/cases.json` and `sthira-cli`'s `resume_model_eval` binary, using the production local adapter, prompt, schema and quote validator. Both official Q8 files ran on the same pinned llama.cpp CPU runtime with two threads, no reasoning, seed 42, temperature 0.2, and process startup per request. Seven cases invoke a real model; an eighth empty-evidence case exercises a factual bypass.

A stricter extractive prompt was tested after the initial prompt allowed malformed/repetitive output or omissions. The initial results are retained in temporary evaluation output rather than represented as passing. The selection comparison below is a small implementation study, not a general benchmark:

| Candidate | Model file | Content checks, seven inference cases | Median request including startup | Observed peak child RSS |
|---|---:|---:|---:|---:|
| SmolLM2-360M Q8 | 386 MB | 5/7 | 3.82 s | about 749 MiB |
| Qwen3-0.6B Q8 | 639 MB | 4/7 | 5.06 s | about 1,610 MiB |

Both passed quote/format grounding on six of seven inference cases. Neither passed all completeness/instruction-content checks, so the smaller, faster candidate is optional and labeled preview. Unverified selections fall back to facts; verified selections can still omit useful detail. Factual decisions/tasks are not replaced by the model output. These timings exclude downloading and the installed-pack integrity scan. CPU results from this machine do not predict every supported platform, context size or language. SmolLM2 is primarily English; multilingual quality remains unverified.

Upstream artifacts: [SmolLM2](https://huggingface.co/HuggingFaceTB/SmolLM2-360M-Instruct-GGUF), [Qwen3](https://huggingface.co/Qwen/Qwen3-0.6B-GGUF), [llama.cpp b12110](https://github.com/ggml-org/llama.cpp/releases/tag/b12110). Model license: Apache-2.0; runtime includes its upstream license.

Build and run the comparison with `cargo build -p sthira-cli --bin resume_model_eval`, then `target/debug/resume_model_eval /path/to/llama-server /path/to/model.gguf MODEL_LABEL /tmp/result.json`. Use isolated synthetic cases, never the owner's vault. The explicit native smoke test downloads the selected pack into a temporary vault, runs real inference, removes it, and preserves the saved project: `cargo test -p sthira-desktop --test resume_test real_local_pack_download_and_inference_smoke -- --ignored --nocapture`.
