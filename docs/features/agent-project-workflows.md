# Agent project workflows

This AE2 batch adds reviewed project creation, project-purpose updates and atomic page moves to the [native workspace agent](workspace-agent.md). It extends [project membership and page connections](agent-organization.md); CLI/MCP and cross-agent continuity remain later phases.

## Use the workflows

Enable **Workspace agent** in Settings → Workflows and choose a connected model. Open **Agent**, enter a goal and review the saved proposals after the run finishes successfully.

| Goal | Scope and review |
|---|---|
| “Create a Release planning project with a tentative purpose.” | Choose **Entire workspace**. **Review proposal → Create project** creates an empty project and optional AI-inferred direction. Review warns if a live project already uses that name; accepting creates a separate identity. |
| “Clarify this project's purpose while preserving the unresolved questions.” | Select that project or **Entire workspace**. The agent must read its saved project context. **Apply project direction** appends an AI-inferred purpose history entry; it does not relabel generated direction as user-recorded. |
| “Move this page from Research to Release planning, preserving its other memberships.” | Choose **Entire workspace**. The agent reads the complete page and both projects, then **Move page** updates source/destination memberships together. |
| “Move this standalone page into Release planning.” | Choose **Entire workspace**. A standalone source means the page has no live project membership. It never means detach from every project. |

Saved-page reads expose up to 20 current project membership identities within the run’s grant, with a truncation flag, so the agent can identify a move’s source. Project-scoped reads hide other memberships; document chat receives none. These hints do not expand the grant or replace the required project read.

Use the review's named project/page buttons to inspect exact identities. Saved project citations open their project overview through the normal navigation guard. Page content, authorship, revision identity, references, lineage and unrelated project memberships remain intact during a move.

A proposed new project has no saved identity until accepted. After creation, choose **Open project** or send a follow-up such as “Use the accepted project and prepare membership changes for these pages.” Bounded conversation history includes the accepted project identity and actual review state; it does not replay creation automatically. Adding pages to a not-yet-created project in the same model run is outside this batch.

Document chat cannot create projects, change project purpose or move pages, even when document editing is enabled. Project-scoped agent goals may update only their selected project's purpose; creation and cross-project moves require a separate Entire workspace goal. Existing work and unsent drafts retain their navigation guards.

## Review, attribution and recovery

All three tools only record proposals during inference. Successful run completion enables owner acceptance. Model/provider attribution stays with the saved run, action and change set. Purpose changes remain AI-inferred; old purpose records retain their original content and origin. A purpose is workspace direction, not independently verified evidence or an adopted epistemic decision.

Acceptance rechecks the captured project revision, page revision and membership state under the same transaction as writes, events, change set and review receipt. Failed writes roll back everything. Repeated/concurrent acceptance cannot consume one proposal twice. Same-name project counts are rechecked before creation to avoid applying a now-misleading creation review.

**Undo project change** uses guarded change-set recovery:

- **New project:** Undo soft-deletes only an unchanged, unused project. Added memberships (even if later detached), canvases, project references, memory, scheduled work, scoped agent runs or substantive project events prevent this recovery. Ordinary project actions remain available for a project that has been used. No page cascade is performed by this Undo.
- **Project direction:** Undo appends a restored purpose record with the previous content, origin and producing change-set link. Previously saved records remain in history. Undo rejects later project metadata or purpose changes, including changing a value and restoring it under the same clock. If no purpose existed before acceptance, Undo leaves no active purpose.
- **Page move:** Undo restores only the memberships changed by the move, atomically. It rejects any newer membership changes, including change-and-restore operations in another project. Later document edits remain intact because move recovery never writes page text. Missing/deleted projects or pages block recovery.

A failed review or undo leaves its saved state inspectable and shows the error. Dismissal changes no project or page. New names are bounded to 160 characters and one line; proposed purposes to 4,000 characters and reasons to 2,000. A purpose longer than the complete-read budget cannot be rewritten by this tool. Existing eight-action, eight-minute and context budgets still apply; use smaller goals and follow-ups for larger projects.

## Storage and shared actions

SQLite **v24** adds `workspace_project_versions` and triggers covering project metadata and purpose history, plus an index for membership revision lookups. Upgrade seeds existing project versions without changing names, content, direction, attribution or older pending organization proposals. Repeated migration preserves counters. Relationship revision tombstones introduced in v23 also protect moves.

Ordinary owner project creation, purpose updates and moves reuse the same transactional core primitives as reviewed agent actions. Owner purpose updates now supersede the old record and insert its replacement in one transaction, preserving the active direction if insertion fails. Agent-generated direction always supplies trusted AI attribution; tool arguments cannot select an author or grant access.

Database backups include projects, purpose history, revisions, receipts and change sets. Clear vault data removes project revision counters after tombstoning projects; historical recovery cannot recreate cleared live work. Automatic project renaming, project deletion by the agent, bulk moves, direction adoption and automatic task resumption remain outside this batch. Undo of an unused agent-created project is an explicit owner recovery action, not a general project-delete tool.

## Verification boundaries

Disposable-vault loopback model tests exercise creation/review/undo, purpose provenance/history/reopen, accepted identities in follow-ups, atomic moves with shared membership, failure rollback, same-clock changes, scope limits, and additive/idempotent v23 upgrades retaining old pending proposals. UI tests verify review wording, literal identities, exact project navigation, busy/error states, acceptance and guarded undo failures. Browser fixtures verify presentation separately from persistent native execution. Native Tauri GUI execution/restart, live-provider quality, assistive technology and native display scaling remain unverified; see [known limitations](../known-limitations.md).
