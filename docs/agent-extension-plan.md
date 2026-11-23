# Agent Extension: five implementation phases

This is a new five-phase sequence, **AE1–AE5**, separate from the historical phases 1–25 in [the original plan](plan.md). It does not renumber or restart that plan. [Permanent product policy](policies/product.md#planned-agent-and-migration-roadmap) governs behavior; feature contracts describe what actually exists. Implement and verify one phase before moving to the next. No dates are promised.

## AE1 — Shared memory and shared actions

Give the workspace a visible, durable place for useful context that future agents can share. Support workspace, project, and session scopes; manual creation and editing; pinning; archive/restore; original author/model/provider attribution; source identity and freshness; revision history; stale-write protection; and bounded retrieval.

Use one typed core action boundary with trusted caller identity and scope grants. The native owner UI uses this boundary now. Later adapters must reuse it rather than writing directly to memory tables or accepting caller-supplied permissions.

The initial implementation is described in [Shared memory](features/shared-memory.md). It does not automatically extract memories or supply them to existing model workflows. Shared actions in this phase cover memory; later workspace operations must reuse existing protected domain APIs.

Acceptance requires additive/idempotent migration and reopen coverage, isolated grant checks for reads/history/context/writes, immutable original attribution, atomic revisions and failed-write rollback, bounded context excluding archived/stale/unavailable data, and UI coverage for drafts, failures, and lifecycle actions. Browser inspection and native GUI/restart verification are separate; outstanding native checks stay in [known limitations](known-limitations.md).

## AE2 — Native workspace agent

Let users give Sthira a goal and have its own agent use configured models, eligible memory, and supported workspace actions. Start with a narrow tool set for reading/searching context, creating pages, and proposing changes. Reuse existing document, membership, reference, provenance, revision, and change-set protections.

The native foundation is described in [Workspace agent](features/workspace-agent.md). AE2 now includes [agent-powered document conversations, follow-ups and background tasks](features/agent-conversations-and-tasks.md), [reviewed Refine, Derive, title and shared-memory proposals](features/agent-workflow-proposals.md), [reviewed project organization and page references](features/agent-organization.md), and [project creation, direction and moves](features/agent-project-workflows.md), while preserving original plan numbering and keeping AE3–AE5 separate. Native GUI/live-provider follow-ups remain separate from engine and browser evidence.

Show scope, producing model/provider, progress, tool activity, results, and failures. Provide cancellation and bounded work. Treat retrieved pages and memories as untrusted context. Changes that require review must remain proposals until accepted; model output cannot grant itself authority.

Acceptance requires a complete goal-to-result flow with a configured provider, persisted run/session identity and handoff, explicit grants, source-backed context, honest tool errors, stale-result isolation, cancellation at write boundaries, and review/undo where promised. Model-free and unavailable-model states must remain usable and truthful.

## AE3 — CLI and MCP access

Extend the existing CLI and add an MCP server exposing supported actions and memory to external applications. Resolve authentication, caller identity, grants, and read/write permissions in trusted adapters. External adapters also need a shared-vault attach lifecycle that does not run native startup recovery or interrupt work already owned by the desktop app. Preserve the same validation, concurrency checks, evidence, revisions, and event behavior as the native UI/agent.

The first local adapter batch is implemented in [CLI and MCP access](features/cli-and-mcp.md): safe existing-vault attachment, revocable scoped credentials, JSON CLI and stdio MCP discovery/calls, shared native proposals and owner review, retry receipts, and cross-process event hints. Native GUI and real client/platform checks remain separate.

Acceptance requires documented setup, action schemas and examples, capability discovery, scoped authorization, explicit connection revocation, error handling, and end-to-end tests against the real CLI/MCP transport. Do not accept arbitrary SQL, model-supplied permissions, or unrestricted shell access as workspace tools.

## AE4 — Cross-agent continuity

Make handoffs between the native agent and external agents deliberate and inspectable. Carry goals, unresolved questions, selected context, stable artifact identities, and source/model attribution across sessions. Keep each contributor's history and handle competing writes without silent overwrites.

The first batch is implemented in [Saved cross-agent handoffs](features/cross-agent-continuity.md): explicit owner-shared snapshots, scoped CLI/MCP contributions, current review/source context and owner-controlled native continuation. Automatic delegation/resumption and native GUI/live-client verification remain separate follow-ups.

Acceptance requires a native → external → native scenario using AE3, reopening persisted handoffs, scope/revocation checks, source-change invalidation, and concurrent-edit resolution. Reading a handoff must not create duplicate pages, change authorship, or execute its text as instructions.

## AE5 — Migration system and migration agent

Bring existing knowledge from Notion, Obsidian, Google Docs, and additional sources into Sthira. Build deterministic importers first; the migration agent can help select sources, explain mappings and unsupported content, and guide a reviewable import using those importers.

Start with a source inventory and preview, then mapping, validation, explicit import, and a readable result with errors and retry guidance. Preserve original source data, source attribution, stable import identity, and supported content/relationships. Define attachments, links, properties, duplicate detection, incremental reruns, cancellation, and rollback behavior per importer.

The local-export foundation is implemented in [Migration assistant](features/migration.md): deterministic Markdown/text presets, explicit inventory/review/apply, source provenance, duplicate/conflict handling, atomic receipts and guarded recovery. Live connectors, rich formats and model-led migration planning remain follow-ups.

Acceptance requires disposable source fixtures and documented fidelity for each supported export/connector, duplicate-safe retries, bounded imports, partial-failure reporting, and verification that source data remains untouched. Export formats, live API permissions, and media fidelity are not guaranteed until implemented and tested.

## Scope boundaries

Embedded terminals and hosting third-party agent interfaces are outside this sequence. Existing workflow/template/media/platform follow-ups remain separate. No later phase is authorized as part of implementing an earlier one, and an implementation or passing unit test alone does not certify native GUI or live-provider behavior.
