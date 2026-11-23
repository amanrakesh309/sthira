# Native workspace agent

The [Agent Extension AE2](../agent-extension-plan.md#ae2--native-workspace-agent) implementation adds **Agent** to the sidebar. It uses Sthira's configured model adapters and protected workspace APIs. It can investigate saved context and prepare pages or document edits for explicit owner review. It does not provide web research, shell access, an embedded terminal, external MCP tools, or an autonomous background service.

## Start a goal

1. Connect and enable a model in **Settings → AI & Models**. Enable **Workspace agent** under **Workflows**, optionally choosing its default model. The workflow starts disabled on fresh and upgraded vaults.
2. Open **Agent**, enter a goal, and choose its scope and model. **Entire workspace** permits reads across all saved pages and live project memories. A selected project permits only pages attached to that project and its project memories. The thread has a stable session memory scope, shared by its follow-up runs.
3. Optionally enable **Include workspace-wide memory**. This is a separate grant, off by default, including when the page scope is the entire workspace. Relevant saved context is sent to the chosen provider; a locally named connection is not proof that processing is local.
4. Choose **Start working**. Follow the saved activity as the model searches, reads pages, prepares proposals, and finishes with a result and handoff. Tool errors are visible to both the owner and model. No page or shared memory is silently created by reading the result.
5. Choose **Review proposal** after a successful run. Inspect the current and suggested rich-text documents, then Apply/Create or Dismiss. New pages are AI-authored; an accepted project-scoped creation is attached only to the selected project. An edit retains the page's identity and original authorship and records an AI revision/change set. **Undo applied edit** uses the existing guarded document undo path.

An unsent goal/follow-up blocks navigation until sent or explicitly cleared; a failed start preserves it. Active tasks continue when leaving the view, switching tabs, or working on another goal. Use the checklist button beside Agent for global progress, results and Stop controls. Runs and results stay in the vault database; selecting a saved goal does not dispatch inference. Up to fifty runs are listed with active tasks first; older history remains stored. See [agent conversations and background tasks](agent-conversations-and-tasks.md) for document chat, follow-ups, inherited scopes, concurrency and recovery.

## Tools, review, and trust

The model returns one strictly typed JSON action at a time: `search`, `read_page`, `propose_create`, `propose_edit`, or `finish`. Tool payloads cannot supply author identity, grants, SQL, shell commands, arbitrary file paths, or external URLs to fetch. Structured instructions/schema support varies by model; an unsupported/malformed action stops the run with an honest error rather than a fabricated result.

Search is a literal, primarily ASCII-case-insensitive substring of saved page titles and bodies, with at most 20 results. Each read rechecks saved scope, project availability, deletion, and membership. A page must have been read completely in the same run before the model can propose its edit. Changed read bases reject the proposal; changed saved content rejects later application. Scope is checked again inside the edit transaction, including when the same proposal is reviewed from document conversation.

All document content, memory, tool results, summaries, and handoffs are untrusted context. Citations retain artifact identities and open those saved pages. A model-authored summary is not a verified workspace decision. The initial agent does not create semantic references, derive lineage, move pages, change project purpose, or adopt epistemic claims. New-page creation is consumed atomically with its proposal so repeated/concurrent acceptance cannot create duplicates. Normal mirror-sync warnings remain visible; database persistence and successful Markdown export are distinct.

Shared-memory reads reuse AE1's trusted access boundary and freshness checks. Workspace-wide memory requires the separate grant; archived entries, unavailable scopes, and changed/unavailable source bases are excluded. Up to 8 entries and 8,000 serialized characters are requested before each model step. The agent has no shared-memory write tool. Its handoff remains part of the saved run; deliberate cross-agent handoff and memory publishing belong to later work.

## Limits and lifecycle

- At most **8 model actions**, **48,000 prompt characters**, and **8 minutes** per run. The existing model adapter also enforces its connection/generation timeout policy. These are character/work limits, not a tokenizer-aware guarantee that a small model can fit the context.
- Read at most 16,000 characters per page; a truncated page cannot be edited through this agent. Proposed page/edit content is limited to 16,000 characters. Goals are limited to 8,000 characters; final summaries to 8,000 and handoffs to 4,000.
- A successful `finish` records the summary/handoff and enters review if proposals remain. Reaching a budget without finishing is a failure, not a completed goal. Only successfully finished runs can apply proposals.
- **Stop run** persistently marks the run cancelled, dismisses pending proposals, signals in-flight inference cancellation, and prevents late tool results or automatic restart. Reads/proposal records committed before stopping remain inspectable; no tool automatically applies a document write.
- On application startup, queued/running runs become **interrupted**. They remain inspectable with a clear recovery message; starting another goal is explicit. This is engine-startup handling, not a claim of native GUI crash/relaunch verification.
- Workflow and selected project availability are rechecked between model calls. Unavailable/disabled models reject dispatch without fallback. Model/provider labels are saved with the run and edit proposals.
- New-page creation uses the existing artifact/revision/membership/mirror path with an atomic agent proposal receipt. Document edit proposals use existing conversation review, stale-content checks, revisions, change sets, mirror protections, and undo. Dismissal preserves sources; undo refuses to overwrite later content.

SQLite **v21** adds `agent_runs` and `agent_steps` and extends workflow settings with `agent`, preserving prior settings. These tables are included in ordinary database backups and removed by the existing destructive **Clear vault data** action. Active work cannot recreate deleted run rows after a reset. SQLite v22 extends runs with stable threads, parent/document links, edit permission and revisions. There is no automatic run pruning or individual run-delete UI.

## Verification boundaries

Engine integration tests use disposable vaults and real loopback HTTP model adapters to exercise goal → tools → proposal → review → undo, memory/scope isolation, attribution, stale reads, concurrent create acceptance, failed-write rollback, cancellation/late responses, migration, and reopen. Desktop tests cover workflow opt-in, reset, and duplicate cancellation-token protection. UI tests mock the action boundary. These are separate from model quality.

Browser preview supports the real setup/form layout but deliberately refuses agent dispatch and does not seed successful runs. Native Tauri GUI execution/restart and live-provider goal quality remain unverified; see [known limitations](../known-limitations.md). External CLI/MCP access and cross-agent continuity remain AE3 and AE4.
