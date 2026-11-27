# Shared workspace backup and recovery

Open **Recovery** in a team workspace. Owners and Admins can create a saved snapshot, export a portable `.sthira-backup.json` file through the Linux file chooser, and remove saved snapshots after confirmation. Current members can inspect restored history. Delegated agents cannot perform backup or restore operations.

Saved snapshots use the same service storage as the workspace. **Export a file and keep a separate copy** to recover if that service storage is lost. Exports contain team content in plain JSON, are not encrypted, and are created with owner-only file permissions on Linux. A checksum detects corruption; it does not authenticate the file’s origin.

## Restore into an empty workspace

1. Create or select an empty team workspace. Add the members and roles you want there using normal invitations.
2. Choose **Recovery → Choose backup file**. Sthira validates the format locally before upload, then previews the source, exact target, content counts, member count, and file identity. Previewing does not publish content.
3. Review the target and confirm the checkbox, then choose **Restore workspace**. The preview expires after 15 minutes.
4. Inspect **Pages**, **Projects**, discussions, and **Inspect recovery history** from both accounts. Keep the original file.

The service rechecks membership, Owner/Admin permission, the active device session, target emptiness, and target name/membership at commit. Changed targets require another preview, including changes that are later reversed. Tickets belong to the initiating account and device session. A restore is atomic: failure leaves no partial content, and retrying the same ticket returns the same receipt without duplicating or overwriting later work. Nonempty workspaces cannot be restored over. File selection can be cancelled without importing anything.

## Included content and attribution

The snapshot captures published pages and every revision, archived pages/projects, project membership links, and page/project discussions at one service transaction boundary. IDs are regenerated and relational links remapped. Discussion order is preserved, including equal-timestamp messages.

Proposal submissions, human decisions, agent suggestions, activity, and acknowledged collaborative draft text are retained as **read-only source history**. They are not active proposals, current approvals, agent grants, or resumable collaborative sessions. **Copy saved draft** lets a member explicitly start a fresh proposal and independent review. Unacknowledged edits on other devices are not in a service snapshot.

Source display labels remain visible, but imported actor identities are detached from current accounts. The restored-workspace notice identifies historical attribution and decisions as unverified source records. A file cannot grant an imported author permissions over a current member’s discussions. Existing target memberships and roles remain unchanged. Restored revision links do not activate historical proposals.

Backups of restored workspaces retain the earlier recovery records and remap their page/project identities through subsequent restores. Inline Markdown links keep their original text; links that encode old shared IDs may need updating. Restore enforces the current shared-content policy and rejects incompatible or unsafe content rather than silently changing it.

## Excluded data and limits

Account/device sessions, credentials, invitations, workspace memberships/roles, delegated agent connections and tokens, provider keys, presence/cursors, raw Yjs operations, local device run state, and Personal vault data are excluded. Removing a saved snapshot does not delete exported copies or workspace content.

Export format **version 2** adds resource classification: each project's workspace/restricted mode and each page's workspace/follow-project/selected mode with its access-defining project. Audience grants are not exported, so a restore never recreates source authority. Restored restricted projects and pages are available only to the target workspace's Owners and Admins until a fresh explicit share is made; a page whose mapped access-defining project is missing fails closed.

**Version 1 remains readable** but has no access metadata. Preview now requires an explicit interpretation: the safe **administrator-only** default, or an explicit Owner/Admin opt-in to the earlier **workspace-visible** journey. A client that sends a bare version-1 file receives a review-required conflict instead of an implicit choice. Recovery receipts, resource maps, counts and history rows are filtered by current access for ordinary members; hidden or unmapped records remain administrator-only.

Versions 1 and 2 accept up to 16 MiB and 25,000 content/history records, with at most 32 retained history sets. A workspace can keep 20 saved snapshots totaling 64 MiB. Export and explicitly remove older saved snapshots when a limit is reached. Local validation and service validation reject altered checksums, unknown versions/groups, invalid relationships, and malformed structured metadata.

This is a local Linux pilot with a logical, versioned content format. The production shared database, hosting, scheduled backups, external backup storage, encryption/key management, large-workspace streaming, and non-Linux file selection remain open. It is not a complete operator backup of the authentication service. Restoring a file requires a working sign-in service and an authorized empty target workspace. Live web Research and Inkling 2 Mini remain deferred.
