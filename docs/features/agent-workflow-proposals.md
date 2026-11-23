# Agent workflow proposals

This AE2 batch connects the native agent to Refine, Derive, title suggestions, and reviewed shared-memory contributions. It builds on [document conversations and background tasks](agent-conversations-and-tasks.md). CLI/MCP, cross-agent continuity and migrations remain AE3–AE5.

## Use the workflows

1. Enable **Workspace agent** in Settings → Workflows and choose a connected, enabled model. Open **Agent**, select the project or entire workspace, and give a goal such as “Refine my research note, derive an execution plan, and suggest clearer titles.” Include the page name or identity so the agent can find it.
2. For memory, ask “Propose a project memory of the unresolved questions, citing the source note.” Select a project to create project memory. Workspace memory requires the separate **Include workspace-wide memory** grant. Session memory uses this conversation's stable thread ID. The agent may update eligible existing memories in its scope, with owner review.
3. Continue working while the native task runs. Open **Agent tasks → View task**, or the saved goal in Agent, to inspect the activity. Generation only saves proposals. A successfully finished run must precede acceptance.
4. Choose **Review proposal**. Inspect the saved source/current content, proposed result, model/provider and operation details. Choose Apply/Create/Use title/Save memory, or Dismiss. A failed acceptance leaves the proposal available and shows the error. A changed source, deleted target, changed memory version or lost project membership requires a fresh proposal.

Agent workflows use the model chosen for that agent request. The dedicated Refine, Derive and title controls retain their existing model preferences and behavior. The agent composes the structured proposal itself; it does not silently dispatch another model or perform web search. Built-in derivation types are execution plans (`plan_from_research`), ideas (`extract_ideas`) and task checklists (`extract_tasks`). Template selection through the agent is not implemented.

Document chat can propose Refine or titles for its saved document when **Suggest a document edit** is checked. Use **View agent activity** or Agent tasks to review these operations. Document grants cannot create derived pages or publish/update memory; use a workspace goal with an explicit broader scope. Ordinary conversation edit proposals still use the document-chat review surface.

## Review and recovery

| Operation | Acceptance and recovery |
|---|---|
| Refine | Reuses the existing refinement transaction, preserves document identity, title and original authorship, saves an AI revision and restore change set. **Undo applied edit** uses the existing guarded refinement restore; newer edits prevent undo. |
| Derive | Creates a separate AI-authored page with `derived_from_artifact_id` and derivation type. Project scope uses that project; entire-workspace goals inherit a single active source project, otherwise create standalone. The captured destination is shown in review and must remain available. Source content remains intact. Use normal page deletion for a derived page; there is no agent-specific creation Undo. |
| Titles | Offers up to three validated saved suggestions. No option is preselected; **Use title** applies only the exact selected suggestion. A matching initial `#` heading follows the rename, using the existing title API. Other content and original authorship are preserved. Rename again in the editor if needed; there is no agent-specific title Undo. |
| New memory | Creates an agent-attributed contribution with producing model/provider, captured source hashes and its first revision. Only completely read, unchanged pages may be attached as sources. Source-free memories remain explicitly unsupported contributions. Edit/archive through Memory afterward. |
| Memory update | Owner approval permits a scoped revision of another contributor's memory. Original author, scope, source basis and pinned state stay intact. The revision records the agent/model/provider as editor. Reject archived, stale-source or unavailable memories and stale versions. Use Memory editing/history or archive afterward; no automatic memory Undo is advertised. |

All four operations consume their saved proposal receipt inside the domain write transaction. A failed database write rolls back both; repeated or concurrent acceptance cannot apply the same proposal twice. Refine and Derive preserve database success when Markdown mirror synchronization fails and report an advisory warning. Dismissal does not mutate source pages or memory. No model tool can accept a proposal, change its grant or supply author identity.

## Storage and bounds

Existing v22 run/step tables store the typed action, source snapshots and review receipt; no new database migration is required. Current limits still apply: eight model actions, eight minutes, 48,000 prompt characters, full-page reads/proposed document bodies up to 16,000 characters, memory titles up to 160 characters, memory bodies up to 16,000 characters and up to 20 source pages. Read-only memory lookup rechecks scope and source freshness. Prior proposals are supplied as bounded excerpts with their current decision states; no previous action is replayed automatically.

## Verification boundaries

Disposable-vault engine tests exercise loopback model transport, read/propose/finish/review, lineage, original attribution, revision editors, stale competitors, exact title choice, repeat acceptance, restore and reopen. UI tests verify the review dialogs and chosen-title submission. Browser fixtures verify rendered controls separately from engine execution. Native Tauri GUI execution/restart, live-provider quality, screen-reader operation and native display scaling remain unverified; see [known limitations](../known-limitations.md).
