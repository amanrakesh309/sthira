# Shared project and page permissions (local Linux pilot)

Human resource permissions narrow what a workspace member can see and change. They never raise a
workspace role. Personal content stays outside this model, and the production shared database and
hosting decisions remain open.

## Effective permission

An operation succeeds only when all of the following hold:

1. The device session is currently valid.
2. The caller has an active membership in the workspace.
3. The caller can currently read the resource (see below).
4. The caller's workspace role permits the operation (Owner, Admin, Editor, Viewer).
5. The resource lifecycle state allows it (not archived where the operation requires an active page
   or project).
6. For a delegated agent, its connection, issuing session, allowlist and generation are still valid.

Owners and Admins always read every project and page in their workspace, including restricted
ones, and can create full workspace backups. Restricting a resource does not hide it from workspace
administrators; the sharing dialog states this explicitly.

## Policies

| Resource | Mode | Visible to |
| --- | --- | --- |
| Project | `workspace` | Every active workspace member |
| Project | `restricted` | Selected active members plus Owners/Admins |
| Page | `workspace` | Every active workspace member |
| Page | `project` | The current audience of one designated same-workspace project |
| Page | `selected` | Selected active members plus Owners/Admins |

A page's access policy is separate from its project-page grouping memberships. Attaching a page to
another project never widens or narrows its policy. A page that follows a restricted project stays
restricted when attached to a workspace-visible project, and a workspace-visible page stays visible
when attached to a restricted project.

Defaults: new regular projects are workspace-visible; a page created with `followProjectId` follows
that project; standalone pages are workspace-visible. Existing pages and projects migrate as
workspace-visible. Legacy multi-project pages are not guessed into a following policy.

## Audience grants and versions

Audience rows store the target `membership_id`, which is unique per membership incarnation. Removing
and rejoining a workspace creates a new membership row, so earlier grants never revive. Grants only
reference existing active members; selecting an audience never creates memberships, invitations or
role changes.

`access_version` increases monotonically on every policy or audience change. A project change also
advances the access version of pages that follow it, including when a later change returns the
audience to an earlier set. Submissions and review bases record the page access version and a stable
digest of the effective audience; a review action made against an older basis fails until the author
resubmits against the current audience.

## Sharing preview and commit

Only Owners and Admins can change project/page policies or audiences. A change is two steps:

1. `POST …/projects/{id}/access/preview` or `POST …/pages/{id}/access/preview` binds the acting
   account and session, resource, base access version, current audience digest, proposed mode and
   audience, dependent page versions and project version, with a five-minute expiry.
2. `POST …/access/commit` rechecks all of that inside an immediate transaction, writes the policy,
   audience and audit row together, and records an idempotency receipt.

Previewing never changes access and does not require advance confirmation. Committing an
expanded audience requires `confirmExpansion: true` in the commit request, after reviewing the
preview. The preview includes effective audiences for workspace and follow-project policies;
conversion of an attached page is also checked for expansion. A legacy preview-level
`confirmExpansion` field is accepted but does not authorize a commit. Restricting a project previews its attached pages
and accepts `convertPageIds` to atomically convert chosen legacy pages to “follow this project”.
Stale previews (concurrent manager, member removal/rejoin, role or policy cycle) return
`stale_membership`; reused previews and changed idempotency payloads are rejected. Retrying an
already committed request returns the original result without duplicating grants.

## Enforcement surface

Filtering happens before serialization, totals and pagination: page/project lists, project details,
pickers, page detail, revisions, discussion threads, proposals, live-draft reads/updates, Activity,
realtime catch-up, delegated agent reads and backups/recovery. Missing and inaccessible resources
share the same `page_not_found` / `project_not_found` response. Activity and realtime rows that
reference hidden pages or projects are removed server-side, not hidden by the client.

Sharing commits also emit a metadata-free workspace invalidation so clients can refresh their
current authorized views without broadcasting hidden resource identities. Idempotent retries
recheck current membership/session and resource visibility before returning cached content.

Workspace event delivery rechecks page/project readability for each delivery; live-draft subscriptions
recheck the page through the live-draft lookup. A sharing commit revalidates open draft subscriptions
and evicts members who lost access. A member who loses access to one resource remains in the
workspace and receives no other content from it.

Sharing invalidations reauthorize open readers, proposal reviews and live editors; a denied read
clears their loaded content and refreshes the filtered directory. Ordinary private drafts flush
pending local edits before unmount cancels their autosave debounce; this never writes to the
denied shared page. Agent reviews close source
snapshots before reauthorization and reject late snapshot responses. Workspace role changes also
refresh capabilities and reauthorize reviews, including Admin-to-Editor loss of implicit access.
An authorized live editor stays open when its role changes without losing page or editing access.

Delegated search and connection-status responses filter page titles and snippets through the
initiating member's current access. Human proposal lists/details and related Activity require
access to the represented sources. Source reads and proposal bases retain page access versions and
audience digests; proposals also retain the target project's access version. Revoking and restoring
access does not revive an older basis.

An accepted agent-created page starts with a selected audience equal to the intersection of its
sources' and destination project's current audiences. Owners/Admins retain administrative access.
It does not inherit workspace visibility merely by being grouped under a workspace-visible project.
An agent edit that would copy narrower sources into a broader existing page is rejected; review the
page's sharing before preparing a new edit.

## Backups and recovery

Export format version 1 preserves the original serialized payload and checksum and remains readable.
Version-2 archives must classify every page and project exactly once with valid policy modes;
missing, duplicate, unknown-resource and invalid classifications are rejected before restore. Version 2 adds resource classification
(project mode; page mode and access-defining project) but never exports audience grants. Restoring a
version-2 archive regenerates identities, remaps policy references and leaves restricted resources
accessible only to target Owners/Admins until a fresh explicit grant is made. A missing mapped anchor
fails closed (selected with no audience).

Version-1 archives have no access metadata and require an explicit interpretation: the safe
default is administrator-only; an Owner/Admin may explicitly opt into workspace visibility to keep
the earlier v1 journey. A client that omits the choice receives a clear review-required conflict.
Recovery receipts, resource maps, counts and history rows are filtered for ordinary members; hidden
or unmapped records are available only to Owners/Admins.

## Service endpoints

| Method | Path | Notes |
| --- | --- | --- |
| GET | `/v1/workspaces/{ws}/access-members` | Owner/Admin audience picker |
| GET | `/v1/workspaces/{ws}/projects/{id}/access` | Current project policy and following pages |
| POST | `/v1/workspaces/{ws}/projects/{id}/access/preview` | Version-bound preview |
| POST | `/v1/workspaces/{ws}/projects/{id}/access/commit` | Atomic commit with receipt |
| GET | `/v1/workspaces/{ws}/pages/{id}/access` | Current page policy |
| POST | `/v1/workspaces/{ws}/pages/{id}/access/preview` | Version-bound preview |
| POST | `/v1/workspaces/{ws}/pages/{id}/access/commit` | Atomic commit with receipt |

Create-project accepts `accessMode` (`workspace` or `restricted`) and `audienceMembershipIds`; a
restricted project requires an Owner/Admin. Create-page accepts `followProjectId`.

## Desktop

**Share** on a project overview or shared page opens the access dialog: workspace/selected-members
(or access-follows-project for pages), member pickers, preview, expansion confirmation and save.
Access badges show workspace, selected members or access-following without revealing an inaccessible
project name. The administrative-access disclosure is always visible in the dialog.

## Using sharing controls

Open a shared project and choose **Share**, or open a shared page and choose **Share** beside its
access badge. Owners and Admins can choose workspace access, selected members, or (for pages)
access that follows a project. Selecting members does not invite them or change their role. Preview
the change, confirm any audience expansion, then save. Saving updates sharing only; it does not
change page text, revisions, attachments or attribution.

A restricted project does not silently restrict every page attached to it. Review the listed pages
and optionally convert chosen pages to follow the project in the same atomic change. Drafts stay
open if you close the dialog, and a stale preview asks you to review again rather than overwriting a
newer decision.

## Role matrix

| Action | Owner | Admin | Editor | Viewer |
| --- | --- | --- | --- | --- |
| Read permitted shared pages, projects, history and discussions | Yes | Yes | Yes | Yes |
| Read every resource in the workspace (administrative) | Yes | Yes | No | No |
| Create a workspace-visible project | Yes | Yes | Yes | No |
| Create a restricted project | Yes | Yes | No | No |
| Change project/page access or audience | Yes | Yes | No | No |
| Attach/detach pages in an accessible project | Yes | Yes | Yes | No |
| Comment, propose, edit live drafts in accessible resources | Yes | Yes | Yes | No |
| Independent apply of another member's proposal | Yes | Yes | No | No |
| Full workspace backup/export/restore | Yes | Yes | No | No |

## Verification status

Implemented and exercised by `crates/sthira-session-service/tests/resource_access_test.rs` and the
existing shared-page, agent and backup suites: migration, policy modes, role boundaries, foreign and
missing IDs, rejoin incarnations, multi-project isolation, anchor detach/archive, ABA preview
staleness, expansion confirmation, competing managers, atomic rollback through SQLite transactions,
version-2 archives and version-1 review requirements.

Native two-account Linux acceptance uses the standalone Tauri app with embedded UI, a real local
session service, independent disposable vaults and Secret Service keyrings, and loopback
OAuth/Firebase identity fixtures. It exercises native sharing preview/confirmation and grants,
multi-project reuse, Viewer/Editor/Admin capabilities, independent and contributor-excluded review,
stale-audience rejection, live-editor/proposal/agent-source revocation, immediate ordinary-draft
revocation with account-scoped local retention, removal/rejoin, native
app/service restarts, A → B → A account switching, file-picker export and v1/v2 recovery. Direct-ID
denials and fixture setup use the real service API as separate cross-checks. These checks do not
establish live-provider acceptance or model inference. The owner's vault remains untouched.

Live identity providers, non-Linux platforms, and production database/hosting acceptance remain
open; see [known limitations](../known-limitations.md). Local verification does not grant owner
product sign-off.
