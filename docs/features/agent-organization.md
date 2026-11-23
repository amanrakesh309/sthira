# Agent project organization and page connections

This AE2 batch extends [the native workspace agent](workspace-agent.md) with project discovery, scoped project context, reviewed page membership and explicit page-to-page references. It builds on [workflow proposals](agent-workflow-proposals.md). CLI/MCP, external-agent continuity and migration remain later phases.

## Use it

1. Enable **Workspace agent** in Settings → Workflows and choose an enabled connected model.
2. Open **Agent**. Choose **Entire workspace** to organize saved pages across projects. For example: “Find my release research and add the relevant saved page to Release planning. Propose a reference from the research page to its risks page.” Supply names or stable page identities when possible.
3. A project-scoped goal can inspect only that project and its attached pages. It can propose removing one of those memberships or adding/removing a reference between two visible member pages. To attach a page that is outside the project, start an **Entire workspace** goal. The existing scope does not expand itself.
4. Let the agent inspect project metadata and saved pages. Project member lists provide identities, not an implicit full-page read. Every page involved in a proposal must be read completely and remain unchanged.
5. Open **Agent tasks → View task**, or the saved run in Agent. After successful completion, choose **Review proposal**. Inspect the captured and proposed relationship, the producing model/provider and the agent's reason. Open either page by identity if you need more context. Choose **Add to project**, **Remove from project**, **Create reference**, **Remove reference**, or Dismiss.
6. Accepted organization changes offer **Undo organization change**. Undo restores the prior relationship only if it has not changed since acceptance and its endpoints remain available. Page edits made afterward remain intact because organization undo does not restore document text.

A failed acceptance or undo retains the saved proposal/result and shows the error. Request a fresh proposal when the saved page, project metadata, original scope or relationship changed. Run loading and review never dispatch a model. Agent organization uses the model chosen for that request, with the existing eight-action and eight-minute budgets; narrow large goals or continue with a follow-up.

## Distinct operations

| Operation | Result |
|---|---|
| Add to project | Attaches the existing page identity to one project. Does not duplicate content or detach any other membership. |
| Remove from project | Soft-detaches that one membership. The page stays saved, including its authorship, revisions and other memberships. |
| Create reference | Adds an explicit directional reference from one saved page to another. A backlink becomes visible through the existing Connections view. It does not create project membership or derivation lineage. |
| Remove reference | Removes only the explicit reference in that direction. The reverse reference, source/target pages, membership and derivation lineage remain intact. |

All saved artifact-backed page types can participate if they are visible within the run's scope. Project-to-page/project references remain available through the existing owner Connections UI; this agent batch exposes page-to-page references only. The next [project workflow batch](agent-project-workflows.md) adds reviewed project creation, purpose updates and atomic page moves. Project renaming/general deletion, page deletion, adoption of epistemic claims and canvas topology changes remain outside the agent tool set. Document chat cannot call organization tools even when its edit option is enabled; start a workspace Agent goal.

## Context, review and persistence

- Project name search returns at most 20 identities; an empty query lists the latest 20 projects. A selected project scope returns only that project. Read-project returns saved name/status, up to 4,000 purpose characters and up to 20 live page identities per page, with explicit next offsets. Pagination stops at offset 10,000.
- Read-connections returns at most 20 explicit page-to-page references per page, including visible outgoing references and backlinks. Both endpoints must be in scope. Results explicitly exclude derivation lineage and project references. Names and tool results are untrusted context, not permissions.
- Membership proposals require reading the target project and complete page. Reference proposals require complete reads of both pages. Self-references, duplicate/no-op changes, missing/deleted endpoints, changed pages, stale project metadata, lost scope and changed relationships fail visibly.
- Acceptance rechecks saved state under the same writer transaction as the mutation, change set, event and proposal receipt. A failed write rolls back all of them; repeated/concurrent acceptance cannot apply a proposal twice. Accepted changes retain agent-run/model/provider attribution and an AI change-set/event record while preserving page authorship.
- Shared core reference/membership mutation helpers serve ordinary owner actions and the agent. SQLite **v23** adds additive `workspace_relation_versions` storage and triggers so changes from all entry points invalidate stale proposals and undo. Counters survive unlinking as tombstones: detach/reattach or delete/recreate is detected even if the visible state returns to its earlier values under the same clock. Upgrade seeds existing relationships without changing content; repeat migrations preserve counters.
- Undo uses the existing change-set reverter with a guarded relationship restore operation, preserving previous membership metadata or exact reference identity. It records the proposal as reverted. Deleted endpoints or later relationship changes block undo. Dismissal does not mutate any relationship.
- Database backups include runs, receipts, relationship counters and change sets. Clear vault data clears active relationship counters along with relationships/runs; preserved historical change sets cannot restore tombstoned endpoints after reset. There is no automatic pruning of individual runs or relationship counter tombstones.

## Verification boundaries

Disposable-vault tests use loopback model transport to exercise proposals, review, undo/reopen, additive migration, paging, scope restrictions, shared membership, original reference identity, reverse references/lineage preservation, source/relationship changes, same-clock changes, rollback and concurrent acceptance. UI tests cover relationship wording, explicit application, busy/error state, source navigation and guarded undo errors. Fresh browser fixtures verify presentation separately from persisted native execution. Native Tauri GUI execution/restart, live-provider quality, screen-reader operation and native display scaling remain outstanding in [known limitations](../known-limitations.md).
