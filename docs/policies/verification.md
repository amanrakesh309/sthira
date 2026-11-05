# Verification and evidence policy

## Honest results

Each claim must state what was executed and at which layer: static inspection, mocked component/JSDOM test, native engine/database integration, browser preview, native GUI, live model/search provider, or platform/assistive technology.

Use PASS, FAIL, BLOCKED, NOT RUN, or NOT APPLICABLE per independently verifiable assertion. Partial evidence requires splitting the assertion. Give exclusions a concrete contract-based reason; do not rewrite a requirement to make it inapplicable. Report counts from actual results and preserve initial failures alongside retests.

A callback assertion does not prove a database write; an engine reopen does not prove native application close/relaunch; JSDOM markup does not prove browser geometry; a test named “live” with a fake model does not prove real inference. Code review and saved screenshots cannot replace unexecuted interactions. Avoid blanket “fully working,” “100% compliant,” or invented performance/fidelity claims.

## Fresh evidence and retention

- Capture evidence from the current source/build and fixture. Record revision plus dirty changes, launch method, layer, fixture identity, viewport/theme/scale, steps, expected/actual result, and command exit code where relevant.
- Never copy an old audit image/log/report into a new run and call it current. Approved design references are **targets**, not execution evidence. If historical comparison is explicitly needed, identify its original build/source/layer separately.
- Store generated logs, screenshots, comparison images, coverage registers, QA databases, and per-run notes in ignored `tmp/verification/<unique-run>/` or an external temporary directory. Do not recreate repository audit albums, dated completion reports, or repeated agent execution prompts.
- Keep only reviewed product decisions in permanent policies, implementation contracts in feature docs, reproducible test source in tests, and unresolved work in [known limitations](../known-limitations.md). Temporary run output is disposable after review. Do not create a hidden evidence archive during cleanup.
- Deleting working-tree evidence does not erase Git history and is not a history-rewrite request. History is never valid evidence of the current build merely because a file can be recovered.
- Do not expose keys, private records, prompts containing user data, or unrelated desktop content in evidence. Inspect the exact final images and ensure links resolve before sharing.

## Isolation and fixtures

- Never use the owner's vault for seed/reset/migration/deletion/reconciliation experiments. Confirm the actual native startup path before mutations. Native desktop uses `STHIRA_VAULT_DIR` when supported; otherwise it may default to the owner's home vault.
- Ensure `STHIRA_SEED_DEMO` is absent for an empty-workspace test; its presence can enable seeding even if its value is `0`.
- Default automated tests to `tempdir()` and fake clocks. A retained fixture is explicit opt-in to a new empty directory; reject a nonempty target. Never delete/replace an arbitrary directory supplied through an environment variable.
- Record actual run-specific IDs and paths. A random UUID is not a deterministic identity. A seed creates the manifest it describes, and a replayable fixture remains available for the intended review duration.
- Put a native-launchable database at the filename/layout the native entry point actually opens. Merely running unrelated suites after `mktemp` does not seed that directory.
- Use user-created synthetic content: empty/source-only/shared projects, notes/objects/sources/snippets/canvases, direct references/backlinks/cycles, derived lineage, draft/hedge/decision/AI evidence, long/Unicode/duplicate titles, wide tables/code, pagination boundaries, and clock/schedule cases. Distinguish seeded data from observed product facts.

Example native launch, after verifying startup and building/serving the correct UI:

```sh
qa_vault_dir="$(mktemp -d "${TMPDIR:-/tmp}/sthira-qa.XXXXXX")"
env -u STHIRA_SEED_DEMO STHIRA_VAULT_DIR="$qa_vault_dir" cargo run -p sthira-desktop
```

For a populated retained fixture, create a separate new empty directory and run `STHIRA_QA_VAULT_DIR="$qa_vault_dir" cargo test --package sthira-desktop --test qa_fixture_seed_test -- --nocapture` before launch. The harness writes `sthira.db` and Markdown into the same root layout used by native startup, prints run-specific IDs, and refuses a nonempty target. Ordinary test runs use temporary storage and retain nothing in the repository. Retention and an engine-reopen assertion are not proof that the GUI has been launched or restarted.

Do not repurpose HOME/system variables, change the host clock, invent credentials, or weaken tests to get a pass. Continue independent checks while infrastructure is unavailable and document the actual blocker.

## Functional coverage

For a complete product review, inventory every actual route, visible control, shortcut, menu action, and command family. Trace UI → API → durable state → restart/recovery where applicable. Keep this matrix current as features evolve; there is no permanent passing test-count target.

| Area | Required checks |
|---|---|
| Shell/navigation/search | Fresh empty startup; active state; sidebar resize/caps/pin; every entry point; correct identity/context; supported deep-link/missing-ID handling; search query, stale-response rejection, arrows/activation, Tab/Escape/focus restore. |
| Home/input | Capture/Ask/Research/Plan/Act/Resume and ambiguity; no accidental permanence; correct authorship; empty/multiline/Unicode/repeated submission; model/web dispatch, honest error and preserved input. |
| Projects | Saved brief/status/counts; all member types/canvases/direct references/backlinks; identity dedup/multiple roles; exclusion of deleted/unrelated records; combined search/filter/sort; pagination/list-grid; New page/Add existing; correct editing context and browse-state return. |
| Directories/lifecycle | All tabs/scopes, counts/search/sort/pagination; rename/pin/duplicate/attach/move/detach/convert/delete from all entry points; body/identity/history preservation; transactional failure and shared/exclusive deletion semantics. |
| Editor | Real typing/paste/IME/selection/undo; every formatting/slash/object/AI block; tables/tasks/code/links/directives; Markdown round-trip; title rules; autosave/blur/manual/navigation; failures, rapid document switching and late async results. |
| Connections/AI editing | Supported endpoints/scope; duplicate/self/invalid/deleted target handling; nested preview/trail/Back; dirty source retention; explicit caret insertion; rename/delete resolution; unlink/Undo; saved-draft Derive/lineage; Refine snapshot/restore and run isolation. |
| Objects | Built-in/custom types; field validation/null/false/zero; schema/options/template lifecycle; collection table/list/board and persisted drag change; conversion, embedding and shared reuse. |
| Trust/memory/history | Quote verification and authorship barriers; hedges/conflicts/rejected approaches; ≤75-word brief and grounded review; cold-return/draft/conflict hints; fingerprint/materiality correctness; measured cache path; append-only history; proposal budget/accept/dismiss/Undo; mirror isolation. |
| Research/streaming | Actual provider/model/template/project/web mode; supported adapters plus failures; citations/import/offline source; honest local fallback; DAG/widgets and actions; arbitrary chunk boundaries/malformed output; cancellation/retry/navigation and saved-run identity. |
| Trackers/digests | Daily/weekly time boundaries/timezone; fake-clock due, pause/resume/run-now, duplicates; continuity window; failure/restart; tracker deletion preserves digests; digest filtering/open. |
| Graph/canvas | Persisted topology and edge semantics; filter/search/select/pan/zoom/fit/minimap/open; geometry and labels; saved items/layout/revert; missing targets; spatial/semantic isolation; actual geometry after reopen. |
| Settings | Every actual section/control; provider connect/discover/test/errors; selected/default dispatch; capabilities versus search availability; masked secrets; theme consistency/persistence; accurate storage/account/export/reset semantics. |
| Storage/robustness | Fresh/upgrade/idempotent migrations; FK/index/file/DB parity; on-disk reconcile/conflicts; native process restart; offline/unwritable/write failures; deletion/search/reference cleanup; input/link/content trust boundaries. |

Exercise connected journeys as well as isolated controls:

1. Capture → edit/save → project → shared reuse → restart.
2. Research → inspect/import evidence → Refine/restore → Derive → lineage/preview/back.
3. Recorded decision/hedge/question → cold return → brief/review/history → substantive/minor edits.
4. Custom type/template → object/table/board → shared membership/reference → safe deletion.
5. Project contents → combined search/filter/sort/page → external document edit → Project back → canvas and related project.
6. Tracker → due digest → continuity → pause/resume/run-now → delete tracker with digests retained.
7. Dirty draft + failed/slow save/navigation/generation → recovery → offline read/edit → restart.
8. Canvas placement → no semantic change → explicit reference → graph/overview → unlink/Undo with geometry intact.

## Visual and accessibility execution

Capture current baseline → compare to policy/reference → list concrete discrepancies → correct → recapture and inspect → retest the affected controls. Preserve raw full-context images; detail crops/overlays can explain geometry without hiding surrounding defects.

Use representative main views at 1672×941 light, 1366×768 dark, and 900×768 constrained desktop. Inspect normal 1200×850, wide 1920×1080 and 125% zoom/scale; distinguish browser zoom from native OS scaling. Expand around wrapping failures and shared CSS changes. Check sidebar/panel variants, long/empty values, populated/no-result/error/loading states, and all affected overlays.

Measure rendered bounds, text anchors, overflow, actual contrast pairs, and keyboard focus where relevant. Test active states and motion in operation. Restore temporary overrides. Missing native GUI, fresh screenshot, assistive-tech, live provider, or platform execution stays explicitly unverified.

## Commands and repair discipline

Discover scripts/tooling first. In the actual UI package run `npm test` and `npm run build`. From the root run `cargo test --workspace`, available phase gates (inspect xtask mapping; currently 1–7 and 9), `cargo clippy --workspace -- -D warnings`, `cargo fmt --all -- --check`, and `git diff --check` as appropriate. Do not invent a lint script or treat whitespace validation as Clippy/rustfmt.

Record unavailable tools separately. Run timing-sensitive frozen Rust checks without unrelated heavy builds; retain an initial timing failure and subsequent retests rather than silently discarding it. A provider-injecting fake test is deterministic evidence only.

For defects: reproduce → identify root cause → smallest coherent correction → meaningful regression test when warranted → relevant check → fresh rendered check for visual changes → affected journey. Do not weaken assertions, mock away a broken boundary, remove tests, or perform unrelated broad refactors.

Acceptance is limited to executed assertions. Open data-loss/provenance errors or unverified critical native/live journeys prevent unconditional product sign-off. A clean automated baseline does not establish complete visual quality.
