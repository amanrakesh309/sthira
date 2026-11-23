# Shared memory

Shared memory is the foundation of [Agent Extension phase AE1](../agent-extension-plan.md#ae1--shared-memory-and-shared-actions). It stores intentional context separately from original pages and factual Resume state. No AI connection is required. Research, Plan, Draft and Resume do not automatically read or write this store. Native agent conversations use eligible context within their explicit scope; memory writes require proposal review.

## Using Memory

1. Open **Memory** in the sidebar and choose **New memory**.
2. Add a title and content. Choose **Workspace**, an existing **Project**, or **Session** with an explicit identifier. The Memory form accepts a manually supplied session label. Native agent conversations use their stable thread ID as their session scope; a dedicated thread picker is not implemented.
3. Optionally pin the entry and search for saved source pages. Project-scoped memories require source pages attached to that project. Save explicitly.
4. Search saved titles/content, filter by scope, and select an entry to inspect its original attribution. Choose **Edit memory** to update its title, content, or pin. Scope and captured source basis are immutable; create another memory to change them.
5. **History** shows the latest 100 revision snapshots and the editor responsible for each. **Archive memory** removes an entry from active lists/context; **Include archived** exposes it for **Restore memory**. Archive preserves content and revisions and is not permanent erasure.
6. **Preview shared context** shows eligible saved context for the current filter, without calling a model. It is an owner preview of what a suitably granted adapter could request, not proof that an agent is connected.

Unsaved edits block switching memories or leaving the view. A failed save retains the draft. Stale versions produce an error instead of overwriting another writer; **Discard draft and reload latest** explicitly discards the local changes. Memory mutations do not edit page bodies, membership, references, epistemic claims, or Resume caches.

## Persistence and provenance

The additive SQLite **v20** migration creates `shared_memories` and `shared_memory_revisions`. Each successful create/update/archive/restore saves the entry and a full revision in one transaction, then emits `memory_changed`. Original author, scope, and creation time are locked by a database trigger. Expected versions protect updates, archive, and restore against concurrent changes.

Native UI entries are user-authored. The core also supports agent contributions labeled with agent ID, model, and provider supplied by a trusted adapter; the native agent now exposes reviewed memory proposals through [AE2 workflow proposals](agent-workflow-proposals.md). Owner edits retain the original contribution's attribution while revision history records the owner as editor. A memory is a contribution, not independently verified evidence or an automatically adopted decision.

Memories live in the vault database, including their history. They have no Markdown mirror and are not ingested as artifacts. Include the database in ordinary vault backups; copying only document Markdown will omit memories. The existing destructive **Clear vault data** operation also removes memories and revisions. There is no individual permanent-delete UI in this phase.

## Sources and lifecycle

Captured sources retain artifact ID, title at capture, and content hash. Reads compare that basis to current saved artifacts. A changed hash is **changed**; deleted or unavailable sources, and sources detached from a project-scoped memory's project, are **unavailable**. Their memories remain inspectable but are excluded from shared context. Refresh the basis by explicitly creating a new memory from the current sources.

Deleting a project preserves its memory rows and history for the owner in **All scopes**. The detail view marks the project unavailable. Such entries are excluded from context and non-owner agent access, including when the entry has no source pages. They are not reassigned to workspace scope.

## Shared action boundary and permissions

`Engine::execute_memory_action` supports `list`, `read`, `create`, `update`, `archive`, `restore`, `history`, and `context`. Native `cmd_memory_action` resolves vault-owner access itself. Neither action payloads nor the desktop UI may supply an author or grant themselves access.

`MemoryAccess` is constructed by trusted Rust adapters and is deliberately not deserializable. It separates workspace, project-ID, and session-ID grants from write permission. Reads, searches, history, and context apply scope grants. Non-owner agents may mutate only their own exactly attributed contributions and may cite only saved pages within their project grants. Owners can inspect and edit all memory. This is an internal authorization boundary; external authentication and revocation now have an [AE3 CLI/MCP adapter](cli-and-mcp.md). That adapter exposes reads and reviewed native proposals, not direct external mutation.

Read operations use a SQLite snapshot for consistent records and source freshness. Writes are atomic with revisions; revision failure rolls back the entry and emits no success event. Core timestamps use the injected clock. Source content and memory text are untrusted data, never instructions that confer tool permissions.

## Retrieval limits

| Constraint | Current behavior |
| --- | --- |
| Title / body | Nonblank; at most 160 / 16,000 characters. |
| Scope identifier | Nonblank for project/session; at most 160 characters. Projects must exist when creating. |
| Captured sources | At most 20 supplied IDs, deduplicated. |
| Search | Literal title/body substring, at most 500 characters; SQLite's built-in case folding is primarily ASCII. No semantic retrieval. |
| Candidate list | First 100 matching records, pinned first, then newest update and stable ID. No pagination yet. |
| Context | Excludes archived entries, unavailable scopes, and any entry with a changed/unavailable source; uses whole entries, without truncation. |
| Context ceiling | At most 20 entries and 24,000 serialized JSON characters including provenance. This is not a model token count. |
| UI context preview | Requests at most 8 entries and 8,000 characters from the current matching candidate set. Shows stale and budget exclusions. |
| History | Latest 100 revisions returned; older revisions remain stored. |

Pinned entries receive priority, not guaranteed inclusion. A large entry may be skipped while a later smaller entry fits. A session grant is independent from a project grant; a future adapter must request only the scopes appropriate to its task.

## Execution layers and remaining work

Browser preview uses an explicitly labeled in-memory store that resets on reload. It demonstrates UI interactions only. It does not establish native durability, native permissions, or parity of source validation. Engine and desktop handler tests use disposable vaults to exercise actual SQLite migrations, grants, attribution, source invalidation, revisions, reopen, rollback, and reset. UI tests mock the action boundary.

Native Tauri GUI action/restart checks remain outstanding, as tracked in [known limitations](../known-limitations.md). Owner memory actions are model-free; agent generation requires separate live-provider quality checks. The [native workspace agent](workspace-agent.md) now reads eligible memory through this boundary and uses its persisted thread ID as session identity across follow-ups. Reviewed memory publishing and revisions are available through [agent workflow proposals](agent-workflow-proposals.md). The [AE3 CLI/MCP adapter](cli-and-mcp.md) also reads scoped memory and submits reviewed memory proposals. Automatic extraction, scheduled consolidation, autonomous publishing and cross-agent handoffs remain deferred.
