# Unified note editor and toolbar validation

5 November 2026. Implements the owner's four requested changes after workspace connections.

## Decisions and resulting behavior

1. **Derive automatically connects its result to its source.** The core already writes `derived_from_artifact_id` when it creates a child. Connections now reads that durable lineage directly as a `derived_from` relationship: the child shows **Derived from**, and the source shows the child under **Linked from**. Knowledge Graph includes the same relationship with a readable label. Existing derived notes receive these connections without a migration, duplicate manual link or a second write that could fail after creation. Lineage connections cannot be manually unlinked. The existing parent badge and project inheritance remain.
2. **One editable document replaces Edit / Preview.** A local Tiptap / ProseMirror editor renders and edits headings, formatting, lists, checkboxes, tables, code, links and embedded objects. Markdown remains the saved representation. The separate title input represents the matching leading H1 once; editing it updates the stored heading. Opening a document does not normalize or save its body. Actual content edits serialize to Markdown. Formatting, slash commands, template/object insertion, inline AI insertion, decision recording, autosave and reference previews remain available. The old Cmd/Ctrl+E mode switch is removed.
3. **No theme button in the note header.** Theme controls remain in the application sidebar and Appearance settings.
4. **Canvas and Margin were useful, but did not need header buttons.** Canvas is a working spatial board with independent placement, cards, alignment, links and navigation; it now has a **Spatial canvas** item in expanded and collapsed sidebar navigation, alongside existing project canvases. The margin provides note-specific evidence, claims, provenance and external citations. It now opens from **Page actions → Sources & insights**, keeping that context with the note. History, Connections and save status remain in the header.

Derive and Refine save the current draft before invoking their operations. If saving fails, the note stays open and derivation does not run. Async save results cannot restore the previous document after navigation. Inline generation and derivation completion are scoped to the document request that started them, so late results cannot overwrite or navigate away from a subsequently opened note.

## Editor compatibility

- Uses bundled, MIT-licensed Tiptap 3.32.2 packages; no hosted editor service, account, CDN, font or storage-format migration.
- Uses the library's [Markdown API](https://tiptap.dev/docs/editor/markdown/getting-started/basic-usage) with local extensions for raw HTML preservation, embedded object/diagram views and decision actions.
- Markdown pasted as plain text is parsed when it contains Markdown block/link syntax. Paste inside a code block remains literal. Rich HTML paste uses the editor schema.
- Internal reference links insert at the selection as inline text, without creating a separate paragraph. Clicking them opens Connections. Web links open separately.
- Embedded `sthira-object` blocks retain their identifiers/properties and use the existing live object card. Unknown language fences and raw HTML remain represented in stored Markdown. Raw HTML is shown literally rather than executed.
- Editable note tables use standard table editing. Read-only reference previews continue to use `RichDocumentRenderer`, including its comparison-table filter/copy controls. This is a unified editor, not a claim of complete Notion or Anytype feature parity.

## Verification

- `npm test`: **29 files, 167 tests passed**.
- `npm run build`: TypeScript and Vite passed. The large-bundle advisory remains; the editor adds bundled JavaScript.
- `cargo test --workspace --offline`: **126 tests passed**.
- Focused Rust verification additionally covered connection/derivation/deletion tests: **13 passed**.
- `git diff --check`: passed.

New tests exercise formatted content on initial load without emitting a change, selection formatting and Undo, checklist edits, inline reference insertion/opening, embedded and unknown fenced-block preservation, raw block/inline HTML, echoed autosaves without cursor resets, title-only notes, streamed insertion retaining surrounding text, tables, Markdown paste, derivation waiting for save and save-failure handling, and automatic source/backlink/graph relationships. Existing regression tests now interact with the real contenteditable editor rather than a textarea or mode toggle. The document-switch regression verifies dirty text saves under the original project before the next note loads.

## Running browser checks

Used manually entered QA content in the existing browser fallback, not the owner's vault. Confirmed Markdown paste renders immediately; direct typing updates formatted paragraphs; checkboxes update on the same page; save status settles; Page actions opens Sources & insights; Spatial canvas opens from the sidebar; theme switching uses the sidebar; and the note header omits Canvas, Margin and theme controls. The test browser's viewport override was reset.

| Screenshot | State |
|---|---|
| [unified-light-1150.png](note-editor/unified-light-1150.png) | Light note at 1150 × 864, with live formatted content, task checkboxes, editable table and decision action. |
| [unified-light-1672.png](note-editor/unified-light-1672.png) | Wide desktop at 1672 × 941. |
| [unified-dark-1366.png](note-editor/unified-dark-1366.png) | Dark laptop layout at 1366 × 768. |
| [unified-light-900.png](note-editor/unified-light-900.png) | 900 × 768: header and actions wrap; content and table fit the available document width. |

DOM measurements confirmed no horizontal document overflow at 1366 or 900px, and task items use aligned flex rows. Native Tauri GUI, OS scaling, cross-platform interaction, IME composition and live provider inference were not exercised. Desktop persistence and derivation semantics were covered by Rust integration tests against temporary vaults.
