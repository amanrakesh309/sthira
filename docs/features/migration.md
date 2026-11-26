# Migration assistant (AE5 reviewed imports and guidance)

Import existing knowledge through **Settings → Workspace & Data → Import existing knowledge**, or the owner CLI. The guided assistant inventories local exports, explains supported mappings, previews each document and records an explicit import receipt. The import engine is deterministic and model-free; optional migration-agent guidance is a separate, explicit request. Imported content never grants tool permissions or silently becomes shared memory, a recorded decision, or adopted AI direction.

## Source preparation

- **Notion:** Export Markdown & CSV, extract the ZIP locally, then choose the Markdown files or export folder. CSV database tables, attachments, HTML and PDF are reported as unsupported. See [Notion export documentation](https://www.notion.com/help/export-your-content).
- **Obsidian:** Choose exported/copied Markdown notes or a vault folder. Relative Markdown links and unambiguous wiki links resolve among the imported/previously imported files in the selected collection. Embeds, anchors, ambiguous/missing links and media stay literal with warnings. See [Obsidian internal links](https://obsidian.md/help/links).
- **Google Docs:** Download Markdown or plain text, then select the downloaded files. Rich document formats, comments, suggestions, permissions and revision histories are outside this batch. See [Google Docs Markdown export](https://support.google.com/docs/answer/12014036).
- **Other sources:** UTF-8 Markdown and plain-text exports use the same bounded workflow. No account login, live API connector, archive extraction or arbitrary directory access is provided to models/MCP clients.

The source preset records provenance and preparation guidance; it does not claim a source-specific rich-format converter. Keep original exports as backups. Import reads file contents selected through the local file/folder chooser and never writes those source files. Choose up to 200 files, 256 KiB per text document, 4 MiB of text per batch. Unsupported formats, hidden files, invalid text, empty files and oversized documents are reported. Split larger exports into smaller batches.

## Preview and mapping

Choose a stable **collection identity** and a destination project or **Standalone pages**. The source preset, collection and normalized relative path identify a source document across repeated imports. Keep those values stable for repeat detection; moving/renaming an export path is a new source identity. Folder selection preserves relative paths. For nested exports prefer folder selection over flattening file names.

The inventory shows new, already imported, conflicting and unsupported files. Expand a document for its title, identity, warnings and literal document preview. A Markdown heading supplies the title when available; otherwise the file name supplies it. Frontmatter remains document text and is never used to supply Sthira IDs, authorship or object schemas. Source paths cannot escape the export collection or select a destination filesystem path.

**Import selected documents** explicitly accepts the checked new files. Use per-file checkboxes, **Select all new files** or **Clear selection** after preparing a preview. Expand selected documents to inspect their exact content. Selection changes request a fresh native preview; import stays unavailable until that response matches the current selection. Links to omitted targets stay literal with warnings. Apply rechecks the reviewed selection fingerprint inside the write transaction, so changed link availability or destination/source conflicts cannot silently change what was reviewed. The receipt and reopened applied plan retain the exact imported previews. Checkbox choices before Apply belong to the current review; reopening a pending batch initially selects its new files again, so review the selection before importing. Guidance retains its own captured selection and identifies a mismatch with the current one. Unsupported/duplicate/conflicting entries are not imported. Existing pages are never silently overwritten. Changed source files in an existing collection are reported as conflicts; review them manually or intentionally use a new collection identity for a separate copy. Incremental batches add new paths; automatic merge, rename detection and source updates remain follow-ups.

Imported pages have **external authorship**, retain source identity/hash and original source text in the receipt, and receive normal page revisions, search indexing, membership, activity events and Markdown mirrors through the shared page creation primitive. This does not assert who originally wrote an export. Existing user/AI/source attribution on other records is unchanged.

Resolved explicit local links become stable `sthira://artifact/ID` links and directional references. Code fences and inline code remain literal. Plain-text files stay literal and do not infer document relationships. No proximity/mention-based links or derivation lineage are manufactured. Missing/ambiguous links, anchors and embeds remain inspectable text, with warnings. Relative media paths are not copied or made available offline. The initial inventory assumes new files are imported together; the selected-document preview recalculates against the chosen files and live earlier imports. Unsupported, omitted or unavailable targets remain literal. This parser intentionally handles common Markdown links and wiki links, not every Markdown extension.

## Optional migration-agent guidance

1. Enable **Workspace agent** in Settings → Workflows and configure an available model. Manual import stays available with that workflow off or with no model.
2. Select 1–50 new files in the prepared review. In **Plan this import with the agent**, choose the guidance model, enter optional instructions (up to 2,000 characters), and inspect the provider label.
3. The default request includes only selected paths, titles, bounded warnings and instructions. Source excerpts are off by default; explicitly opting in includes up to 500 characters per selected source. The total prompt is capped at 40,000 characters. Context is sent to the selected configured provider; a locally named connection does not prove local processing.
4. Choose **Request migration guidance**. The native app owns a saved, cancellable background task. This specialized agent returns a structured summary, per-file page/review/omit suggestions, optional organization-group labels and cautions. It has no filesystem, web, memory, workspace-read or write tools. It cannot convert unsupported formats, create projects or import documents.
5. Review the producing model/provider and suggestions. **Use suggested file selection** explicitly removes files marked `review_first` or `leave_out`; unspecified files keep their current selection. Group labels are suggestions, not created destinations. The selected preview is recalculated again, and **Import selected documents** is still a separate owner action. Suggestions for an earlier selection cannot be adopted into a changed selection.

Guidance accepts only paths from its captured selection and a bounded, strictly validated response. Malformed responses, unknown actions/paths, unavailable models and source/destination changes fail honestly without creating pages. Request IDs make unknown-response retries return the saved task without dispatching a second model call. Reading saved guidance never invokes a model.

Progress and **Stop guidance** live in the migration review, not the global workspace-task list. Closing Settings or switching batches does not stop the owned worker; reopening a batch retrieves its latest saved guidance. Native Stop drops the inference future; cancelling/applying an import cancels active guidance state, and late responses cannot publish. Cross-process cancellation is durable, while an already dispatched provider request may finish or time out before its worker observes it. The per-request overall deadline is 185 seconds, with existing provider connection/generation budgets. Allow one active guidance task per batch and three per vault. App startup marks unfinished tasks interrupted without automatic model dispatch; explicitly request new guidance to continue. Reset cannot resurrect cleared jobs.

## Receipts, conflicts and recovery

Preview is saved locally without creating documents. Apply rechecks destination project revision, source identity and exact eligibility in the writer transaction. Page creation, references, membership, provenance and the applied receipt commit together; failure rolls them back. Concurrent acceptance or a repeated identical selection cannot duplicate one batch. Preview request IDs are retry-safe; different input cannot reuse a receipt. **Cancel preview** persists cancellation without importing. Apply is a bounded atomic operation; it does not expose mid-commit cancellation or automatic background resumption.

Saved receipts remain inspectable after reopen. **Saved import batches** loads ten lightweight summaries per page, with Previous/Next and Refresh controls; opening a batch fetches its full plan on demand. Keyset cursors use creation time and stable ID, keeping traversal deterministic when timestamps tie. Summary metadata is stored separately from source/preview bodies, so listing history does not deserialize all full plans. The owner CLI offers the same paginated history, with page sizes from 1 to 50. Markdown mirror publication happens after database commit and reports warnings instead of pretending the entire import failed. Existing differing mirror files/symlinks and newer saved page versions are preserved. Pending mirror warnings are committed with page creation, so a process stop or failed post-commit receipt update cannot falsely certify mirror success; the current response also retains actual publication warnings. A mirror warning does not authorize repeating page creation; use normal save/mirror repair against the existing saved identity.

**Roll back unchanged import** soft-deletes only the pages created by that batch, removes their imported references/memberships and releases its source keys for a fresh import. Original exports and unrelated Sthira pages remain intact. Guarded rollback rejects newer document/title/authorship/type changes, pins, drafts, object conversion, changed-and-restored memberships/references, later substantive activity, memory/agent/handoff/canvas/evidence reuse or externally edited mirrors. Use normal page actions for work already used or edited. Prior content/history and the receipt remain in the database; rollback is not permanent erasure. Mirror cleanup is post-commit and authoritative tombstones prevent reconciliation from reviving deleted identities.

SQLite **v27** added migration plans/receipts and source identity mappings. **v28** adds lightweight summary metadata/indexes and saved guidance tasks, backfilling existing plans through an additive/idempotent upgrade. Backups contain these records and source content, including opted-in guidance context. Clear vault data cancels native guidance tokens and removes guidance/import records and source mappings before clearing the rest of the workspace; it does not touch original export files.

## Owner CLI

Build the existing CLI and use its actual binary path. These owner commands are absent from MCP tools; connection credentials cannot grant filesystem import or bypass review.

```sh
sthira-cli --workspace /absolute/path/to/vault migration preview @/absolute/path/to/manifest.json
sthira-cli --workspace /absolute/path/to/vault migration list
sthira-cli --workspace /absolute/path/to/vault migration history 10
sthira-cli --workspace /absolute/path/to/vault migration history 10 '{"created_at":1000,"id":"LAST_ID_FROM_NEXT_CURSOR"}'
sthira-cli --workspace /absolute/path/to/vault migration read IMPORT_ID
sthira-cli --workspace /absolute/path/to/vault migration review IMPORT_ID CURRENT_REVISION '["Question.md"]'
sthira-cli --workspace /absolute/path/to/vault migration apply-selected IMPORT_ID CURRENT_REVISION @/absolute/path/to/approval.json
# Legacy explicit all-new import remains available:
sthira-cli --workspace /absolute/path/to/vault migration apply IMPORT_ID CURRENT_REVISION
sthira-cli --workspace /absolute/path/to/vault migration cancel IMPORT_ID CURRENT_REVISION
sthira-cli --workspace /absolute/path/to/vault migration rollback IMPORT_ID CURRENT_REVISION
```

The selected-import approval JSON contains `paths` and the exact `basis` returned by `migration review`: `{"paths":["Question.md"],"basis":"REVIEWED_BASIS"}`. `list` now returns the first 50 lightweight summaries; `history` returns `items` and `next_cursor`. Full plans remain available through `read`. Guidance generation currently uses the native migration UI; these commands do not grant it to MCP clients.

Example owner manifest (contents are explicitly supplied; no paths are read by the core importer):

```json
{"request_id":"export-preview-001","source":"obsidian","collection":"research-vault","project_id":null,"files":[{"path":"Question.md","content":"# Open question\nThe date remains uncertain."},{"path":"database.csv","content":null,"problem":"CSV database mapping is unsupported."}]}
```

Presets are `notion`, `obsidian`, `google_docs`, `markdown`. Source references imported through this flow immediately work with AE1 shared memory, AE2 read/edit proposals, AE3 scoped CLI/MCP reads and AE4 handoffs. Subsequent source edits invalidate captured memory/handoff bases; migration rollback cannot discard that newer cross-agent work.

## Verification scope

Temporary-vault engine tests cover all four presets, Unicode, source immutability, code/link preservation, explicit references, incremental links to earlier collection batches, multi-backtick/nested-fence code preservation, duplicate retries, changed-source conflicts, atomic receipt failure, stale destination, cancellation, mirror collisions, guarded rollback, reopen and v26 upgrade. Additional v28 tests cover exact selected previews, omitted links, stale review bases, keyset history, real prior-schema backfill, scoped guidance prompts, opt-in excerpts, malformed/hostile model actions, unknown-response retries, workflow revocation, cancellation and interruption. Desktop handler tests cover owned guidance workers, workflow gates and reset. Real CLI transport tests cover preview/apply/retry/rollback, pagination and selected review/application. A combined migration → memory → scoped external proposal → handoff review test checks external attribution and stale-source barriers. The full Rust workspace and UI suites exercise AE1–AE5 together; isolated browser fixtures exercise the new review controls separately from persistent native behavior.

Hand-authored export-shaped fixtures cover nested Notion Markdown/CSV/frontmatter, Obsidian Unicode/alias/code/embed cases and Google Docs Markdown tables/images/plain text, verifying source bytes before/after import, retry and recovery. They are synthetic format fixtures, not private account exports or live conversion evidence. Native Tauri GUI end-to-end import/restart, live guidance-model quality, actual source-account exports at scale, connector fidelity, assistive technology, native display scaling and non-Linux platforms remain unverified. Direct Notion/Google OAuth, ZIP/media import, HTML/DOCX/PDF conversion, database property mapping, rich comments/history, automatic source merging and automatic application of model mappings are not included. Guidance and selective review complete the current AE5 batch; they do not certify complete source-application fidelity.
