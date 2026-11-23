# Migration assistant (AE5 local export foundation)

Import existing knowledge through **Settings → Workspace & Data → Import existing knowledge**, or the owner CLI. The guided assistant inventories local exports, explains supported mappings, previews each document and records an explicit import receipt. It is deterministic and model-free; imported content never grants tool permissions or silently becomes shared memory, a recorded decision, or adopted AI direction.

## Source preparation

- **Notion:** Export Markdown & CSV, extract the ZIP locally, then choose the Markdown files or export folder. CSV database tables, attachments, HTML and PDF are reported as unsupported. See [Notion export documentation](https://www.notion.com/help/export-your-content).
- **Obsidian:** Choose exported/copied Markdown notes or a vault folder. Relative Markdown links and unambiguous wiki links resolve among the imported/previously imported files in the selected collection. Embeds, anchors, ambiguous/missing links and media stay literal with warnings. See [Obsidian internal links](https://obsidian.md/help/links).
- **Google Docs:** Download Markdown or plain text, then select the downloaded files. Rich document formats, comments, suggestions, permissions and revision histories are outside this batch. See [Google Docs Markdown export](https://support.google.com/docs/answer/12014036).
- **Other sources:** UTF-8 Markdown and plain-text exports use the same bounded workflow. No account login, live API connector, archive extraction or arbitrary directory access is provided to models/MCP clients.

The source preset records provenance and preparation guidance; it does not claim a source-specific rich-format converter. Keep original exports as backups. Import reads file contents selected through the local file/folder chooser and never writes those source files. Choose up to 200 files, 256 KiB per text document, 4 MiB of text per batch. Unsupported formats, hidden files, invalid text, empty files and oversized documents are reported. Split larger exports into smaller batches.

## Preview and mapping

Choose a stable **collection identity** and a destination project or **Standalone pages**. The source preset, collection and normalized relative path identify a source document across repeated imports. Keep those values stable for repeat detection; moving/renaming an export path is a new source identity. Folder selection preserves relative paths. For nested exports prefer folder selection over flattening file names.

The inventory shows new, already imported, conflicting and unsupported files. Expand a document for its title, identity, warnings and literal document preview. A Markdown heading supplies the title when available; otherwise the file name supplies it. Frontmatter remains document text and is never used to supply Sthira IDs, authorship or object schemas. Source paths cannot escape the export collection or select a destination filesystem path.

**Import new documents** explicitly accepts all new files in the preview. File selection happens before preview; choose another selection/new preview to omit a document. Unsupported/duplicate/conflicting entries are not imported. Existing pages are never silently overwritten. Changed source files in an existing collection are reported as conflicts; review them manually or intentionally use a new collection identity for a separate copy. Incremental batches add new paths; automatic merge, rename detection and source updates remain follow-ups.

Imported pages have **external authorship**, retain source identity/hash and original source text in the receipt, and receive normal page revisions, search indexing, membership, activity events and Markdown mirrors through the shared page creation primitive. This does not assert who originally wrote an export. Existing user/AI/source attribution on other records is unchanged.

Resolved explicit local links become stable `sthira://artifact/ID` links and directional references. Code fences and inline code remain literal. Plain-text files stay literal and do not infer document relationships. No proximity/mention-based links or derivation lineage are manufactured. Missing/ambiguous links, anchors and embeds remain inspectable text, with warnings. Relative media paths are not copied or made available offline. The preview assumes its new files are imported together; unsupported or unavailable targets remain literal. This parser intentionally handles common Markdown links and wiki links, not every Markdown extension.

## Receipts, conflicts and recovery

Preview is saved locally without creating documents. Apply rechecks destination project revision, source identity and exact eligibility in the writer transaction. Page creation, references, membership, provenance and the applied receipt commit together; failure rolls them back. Concurrent acceptance or a repeated identical selection cannot duplicate one batch. Preview request IDs are retry-safe; different input cannot reuse a receipt. **Cancel preview** persists cancellation without importing. Apply is a bounded atomic operation; it does not expose mid-commit cancellation or automatic background resumption.

Saved receipts remain inspectable after reopen. The batch picker lists the latest 50 receipts; older receipts can be read through the owner CLI by saved import ID. History currently loads full plans, so many large batches can make this picker expensive. Markdown mirror publication happens after database commit and reports warnings instead of pretending the entire import failed. Existing differing mirror files/symlinks and newer saved page versions are preserved. Pending mirror warnings are committed with page creation, so a process stop or failed post-commit receipt update cannot falsely certify mirror success; the current response also retains actual publication warnings. A mirror warning does not authorize repeating page creation; use normal save/mirror repair against the existing saved identity.

**Roll back unchanged import** soft-deletes only the pages created by that batch, removes their imported references/memberships and releases its source keys for a fresh import. Original exports and unrelated Sthira pages remain intact. Guarded rollback rejects newer document/title/authorship/type changes, pins, drafts, object conversion, changed-and-restored memberships/references, later substantive activity, memory/agent/handoff/canvas/evidence reuse or externally edited mirrors. Use normal page actions for work already used or edited. Prior content/history and the receipt remain in the database; rollback is not permanent erasure. Mirror cleanup is post-commit and authoritative tombstones prevent reconciliation from reviving deleted identities.

SQLite **v27** adds migration plans/receipts and source identity mappings, preserving existing data by additive/idempotent upgrade. Backups contain these records and source content. Clear vault data removes migration records/source mappings before clearing the rest of the workspace; it does not touch original export files.

## Owner CLI

Build the existing CLI and use its actual binary path. These owner commands are absent from MCP tools; connection credentials cannot grant filesystem import or bypass review.

```sh
sthira-cli --workspace /absolute/path/to/vault migration preview @/absolute/path/to/manifest.json
sthira-cli --workspace /absolute/path/to/vault migration list
sthira-cli --workspace /absolute/path/to/vault migration read IMPORT_ID
sthira-cli --workspace /absolute/path/to/vault migration apply IMPORT_ID CURRENT_REVISION
sthira-cli --workspace /absolute/path/to/vault migration cancel IMPORT_ID CURRENT_REVISION
sthira-cli --workspace /absolute/path/to/vault migration rollback IMPORT_ID CURRENT_REVISION
```

Example owner manifest (contents are explicitly supplied; no paths are read by the core importer):

```json
{"request_id":"export-preview-001","source":"obsidian","collection":"research-vault","project_id":null,"files":[{"path":"Question.md","content":"# Open question\nThe date remains uncertain."},{"path":"database.csv","content":null,"problem":"CSV database mapping is unsupported."}]}
```

Presets are `notion`, `obsidian`, `google_docs`, `markdown`. Source references imported through this flow immediately work with AE1 shared memory, AE2 read/edit proposals, AE3 scoped CLI/MCP reads and AE4 handoffs. Subsequent source edits invalidate captured memory/handoff bases; migration rollback cannot discard that newer cross-agent work.

## Verification scope

Temporary-vault engine tests cover all four presets, Unicode, source immutability, code/link preservation, explicit references, incremental links to earlier collection batches, multi-backtick/nested-fence code preservation, duplicate retries, changed-source conflicts, atomic receipt failure, stale destination, cancellation, mirror collisions, guarded rollback, reopen and v26 upgrade. Real CLI transport tests cover preview/apply/retry/rollback. A combined migration → memory → scoped external proposal → handoff review test checks external attribution and stale-source barriers. The full Rust workspace and UI suites exercise AE1–AE5 together; isolated browser fixtures exercise the new review controls separately from persistent native behavior.

Native Tauri GUI end-to-end import/restart, real exported workspaces at large scale, live connector fidelity, assistive technology, native display scaling and non-Linux platforms remain unverified. Direct Notion/Google OAuth, ZIP/media import, HTML/DOCX/PDF conversion, database property mapping, rich comments/history, automatic source merging and model-led migration planning are not included. This is the local-export migration foundation, not a guarantee of complete source-application fidelity.
