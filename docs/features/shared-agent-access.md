# Scoped shared agent access (team workspaces)

Scoped team access lets a member connect an external MCP client to **one shared project** with an
explicit page allowlist, without granting the client a Sthira account session, another member's
identity, or any ability to approve or publish.

This contract covers the service, external MCP adapter, desktop connection setup, attributed
proposal review and scoped native runs. Acceptance remains limited to the executed test layers;
see [current limits](#current-limits).

## Contract

Each shared connection is server-owned state in `sthira-session-service` and binds:

- the authenticated member and the **issuing device session**;
- one workspace and one shared project;
- an explicit allowlist of shared page IDs (1–100);
- read access, optionally with a suggestion capability;
- a stable connection ID, generation, timestamps and revocation state.

Caller identity always comes from trusted credentials. A delegated connection token is an opaque
`stha_…` secret; only its SHA-256 hash is stored (`shared_agent_connection_tokens`), following the
existing device-session token convention. Tool arguments may name a page or project, but never the
acting member, device, role, permission or reviewer.

An **Owner or Admin** enables agent access for the workspace. An active member may then create a
connection for their own account; new connections default to **Can read**. Requesting **Can read and
suggest changes** requires an Editor, Admin or Owner role.

Every request re-derives the intersection of:

current membership + role · issuing device session · workspace policy · connection grant and
generation · project availability · explicit page allowlist · current `shared_project_pages`
membership generation · page archive state · the initiating member’s current human project/page access. A broader agent grant never overrides a narrower human audience, and a broader human audience never silently expands an existing agent grant; both are re-read before reads, context assembly, proposal submission and review.

The explicit allowlist stores the project-page `membership_id` and `generation` it was granted on.
Detaching a page ends access immediately. Reattaching it creates a new generation, so the old grant
and any proposal basis built on it stay invalid until the member explicitly updates the connection.
Attaching a new page to the project never extends an existing grant.

## Service endpoints

Member-authenticated (device-session bearer):

| Method | Path | Notes |
| --- | --- | --- |
| GET | `/v1/workspaces/{ws}/agent-access` | Policy state (default disabled) |
| PUT | `/v1/workspaces/{ws}/agent-access` | Owner/Admin enables or disables |
| GET | `/v1/workspaces/{ws}/agent-connections` | Own connections; `?scope=workspace` for Owner/Admin |
| POST | `/v1/workspaces/{ws}/agent-connections` | Returns the credential exactly once |
| GET | `/v1/workspaces/{ws}/agent-connections/{id}` | Owner, Admin or the owning member |
| PATCH | `/v1/workspaces/{ws}/agent-connections/{id}` | Full page/permission update, bumps generation |
| DELETE | `/v1/workspaces/{ws}/agent-connections/{id}` | Owner/Admin or owning member |
| GET | `/v1/workspaces/{ws}/agent-proposals` | Permission-scoped, attributed review list |
| GET | `/v1/workspaces/{ws}/agent-proposals/{id}` | Details, staleness and review eligibility |
| POST | `/v1/workspaces/{ws}/agent-proposals/{id}/review` | Apply / request changes / dismiss |

Delegated connection token (bearer `stha_…`):

| Method | Path | Notes |
| --- | --- | --- |
| GET | `/v1/agent-access/status` | Grant, project, available pages and limits |
| GET | `/v1/agent-access/pages?q=&cursor=&limit=` | List/search inside the allowlist only |
| GET | `/v1/agent-access/pages/{id}` | Complete page + revision ID, number, SHA-256 |
| POST | `/v1/agent-access/proposals/edit` | Requires suggest + a complete prior read |
| POST | `/v1/agent-access/proposals/new` | Target project is the connection's project |
| GET | `/v1/agent-access/proposals/{id}` | This connection's proposal only |
| GET | `/v1/agent-access/requests/{request_id}` | Scoped idempotency receipt |

Foreign workspace, project and page IDs return the same `404 page_not_found` /
`project_not_found` codes as missing IDs; search, snippets, pagination and counts are all filtered by
the authorization join before retrieval. No cross-project or Personal content is readable.

## Proposals and review

Edit proposals reuse `shared_change_proposals` and the existing service review rules. New-page
proposals use `shared_agent_page_proposals` because the CW2 proposal table requires an existing page.

At submission the service snapshots an immutable source basis: every page the connection has fully
read, with its revision number, content SHA-256 and project-page membership generation. A truncated
or stale read cannot support a proposal. A proposal records:

- target page and base revision (edits) or target project (new pages);
- every source identity and revision;
- original authorship, the initiating member, and the connection/agent label;
- declared model and provider plus `modelTrust: "declared_unverified"` for external clients.

Review is enforced in the service:

- only an Owner or Admin may apply;
- the initiating member and any recorded content contributors are ineligible reviewers;
- expected submission number and current revision are rechecked;
- Apply rechecks policy, connection revocation, grant generation, initiating membership, project
  archive state and every recorded source revision **inside the Apply transaction**. Both review
  routes check the initiating device session and suggestion permission, including immediately
  before commit. A failed initiating-session check keeps the reviewer's session active;
- one revision or exactly one new page is written with the decision, attribution, Activity,
  idempotency receipt and audit row;
- retries with the same idempotency key return the same result; changed payloads fail with
  `idempotency_conflict`;
- revoked or stale proposals remain inspectable and can be dismissed (rejected) or revised against a
  fresh basis. Agent proposals cannot be resubmitted on the old record; the agent reads the current
  page again and submits a new proposal with a fresh request ID. Dismissal never changes published
  content.

Members retain inspection access to workspace proposals. An agent's status and replay requests
recheck its current page allowlist before returning old proposal content. Request IDs are unique
within a connection across proposal kinds.

Agents have no approval, publish, reviewer-impersonation, grant-administration, shell or SQL tools.

## MCP tools

A shared connection runs `sthira-cli shared-mcp --connection-file <private file>`. The Personal
`mcp` command and its four existing tools are unchanged.

- `sthira_shared_status`
- `sthira_shared_pages` — `list`, `search`, `read`
- `sthira_shared_propose` — `edit`, `new_page`
- `sthira_shared_request` — by request or proposal ID

The delegated credential is held in a private (0600) connection file and sent only as a bearer
header. It is absent from process arguments, tool results, MCP configuration snippets and logs.
Service URLs require HTTPS, with HTTP allowed for local loopback fixtures. The transport disables
implicit proxies and redirects and limits response bodies to 1 MiB. Model/provider labels are client
declarations.

## Lifecycle behavior

Verified by `crates/sthira-session-service/tests/agent_access_test.rs`:

- connection revocation (own or workspace-admin), issuing-device revocation, session replacement,
  member removal, workspace policy disablement, Viewer role downgrade, project/page archive,
  page detachment, and reattachment;
- Viewer downgrade saves a read-only grant and invalidates prior read bases. Restoring the member's
  role requires an explicit grant update to allow suggestions again;
- disabling the workspace policy or archiving a project invalidates prior proposal generations.
  Member removal permanently revokes the old connections. Archiving a page removes it from the
  allowlist. Re-enabling, restoring, or rejoining preserves the invalidation of old proposals;
- updating a grant clears its read basis so the next proposal requires fresh authorized reads;
- any request already running when access changes obeys the service ordering because the binding,
  membership and grant are re-read in the same transaction and again before commit where the
  contract requires it.

Audit rows record actor, connection, resource, outcome and proposal identity, capped at 500 rows per
workspace. Credentials and unrelated Personal content are never recorded.

## Desktop setup and review

In **Settings → Agent connections**, choose **Shared workspace**, then select a workspace. The
workspace's **Agents** tab exposes the same controls without opening Settings. An Owner/Admin
first enables workspace agent access. Create a connection by choosing a shared project, selecting
its pages, choosing the client, and selecting a permission. **Can read** is the default. Pages added
to the project later are never included automatically.

Connection credentials are saved in private, account-and-service-scoped files on the current Linux
device (0600 files, 0700 containing directories). Generated JSON/OpenCode/Codex TOML snippets point
to that private file through the desktop executable's `--sthira-shared-mcp` launcher. They never
contain the token or a Personal vault path. Merge the snippet into the chosen client's configuration
and reconnect that client's MCP server. Each connection has a distinct configuration key. Native
connections need no MCP configuration step; their controls are available in the workspace.

**Change scope** requires the current grant generation and invalidates prior read bases and pending
proposals. **Disconnect** revokes access in the service before removing the local credential. A
running external MCP process loses access on its next request. Other connections remain independent.
Old client configuration entries may remain visible until removed manually, but cannot authorize
requests after revocation. Creating a grant does not prove that an external client has connected.

The review list supports edit and new-page proposals. Review shows the initiating human, agent,
connection, reported model/provider, destination project, proposed rich Markdown, and source page
identities/revisions/hashes. Edits can compare the immutable saved base or current published content. Source buttons inspect the
exact saved revision, verify its revision/hash identity, and preserve the review; opening the current
source page is a separate action.
Sharing invalidations and workspace role changes reauthorize an open review, close its source
snapshot and prevent older pending source reads from reopening it. Denied reviews clear their
cached proposal and comparison bodies.
Only an independent eligible Owner/Admin may apply. Request changes and dismissal preserve published
content. Reviewer notes remain visible to the initiating member after a decision. Activity and page proposal links route by stable proposal identity into attributed review.
Native source views retain their originating project context.

## Native scoped runs

Choose a connection, enabled model, goal, and operation in **Run Sthira on shared pages**:

- **Answer from selected pages** returns a local answer without creating a proposal or page.
- **Suggest an edit** reads a selected target and submits its complete proposed replacement against
  the saved revision/hash.
- **Suggest a new page** submits a draft for the connection's project.

The Agent workflow must be enabled. The native adapter resolves the actual configured model/provider
at dispatch, reads complete authorized pages through the delegated service contract, and sends only
those pages and the goal to that model. It has no Personal retrieval, web, shell, publishing or
approval tools. Source text cannot widen the grant. Total context is capped at 120 KB; oversized,
malformed, empty or failed model responses do not create successful proposals. Current account,
connection policy and grant generation are checked again before submitting. Apply still rechecks
all saved source revisions inside the human review transaction.

Runs persist stages, source receipts, actual dispatched model/provider, result or failure, and the
exact proposal request body in private storage bound to the account and service. Navigation does
not stop a queued run. Sign-out cancels active inference. Cancellation can prevent submission until
the submission commit boundary has begun; the UI reports when submission has already started.
Restarted runs are shown as interrupted and require explicit **Recover shared run**. Recovery resends
an already saved proposal payload with the original request ID, preventing duplicate publication or
proposal creation after an ambiguous network response. It never silently resumes inference.

The run table is specific to the current account on this device. Account switching clears the
previous view and rejects late results. Shared proposal history remains service-owned and is visible
to authorized workspace members independently of local run files.

## Current limits

- **Production:** shared database and hosting choices remain open. The local pilot uses the existing
  session service and disposable SQLite stores; it does not choose a production shared database.
- **Live providers:** real Google/Firebase acceptance for this batch, live web Research and Inkling 2
  Mini remain deferred. Deterministic model responses test execution and authorization, not model
  quality or browsing capability.
- **Platforms/clients:** Linux is the native acceptance target. Non-Linux builds, every external
  client's configuration dialect/version, and signed-in client chat acceptance require their own
  checks. The generic stdio launcher can be tested independently of an installed client.
- **Model attribution:** the native host records its resolved model/provider. The service deliberately
  treats client-reported model labels, including native labels, as `declared_unverified`; it cannot
  attest to which model a remote provider actually ran. No service-verified model claim is made.
- **Conflict with live drafts:** agent publication preserves a competing live draft and stale submit
  requires explicit rebase. The corresponding service regression remains required alongside CW3.


Resource audiences also constrain search snippets, connection status, proposal review and Activity.
Page read/proposal bases record access versions and audience digests, and proposals bind the target
project's access version. Accepted new pages use the current intersection of source and destination
audiences; editing a broader page from narrower sources requires reviewing its sharing first. See
[shared resource permissions](shared-resource-permissions.md) for the access contract and remaining
native verification boundary.
