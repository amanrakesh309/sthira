# Product contract

## Purpose and tone

Sthira is an AI-native, local-first workspace for thinking, research, planning, and durable knowledge. Its promise is: **Help me remember where I was, without making me revisit everything I did.**

The experience is calm, capable, trustworthy, exploratory, steady, and user-owned. Capture is lightweight; complexity appears when useful. Orientation is concise, with detail available on demand. No guilt, artificial urgency, streaks, fabricated progress, or repeated dismissed suggestions.

## Knowledge ownership and trust

- Artifacts retain their identity, authorship, content, revision history, and source attribution across reuse, movement, conversion, refinement, and derivation.
- User-recorded claims require actual user authorship and verified quote evidence. Hedges and questions are not recorded decisions. External evidence stays source-backed; saved/generated AI prose stays AI-authored/inferred until an explicit supported adoption action changes the relevant knowledge state.
- Every substantive brief/history claim must lead to inspectable evidence. Missing evidence and uncertainty are visible; plausible prose is not proof.
- AI-proposed structural changes remain inspectable, accept/dismiss controlled, and reversible where promised. Inference does not silently attach, move, link, or change project purpose. Accepted changes use the established change-set mechanism; dismissal persists.
- A transient answer does not become permanent workspace clutter until explicitly saved. Explicit Capture, Research, Plan, Act, Resume, and ambiguous-input handling retain their distinct contracts.

## Navigation and working context

- Opening a project from navigation opens its **overview**, never its first note or an unsolicited generated resume brief. Explicit Resume remains a separate intention.
- The overview shows saved purpose/status/dates, factual counts, active members, saved project canvases, and directly referenced/backlinked entities. Deduplicate by identity and retain all relationship labels. Do not flatten another project's contents into the current project.
- Search names/types/loaded summaries according to the actual search contract; combine type/relationship filters, stable sort, list/grid, and clamped pagination. Do not advertise full-text search if only summaries are searched.
- Selecting an entity opens that exact entity centrally. Project back restores the originating overview's search, filters, sort, layout, and page. An external/shared artifact keeps its correct editing context.
- A note is one formatted editable surface backed by Markdown. Do not restore a binary Edit/Preview workflow. Read-only reference previews remain read-only.
- The note header exposes History, Connections, and truthful save state. Theme is in sidebar/Settings; Spatial canvas is in sidebar; evidence is in Page actions → Sources & insights.
- Preserve the original draft, selection/scroll where supported, and content identity while reading nested references. Back/trail/Escape retraces context predictably; opening a reference centrally saves the original safely first.
- Global search, sidebar, directories, graph, and workflow outputs resolve stable identities. Same-title entities must not be mixed.

## Relationships and reuse

Keep these concepts distinct in storage, wording, and UI:

| Concept | Required semantics |
|---|---|
| Project membership | Many-to-many attachment of one artifact; Add existing does not duplicate its content/history. |
| Manual reference | Explicit directional connection with outgoing references and backlinks. Unlink preserves both endpoints and content; Undo restores the relationship once. |
| Derivation | Automatic source/child lineage from the user's Derive action. Save the newest draft first; preserve provenance and associations. Lineage is not a removable manual reference. |
| AI proposal | Suggested organization pending an explicit decision. It is not already an accepted relationship. |
| Spatial placement | Canvas position/layout only. Proximity creates neither semantic edges nor membership/evidence. |

Manual reference targets currently include projects and artifact-backed pages, typed objects, sources, and snippets. Canvases can be project contents but are not manual reference targets unless the owner explicitly extends that contract. Arbitrary text mentions and pasted external links do not silently create connection cards. Explicit Insert in page uses stable target identity and preserves surrounding content.

## Editing, lifecycle, and persistence

- Autosave, manual save, blur/navigation, and asynchronous generation must keep writes bound to the original artifact/project. Never overwrite a newly opened document with an older save result. A failed save retains a recoverable draft and cannot display success.
- Derive aborts on failed prerequisite save. Refine snapshots the prior state and provides the documented restore path. Late results must not leak into another run/document.
- Markdown round-trip preserves supported tables, tasks, code/language, links, raw directives, and embedded objects. Unsupported blocks must not disappear during surrounding edits.
- Object conversion preserves artifact identity/body; templates instantiate independent content. Shared properties and body remain consistent across contexts.
- Move/attach/detach is transactional. Deleting a project defaults to preserving its notes; explicit cascade respects shared artifacts. Dialog copy must describe actual consequences. Do not invent recovery for irreversible operations.
- SQLite, Markdown files, indexes, fingerprints, references, and UI state must reconcile correctly. Migration/reconciliation tests use copies or disposable vaults.
- Memory mirrors remain outside watched artifacts with their do-not-ingest marker, preventing recursive ingestion of AI summaries.

## Memory, research, scheduling, and settings

- Level 1 orientation is capped at 75 visible words; Level 2 adds grounded detail. Follow deterministic cold-return/draft/conflict hints. Opening/typo/whitespace activity must not fabricate substantive progress; small meaning changes such as negation remain substantive.
- Cache correctness follows the documented basis fingerprint. Performance targets require measured paths and environments; they are not blanket native UI latency guarantees.
- Research/Plan stream in the center workspace. Selected provider/model/search mode/template/project must reach the actual dispatch path. Broken structured output degrades safely to readable content.
- Web failure/zero results is labeled honestly. Local fallback cannot masquerade as web-backed evidence. Import preserves external authorship and source metadata; offline access must match the saved artifact.
- Trackers obey saved cadence/time semantics, pause/resume/run-now, duplicate prevention, and the continuity window. Deleting a tracker preserves generated digests.
- Provider connectivity, model capability, selected/default model, saved state, and search availability are separate facts. Native runtime must not surface browser demo records or deterministic test responses as genuine results.
- The shell and local saved knowledge work without external assets/services. Secrets remain masked and absent from logs/evidence. Exposed account/storage controls must perform their advertised behavior or state their limitation clearly.
