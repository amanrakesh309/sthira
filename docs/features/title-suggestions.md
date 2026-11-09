# Title suggestions

A saved document offers Suggest a title beside its title area. This reads up to 5,000 saved characters on an explicit request, using the configured Resume model by default. Resume summaries can remain off; naming is a separate explicit request and does not change their setting. The panel shows the resolved model/provider, accepts a per-request connected-model or local-pack override, and retains producing-model labels with the suggestions.

Generation never writes a title. The user reviews one to three bounded suggestions and chooses Use title. Changed drafts disable application. Native application checks saved content hash and prior title under a writer transaction, changes the title and an existing leading H1, keeps the remaining body/identity/authorship, records a user-accepted revision/event, and updates the Markdown mirror before committing. A failed mirror write rolls back the database; changed saved data rejects the suggestion. No automatic or background rename is implemented.

The naming prompt composes short titles and is separate from the source-verbatim Resume prompt. Format validation bounds the output, but cannot certify semantic correctness. Review remains necessary for hedges, completed/unfinished work, names, numbers and source-instruction content.

## Small-model comparison

`fixtures/naming/cases.json` has seven actual inference cases and an empty-document bypass. `naming_model_eval` uses the production naming prompt, parser and on-demand llama.cpp adapter. Both pinned official Q8 weights from the Resume evaluation ran on the same two-thread CPU runtime, with seed 42, temperature 0.2, no reasoning and a fresh child per request.

| Candidate | Valid format | Automated content checks | Median including startup | Observed child peak RSS |
|---|---:|---:|---:|---:|
| SmolLM2-360M Q8 | 5/7 | 3/7 | 1.67 s | about 742 MiB |
| Qwen3-0.6B Q8 | 5/7 | 3/7 | 3.26 s | about 1,603 MiB |

Checks require topic specificity, appropriate proposal/uncertainty words and absence of specified invented claims in every returned suggestion. This rubric is deliberately inspectable, but not semantic ground truth. Manual inspection found that Qwen's passing proposal case still included “Analytics jobs moved … proposal pending,” which can imply completed work. Smol followed a source instruction to produce an unrelated title. Both sometimes copied sentences, exceeded formatting limits or omitted necessary context. The empty-document bypass rejected generation for both and is excluded from the seven-case totals.

Neither candidate warrants automatic naming. Keep the existing optional Smol Resume pack as a preview rather than shipping a second larger model for titles. Let users choose a stronger connected model, review all suggestions, and keep manual names unchanged unless explicitly accepted. These results do not rank the models generally or establish multilingual quality. Timings exclude downloads and installed-pack integrity scans.

Reproduce with `cargo build -p sthira-cli --bin naming_model_eval`, then `target/debug/naming_model_eval /path/to/llama-server /path/to/model.gguf MODEL_LABEL /tmp/naming-results.json`. Use the pinned Resume artifacts and synthetic cases; no owner vault is needed. Generated raw results/logs belong in temporary verification output.
