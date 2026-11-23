# CLI and MCP access (AE3)

Sthira exposes the existing protected workspace actions to local external agents through a JSON CLI API and an authenticated **stdio MCP server**. Both use the same scopes, complete-source checks, attribution, proposal receipts and owner approval writes as the native agent. This is a local integration foundation; embedded terminals, remote HTTP access, automatic task resumption and automatic delegation are separate work. [Saved AE4 handoffs](cross-agent-continuity.md) now add explicit shared context and native continuation.

## Build and choose the vault

From the repository root:

```sh
cargo build --release -p sthira-cli
./target/release/sthira-cli help
```

`--workspace /absolute/path/to/vault` uses the desktop layout: `sthira.db` and the Markdown vault in that directory. Use the actual desktop vault directory (`STHIRA_VAULT_DIR` if configured; otherwise the desktop's home `.sthira/vault` directory). Do not point at a source-code checkout or a parent directory by mistake. Legacy `--db` and `--vault` arguments remain available for separate paths.

Ordinary commands **attach to an existing database**. They do not seed demo content, reconcile files, clean provider catalogs or interrupt queued/running desktop work. Attachment applies additive database migrations, so use matching desktop/CLI builds and keep a normal backup before upgrading. Missing databases fail rather than silently creating an empty workspace. Explicit initialization is for a new, empty workspace:

```sh
./target/release/sthira-cli --workspace /absolute/path/to/new-vault init
```

`init` refuses an existing database. Native desktop startup recovery retains its original interrupted-task behavior; external attachment never claims desktop task ownership.

## Create a connection as the local owner

Administration uses the local OS user's vault access. It is intentionally absent from MCP tools. Start with a project-scoped read-only connection; set `can_propose` only when the client should prepare changes for review.

```json
{
  "name": "My coding agent",
  "project_id": "SAVED_PROJECT_ID",
  "workspace_memory": false,
  "can_propose": true,
  "model": "Declared model name",
  "provider": "Declared provider name"
}
```

Save this as a grant file, then run:

```sh
umask 077
./target/release/sthira-cli --workspace /absolute/path/to/vault access grant @/absolute/path/to/grant.json > /private/path/to/credentials.json
./target/release/sthira-cli --workspace /absolute/path/to/vault access list
```

The result contains the connection ID and a secret `token`, shown only at creation. Sthira stores its SHA-256 hash, not the raw token. Store the result privately. The combined external client/model display label must fit 160 characters; provider labels are also bounded to 160. These are owner-configured external attribution labels, not independently verified model identity.

A grant must choose exactly one of `project_id` or `whole_workspace: true`. Entire-workspace scope includes all live pages/projects and their project memory. `workspace_memory: true` separately allows workspace-wide memory. Session-memory browsing is not granted in this batch. A project grant cannot create projects, move pages across projects, or read unattached/other-project pages. It may propose edits, titles, refinements, derivations, membership/reference changes within its grant, its project's purpose, and eligible memory changes. The action-specific native validators still apply.

Creating a connection is a separate explicit opt-in from native model workflows. Disabling Workspace agent generation does not revoke an external connection; use `access revoke` to disconnect it.

`can_propose: false` permits reads only. No external grant permits automatic approval, direct memory mutation, SQL, shell commands, arbitrary file access, provider-key access, or connection administration through the tool protocol. Tool arguments cannot select a caller identity or supply permissions. A same-user local process with unrestricted filesystem/shell access can independently bypass any application API; scoped credentials are an application boundary, not an OS sandbox.

## JSON CLI API

Set `STHIRA_ACCESS_TOKEN` in the client's private process environment to the newly created token. Do not put the token in workspace tool arguments, tracked files or command-line arguments.

```sh
./target/release/sthira-cli api schema
./target/release/sthira-cli --workspace /absolute/path/to/vault api call sthira_status '{}'
./target/release/sthira-cli --workspace /absolute/path/to/vault api call sthira_memory '{"type":"context","query":{"scope":{"kind":"project","id":"SAVED_PROJECT_ID"}},"max_entries":10,"max_chars":8000}'
```

The CLI returns JSON MCP-style tool results. `structuredContent` holds the actual result and `content` carries a JSON text equivalent. Nonzero exit status indicates a CLI, protocol or tool error. A batch with a rejected action returns its saved run and tool errors, with `isError: true`; inspect all steps because other valid proposals may remain pending.

The four tools are:

| Tool | Contract |
|---|---|
| `sthira_handoffs` | List/read explicitly shared handoffs or submit attributed, reviewed contributions; see [AE4](cross-agent-continuity.md). |
| `sthira_status` | Current trusted grant, limits and trust labels. No arguments. |
| `sthira_memory` | AE1 `list`, `read`, `history` and bounded `context`, with native source freshness, archive and scope rules. Direct mutations are rejected. |
| `sthira_workspace` | A bounded batch of existing native `AgentAction` reads/proposals. No local model is dispatched. Writes remain pending until owner review. |

Discover the exact JSON schemas with `api schema` or MCP `tools/list`. For example, save this invocation as a JSON file:

```json
{
  "request_id": "research-edit-001",
  "goal": "Clarify the open question without turning uncertainty into a decision.",
  "actions": [
    {"type": "read_page", "artifact_id": "SAVED_PAGE_ID"},
    {"type": "propose_refine", "artifact_id": "SAVED_PAGE_ID", "content_md": "# Open question\n\nThe release date is still unconfirmed. We need evidence before choosing it."}
  ]
}
```

```sh
./target/release/sthira-cli --workspace /absolute/path/to/vault api call sthira_workspace @/absolute/path/to/invocation.json
```

Use one to seven actions per invocation. The adapter appends a terminal receipt. Reads needed to establish proposal bases must be in the **same batch**: read the complete page before edits/Refine/Derive/titles; read the project before membership or purpose proposals; read both projects and the page before a move; read both pages before a reference proposal. A page read is capped at 16,000 characters; a truncated source cannot support an edit or source-backed memory proposal. Project reads paginate and project membership hints remain scoped. Native name/content/reason/context budgets still apply.

Supported proposal families are pages, edits, Refine, Derive, titles, memory create/update, project membership, directional page references, project creation, project purpose and page moves. Read a memory to obtain its actual version before proposing an update. Memory updates preserve original authorship, scope and source basis. New memory remains attributed to the external producer after owner approval.

`request_id` is unique within a connection and retry-safe. Reusing it with identical contents returns only the saved run ID/status and `replayed: true`; it does not replay cached source text or create another proposal. Reusing it for different contents fails. A failed/interrupted invocation is inspectable but cannot be approved; inspect it and use a new request ID to try new work. This is invocation idempotency, not automatic continuation. AE4 handoffs use an explicit, separate context and contribution tool. A forcibly terminated process can leave an invocation running; stop it explicitly in the native task panel or reopen the desktop app to apply its interrupted-task recovery. It cannot be approved while running or interrupted.

## MCP client configuration

The server follows the official [MCP stdio transport](https://modelcontextprotocol.io/specification/2025-11-25/basic/transports), [lifecycle](https://modelcontextprotocol.io/specification/2025-11-25/basic/lifecycle) and [tools](https://modelcontextprotocol.io/specification/2025-11-25/server/tools) contracts. It supports protocol negotiation for `2025-11-25`, `2025-06-18` and `2024-11-05`. It advertises tools only; it does not advertise resources, prompts, sampling, remote transport or task resumption.

A generic client configuration looks like this; adapt its container/key names to the client:

```json
{
  "mcpServers": {
    "sthira": {
      "command": "/absolute/path/to/sthira-cli",
      "args": ["--workspace", "/absolute/path/to/vault", "mcp"],
      "env": {"STHIRA_ACCESS_TOKEN": "YOUR_PRIVATE_CONNECTION_TOKEN"}
    }
  }
}
```

Use absolute paths and a private client configuration. The launched process uses newline-delimited JSON-RPC on stdin/stdout; diagnostics go to stderr. Initialize first, send `notifications/initialized`, then discover/call tools. Invalid envelopes, malformed JSON, unknown tools and invalid arguments return JSON-RPC errors. Domain failures use tool `isError` results. Notifications receive no response; closing stdin shuts down the server. Operations run sequentially and are local; cancellation notifications do not roll back a committed invocation.

MCP frames are limited to 256 KiB. Owner CLI JSON inputs (including migration manifests) are limited to 5 MiB. Serialized domain results above 1 MiB fail explicitly (text and structured copies add protocol overhead); use narrower scopes, searches or bounded memory context. A result-size error does not undo a recorded invocation: retry with the same request ID to recover its receipt, then inspect it as owner. No stdout banners or secret logging are part of MCP output.

## Review, undo and revocation

External invocations appear in saved Agent history/task discovery with an `External:` producing-model label. Inspect each pending proposal in the existing review surface, then accept or dismiss it. The desktop event bridge polls a bounded cross-process event outbox so it can discover new external runs without restarting or replacing an unsent goal. Outbox delivery is advisory; persisted state remains authoritative. The CLI also supports explicit owner review:

```sh
./target/release/sthira-cli --workspace /absolute/path/to/vault review show RUN_ID
./target/release/sthira-cli --workspace /absolute/path/to/vault review apply RUN_ID STEP_ID
./target/release/sthira-cli --workspace /absolute/path/to/vault review dismiss RUN_ID STEP_ID
./target/release/sthira-cli --workspace /absolute/path/to/vault review undo RUN_ID STEP_ID
```

For a title proposal, append the exact reviewed title to `review apply`. Undo availability and guards are the same as the native action family; this command does not add Undo where the underlying workflow has none. Newer source edits, membership changes or project metadata can reject approval/recovery. Repeated approval cannot consume one proposal twice. Existing author/source attribution, revisions, change sets, events and mirror behavior are preserved.

```sh
./target/release/sthira-cli --workspace /absolute/path/to/vault access revoke CONNECTION_ID
```

Revocation takes effect on the next request in an already-running server and is rechecked under each proposal/approval transaction. Previously pending proposals become ineligible for acceptance; they remain inspectable and dismissible. Revocation does not erase previous work or prevent the owner from undoing eligible earlier changes. A cleared vault removes connection records and rotates its instance identity; an old server/token cannot access replacement data. Connection secrets cannot be recovered from `access list`; create a new connection if a token is lost. Backups contain hashed grants and may restore their historical revocation state, so review connections after restoring a backup.

Legacy owner commands (`home`, `capture`, `route`, `project`, `resume`, `history`, `reconcile`, `audit`) remain available. They are local owner commands, not MCP tools. Their former implicit first-use vault creation is replaced by explicit `init`; attachment no longer runs native agent recovery. CLI generation has no configured inference provider and reports that limitation rather than fabricating a model reply.

## Verification boundaries

Core integration tests use disposable vaults and panic-on-use model fixtures: scoped reads/proposals, read-only denial, native approval/undo, immutable attribution, source staleness, revocation during a batch and at native write boundaries, instance rotation, additive/idempotent migration, ordered notification replay and attachment beside a running task. CLI integration tests launch the real binary and exercise initialization, grants, JSON discovery, submission/retry, owner review/undo, stdio handshake/tool calls, protocol errors, size limits, scoped denial and revocation of an already-running server. Desktop handler tests cover clear/reset of credentials/outbox; mocked UI tests cover newly discovered external history while preserving an unsent goal.

These checks do not certify native GUI end-to-end review, live third-party MCP client compatibility, external model behavior or non-Linux platforms. The outbox is bounded and best-effort; a bridge failure may require an explicit refresh. CLI-owned native-model task execution, remote HTTP/OAuth, client setup UI, session-memory browsing and automatic resumption remain outside this batch. Explicit saved handoffs and owner-controlled return to Sthira are described in [AE4](cross-agent-continuity.md). See [known limitations](../known-limitations.md).
