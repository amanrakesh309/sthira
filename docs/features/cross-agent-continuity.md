# Saved cross-agent handoffs (AE4, first batch)

Cross-agent continuity carries a task's goal, selected source identities, unresolved questions, contributor attribution and actual proposal review state between Sthira's native agent and an external agent. This batch adds explicit saved handoffs, CLI/MCP contributions and owner-controlled native continuation. It uses the [AE3 local adapter](cli-and-mcp.md) and the same [native review writes](workspace-agent.md).

## Native → external → native

1. Finish a workspace Agent goal. In its saved run, open **Saved handoffs → Save a new handoff**.
2. Enter a title, next steps and open questions. Select an existing scoped external connection, then select page or memory references read/produced in this run. Save explicitly. No page is copied or created by saving/reading a handoff.
3. The chosen external connection discovers the record with `sthira_handoffs` and reads its current revision. It must read complete current source pages/projects in its contribution batch before proposing edits or organization, just as in AE3.
4. The external agent contributes a summary, next steps, open questions and optional native action proposals. Those claims retain the connection's externally configured model/provider attribution. A proposed change remains pending until the owner accepts it in the normal review UI or owner CLI.
5. Back in Sthira, refresh/open the saved handoff. Inspect the original goal/result, current source versions, contribution summaries and current proposal states. **Inspect contribution and proposals** opens that exact saved run for review.
6. Enter what Sthira should do next and explicitly choose an available connected model. **Continue in Sthira** queues a native background task in the original scope. If references changed, **Continue with current sources** explicitly acknowledges the displayed current versions. New proposals still require review.

Saved handoffs appear with the original run, its external contribution runs and its native continuation runs. The list shows up to the latest 50 matching handoffs; a detail view shows the latest ten contributions. Older contribution runs remain in ordinary Agent history. Native model context includes bounded excerpts from the latest three contributions, clearly marked as excerpts/untrusted context. This is manual continuity; it does not automatically switch agents, restart tasks, delegate work, import data or adopt generated direction as fact.

## Scope, attribution and source versions

A handoff is shared with exactly one existing connection. Its grant must cover the source run's original project/workspace scope and its separate workspace-memory choice. A project-scoped connection cannot receive a workspace-scoped run. A broader recipient still contributes inside the handoff's original scope. A read-only connection may read an explicitly shared handoff; contributing requires `can_propose`. A source run's read-only action grant is retained. Document-chat handoffs and session-memory sharing are outside this first batch.

The immutable saved snapshot contains the original goal, producing-agent summary/carry-forward, owner-written next steps/questions, producing model/provider labels, selected page/memory IDs and captured versions. Page bodies stay in their existing documents. Source metadata tracks document version/hash/title/authorship, membership revision fingerprints, project metadata revisions, memory versions and memory-source revision fingerprints. Changing and restoring a value can still invalidate the captured basis. External identity labels are owner-configured, not independently verified model identity.

The detail view computes current source availability/freshness and actual proposal states rather than trusting a contributor's claim that work was applied. Saved summary text remains attributed, historical, untrusted context. It confers no grant, is not quote verification or a recorded decision, and cannot approve proposals or run instructions on its own.

Changed sources block new external contributions and acceptance of existing contribution proposals. The owner can save a fresh handoff from the latest appropriate run for further external work. This conservative guard includes selected project/memory context; accepting one proposal can invalidate remaining proposals that relied on the older handoff basis. Normal guarded owner Undo remains available where the action family supports it.

Native continuation can use changed but still available sources after explicit owner review of the current packet. The client sends the displayed `context_version`; queueing rechecks it in the same transaction as the new run and continuation receipt. A changed packet rejects queueing. Captured source bases are rechecked before model dispatch, tool recording and reviewed application. Sources changing after queueing make the run fail safely; no stale document write is silently applied. Missing/deleted/out-of-scope references, archived/stale selected memories or a closed handoff block continuation until the context is made eligible or a fresh handoff is saved.

## Drafts, retries and closing

Unsent handoff/continuation drafts block leaving or switching goals until saved, sent or explicitly discarded. Failed saves and continuations preserve input. Operation errors appear next to the relevant action. Refresh retrieves current context while preserving drafts. Other unsent goals/follow-ups/new-handoff drafts must be resolved before switching into a continuation.

Saving and native continuation use explicit request IDs. Retrying the exact request returns its existing record/run; a different payload cannot reuse the receipt. The UI retains the original payload after an uncertain response so an advisory refresh cannot turn a committed save into a duplicate. Editing the draft or explicitly refreshing context starts a new attempt. External contribution request IDs use the existing connection's AE3 receipt namespace; duplicate requests return only a saved run/status receipt and never replay old cached document context.

**Close handoff** is an explicit owner action that preserves its history while preventing further sharing, contributions, pending contribution acceptance and continuation execution through that packet. Connection revocation likewise blocks an already-running external client's next request and is rechecked at proposal write boundaries. The owner can still inspect old contributions and recover eligible applied work. Native continuation is an owner action; it does not require reactivating an external connection, and it never grants that connection renewed access.

As in AE3, a killed process can leave an invocation active. Stop it through the task panel or use native startup interruption recovery; no automatic resume is claimed. Disabling a native workflow prevents native generation; revoke a connection or close a handoff to stop external access explicitly.

## CLI and MCP

The existing private `STHIRA_ACCESS_TOKEN` environment variable and local stdio configuration continue to apply. `api schema` / MCP `tools/list` now discover four tools, including `sthira_handoffs`. Tool arguments cannot specify actor identity, permissions, approval or native model execution.

```sh
sthira-cli --workspace /absolute/path/to/vault api call sthira_handoffs '{"type":"list"}'
sthira-cli --workspace /absolute/path/to/vault api call sthira_handoffs '{"type":"read","id":"HANDOFF_ID"}'
```

A contribution uses the revision returned by the latest read. Text-only contributions may use an empty action array. This example stages a Refine proposal, preserving uncertainty:

```json
{
  "type": "contribute",
  "contribution": {
    "handoff_id": "HANDOFF_ID",
    "expected_revision": 1,
    "request_id": "release-contribution-001",
    "summary": "Prepared clearer wording; the date remains unknown.",
    "next_steps": "Review the wording and inspect the missing evidence.",
    "open_questions": "Which evidence is still missing?",
    "actions": [
      {"type": "read_page", "artifact_id": "SAVED_PAGE_ID"},
      {"type": "propose_refine", "artifact_id": "SAVED_PAGE_ID", "content_md": "# Release question\n\nThe date is still unconfirmed. Review the missing evidence before choosing it."}
    ]
  }
}
```

```sh
sthira-cli --workspace /absolute/path/to/vault api call sthira_handoffs @/absolute/path/to/contribution.json
```

Up to seven actions are accepted, followed by the adapter's terminal receipt. Native scope/source/tool budgets continue to apply. A failed action remains visible in the saved run and produces `isError: true`; other valid pending proposals still need owner inspection. Concurrent contributions use optimistic handoff revisions and conversation activity checks: one request cannot silently overwrite another contributor's work. Contributors remain separate saved runs in the original conversation scope.

Owner-only commands are deliberately absent from MCP tools:

```sh
sthira-cli --workspace /absolute/path/to/vault handoff list [SOURCE_OR_CONTRIBUTION_RUN_ID]
sthira-cli --workspace /absolute/path/to/vault handoff read HANDOFF_ID
sthira-cli --workspace /absolute/path/to/vault handoff save @/absolute/path/to/handoff.json
sthira-cli --workspace /absolute/path/to/vault handoff close HANDOFF_ID CURRENT_REVISION
sthira-cli --workspace /absolute/path/to/vault handoff continue @/absolute/path/to/continuation.json
```

A save payload contains `request_id`, `run_id`, `expected_run_revision`, `recipient_connection_id`, `title`, `next_steps`, `open_questions`, `artifact_ids` and `memory_ids`. Use the current run revision from `review show RUN_ID`. Title is bounded to 160 characters, next steps/questions to 2,000 each, and references to eight pages plus eight memories. The native form selects references read/produced in that run; the owner CLI can select additional saved in-scope IDs explicitly.

A continuation payload contains `id`, `request_id`, `expected_context_version` from `handoff read`, owner `text` and an explicit configured `model_id`. For example:

```json
{
  "id": "HANDOFF_ID",
  "request_id": "native-return-001",
  "expected_context_version": "CURRENT_CONTEXT_VERSION",
  "text": "Inspect the reviewed contribution and prepare the next evidence check.",
  "model_id": "CONFIGURED_MODEL_ID"
}
```

The desktop queues independent background execution. The owner CLI executes the native continuation in its current foreground process using the configured provider and enabled Workspace agent workflow. It publishes queued/final state through the cross-process outbox. A replayed active/finished run is returned without replaying model dispatch. Failed generation remains a failed saved run; inspect the returned status/error before interpreting its output. Handoff reads/listing do not dispatch a model. Cross-process notifications remain advisory; explicit refresh reads authoritative state.

## Storage and verification boundaries

SQLite **v26** adds handoffs, contribution links and native continuation receipts/bases. Existing grants, AE3 invocation receipts, agent runs, source content and attribution are preserved by additive/idempotent upgrade. Snapshot/link/receipt writes are transactional; injected failures roll back new records, run creation and revision changes. Backups contain these records. Clear vault data cascades their removal before deleting credentials and rotating the vault instance identity; old tokens and handoff IDs cannot recover cleared work.

Engine tests use temporary vaults and loopback model adapters for native → external → native with memory/source identity, attribution, reviewed changes, source conflicts, reopen, read-only and recipient/scope denial, close/revocation, optimistic races, receipt rollback and v25 upgrades. A real CLI/MCP subprocess test carries a saved native run through stdio contribution and owner review, then foreground native continuation against a loopback provider; it checks the actual model prompt's current review state. Desktop handler tests cover reset of these records. Mocked UI tests cover save/continuation, stale/unavailable context, errors, identical retries and draft navigation. Isolated browser fixtures separately check presentation and controls, with no vault writes or model calls.

Native Tauri GUI end-to-end execution/restart, live external agents/third-party MCP hosts, provider quality, assistive technology, 125% zoom/native scaling and non-Linux platforms remain unverified. Automatic delegation, resumable background execution, multiple recipients per handoff, document-chat handoffs, session-memory grants and editable/rebased snapshot history remain later follow-ups. Use a fresh explicit handoff when the original basis is no longer appropriate; AE5 migration remains separate.
