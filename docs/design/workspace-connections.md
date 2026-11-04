# Workspace connections

Implemented 5 November 2026. The feature extends the existing Warm Stone / Forest Sage document workspace.

## Using connections

1. Open a page or typed object and choose **Connections** in the top bar.
2. Choose **Link an item** and search by title or object type. Pages, projects, typed objects, sources and snippets can be linked.
3. Choose an item to create a reference card. **Insert in page** also inserts a clickable reference at the selection in the unified rich editor.
4. Open a reference card or an inline reference to read it in the right panel. The original editor stays mounted, retaining its draft and scroll position.
5. Follow more references inside the panel. **Back**, the reference trail, or Escape retraces the path. At the root, Escape closes Connections. **Open in workspace** opens the item in the main workspace after saving any dirty original document.

When a page belongs to a project, **This page / Project** selects which item owns the connection. A project can refer to another project, page or object without attaching or moving its contents. Project previews show their purpose and current pages/objects; typed-object previews include saved properties.

**Links from here** lists outgoing references. **Linked from** lists backlinks, using the same stored relationship. Removing a connection offers Undo and preserves both items and their text, including any inline links. Inline labels retain their written text when a target is renamed; the stable target ID keeps navigation working, and reference cards resolve the current title.

The rail is visible by default on wide desktop windows, and can be toggled independently of Sources & insights (available through Page actions). At narrower workspace widths it becomes a drawer. Both themes use the existing semantic tokens. Previews are for reading; use Open in workspace to edit the referenced item. The original note is always editable, without separate Edit / Preview modes.

Derived notes automatically expose their parent and backlinks through saved lineage, including notes created before the Connections feature. Those relationships appear as **Derived from / Derived item** and remain part of provenance rather than removable manual connections. See the [unified editor validation](visual-audit/note-editor-validation.md).

## Storage and boundaries

- SQLite migration **12** adds `workspace_links`, with separate artifact/project foreign keys at each endpoint. It preserves existing notes, projects, properties and memberships. Endpoints are validated inside a transaction; duplicate requests are idempotent, and self-links are rejected.
- Pages, sources, snippets and typed objects use their existing artifact identity. The relationship is an explicit reference, separate from project membership, derivation lineage, spatial placement and AI proposals.
- References survive application restart and appear as `references` edges in the Knowledge Graph. Deletion cleans incident references; detaching a shared page from a project preserves its references. Soft-deleted items are excluded from cards, search and reference graph edges.
- Browser preview uses only items created in that browser session. Desktop persistence is SQLite; browser preview references are temporary.
- Manual links are created explicitly through the picker; derivation lineage is included automatically. Plain text names, pasted Markdown URLs and existing AI semantic links are not automatically indexed into this rail. Canvases are not reference targets in this version. These boundaries avoid guessing relationships from text or changing existing semantic contracts.

## Implementation owners

- `crates/sthira-core/src/db/workspace_links.sql` and `migrations.rs`: additive schema migration.
- `crates/sthira-desktop/src/connections.rs`: entity resolution, target search, read-only previews and connection commands.
- `crates/sthira-desktop/src/commands.rs`: deletion cleanup and graph integration; `lib.rs`: IPC registration.
- `crates/sthira-desktop/ui/src/components/ConnectionsPanel.tsx`: reference rail, picker, backlinks, nested navigation and Undo.
- `crates/sthira-desktop/ui/src/components/FocusView.tsx`: source selection, insertion and draft-preserving integration; `App.tsx`: project navigation.
- `crates/sthira-desktop/ui/src/components/RichDocumentRenderer.tsx`: local internal-link activation while retaining normal external links.
- `crates/sthira-desktop/ui/src/api/connections.ts`, `types/sthira.ts`, `styles/connections.css`: typed bridge and responsive theme styling.

Validation and screenshots: [connections-validation.md](visual-audit/connections-validation.md).
