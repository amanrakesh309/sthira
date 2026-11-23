# Agent conversations and background tasks

This AE2 batch connects document chat to the native workspace agent, adds conversational follow-ups, and lets tasks continue while the user edits or navigates. It extends [the workspace agent](workspace-agent.md); CLI/MCP and cross-agent access remain separate later phases.

## Document conversation

Open **Chat** on a saved document. The existing **Document chat** workflow and remembered document model choice continue to control generation. Enabling Workspace agent separately is not required for document chat. The native entry point now queues an agent task instead of waiting on a direct discussion-model call.

Sending first saves the current draft through the editor's existing save path. A failed prerequisite save blocks dispatch and keeps the message. Queueing atomically saves one conversation turn, its producing model/provider, and its agent run. Existing conversations, original goals, proposal decisions, and historical model labels remain intact. Loading a conversation does not start a task.

Document requests are restricted to **that saved document and its conversation**. They do not inherit workspace/project page access or workspace-wide memory. They can read memory explicitly placed in the thread's session scope. Use the Agent workspace and its explicit scope controls for broader work.

A question can finish in one model action using the supplied saved-document context. **Suggest a document edit** permits a reviewed edit to the same document; leaving it unchecked makes the request read-only. Document tasks cannot create other pages. Related-page scope expansion, selection-specific editing and additional tools are outside this batch.

After the task is queued, keep typing in the editor or compose the next question. One task runs per conversation at a time. Closing chat, switching tabs, or navigating elsewhere leaves the native task running. The acknowledged message stays in history; a newer unsent message stays in its document/vault draft namespace. Failure after queueing is saved on the attempted turn and visible in task history.

The conversation offers **View agent activity** and **Stop agent task**. It refreshes saved results without injecting them into the editor. When the current draft differs from a reply's captured context, the reply identifies that the document has changed. Proposed edits remain unavailable for application until the run finishes successfully. Review compares the saved proposal basis with the current draft; application rechecks saved content and scope. Existing Apply/Dismiss, revisions, mirror handling and guarded undo remain in force.

## Follow-ups

Open a completed, failed, cancelled, or interrupted run in **Agent**, enter **Continue this conversation**, choose an available model, and send the follow-up. A new run records its parent and stable thread ID. Scope and memory grants are inherited in trusted native code; the follow-up payload cannot broaden them. A document thread remains document-scoped, including when opened from the global task panel.

Each new document message joins the previous document-agent thread while retaining existing legacy conversation history. Each run snapshots its own model/provider. Changing models does not relabel prior replies. Read-only document runs remain read-only when continued from Agent; use the document conversation's explicit edit option to request a change.

Context includes the original thread goal, a document conversation's original goal/mode, up to three ancestor runs with current proposal decisions, and up to twelve earlier document turns. It is bounded to approximately 12,000 serialized characters including origin metadata. Long fields/proposal excerpts are shortened and marked; omitted items are counted. All context remains untrusted data. Stored history is retained even when it is outside the current model context budget. Following up never automatically reapplies an earlier proposal.

## Global task panel

The checklist button beside **Agent** opens **Agent tasks** from any view; it also exists in the collapsed sidebar. Its badge counts working tasks and runs awaiting review. The panel lists queued/running work first, plus recent finished, failed, cancelled and interrupted runs, up to fifty total.

**View task** opens the saved run by identity; **Open document** returns to its originating note; **Stop task** cancels it from wherever the user is working. Existing navigation/save guards still protect the current editor and unsent goal/follow-up drafts. An active agent task itself no longer blocks navigation. Opening saved task results does not dispatch inference.

The app-wide task store loads lightweight metadata, listens for native agent events, and polls while work is active. Full content and proposals are loaded on demand. Task responses are ordered by persisted revisions; client request/reset epochs reject obsolete refreshes. Closing the task panel does not stop its worker.

## Execution, limits and recovery

- The native start command returns after queueing an owned background worker. The worker holds its engine and cancellation token independently of the invoking component. Execution continues while the desktop process remains open; this is not an always-on daemon or cloud service.
- At most **three queued/running tasks per vault**, and **one active task per thread/document**, are allowed. Capacity or duplicate-start rejection occurs before writing a new document turn/run. Different threads may run concurrently.
- Each request retains the existing limits: eight model actions, an eight-minute overall deadline, 48,000 prompt characters, bounded page reads/proposals and model-adapter timeouts. Follow-ups receive a new explicit request budget. Context budgets are character limits, not tokenizer-aware promises for every model.
- Stopping first records cancellation and dismisses pending proposals, then signals inference cancellation. Late results cannot publish replies, queue proposals or modify a different document. The original attempt stays inspectable.
- On startup, unfinished runs and their pending document turns become interrupted together. No run resumes automatically. An explicit follow-up can continue with retained history.
- Clear vault data cancels agent tokens and removes saved runs/conversations through the existing reset path. Obsolete UI reads and late workers cannot restore cleared tasks or drafts. Foreground operations retain their existing reset contract.
- SQLite **v22** additively adds thread/parent/document links, edit permission and monotonic run revisions. Older runs receive their own thread ID; older conversation rows and Markdown remain unchanged. Database backups include tasks and conversations.

## Verification boundaries

Disposable SQLite/loopback-provider integration tests exercise preserved legacy history, original-goal context, scope and read-only enforcement, follow-up decisions/model attribution, continued editing during inference, stale proposal rejection, queue limits, interruption and additive migration. Native handler tests exercise owned workers after their calling state is dropped, global cancellation before a delayed response, and reset without resurrection. Component tests separately cover task-view navigation, follow-up drafts, global progress after view unmount, saved-task opening, cancellation, and stale/reset UI responses.

Browser preview refuses inference and does not seed successful tasks. Browser checks cover the actual task panel, editor/chat setup, draft behavior, keyboard controls and light/dark constrained layouts. Native Tauri GUI task execution/navigation/restart, live-provider quality, 125% display scaling and assistive technology remain unverified; see [known limitations](../known-limitations.md).
