# Project overview

Opening a project from Home, the sidebar, search, All Files, or the graph opens its overview in the center workspace. It does not select the first note or request an AI resume brief. An explicit document selection opens the editor; selecting a canvas opens that saved canvas; selecting a related project opens its overview.

The overview includes the saved project purpose, status, member and connected-item counts, creation date, and last substantive work date. Its contents combine:

- Active attached pages, sources, snippets, and typed objects.
- Saved canvases belonging to the project.
- Pages, objects, and projects with direct outgoing references or incoming backlinks to the project.

An entity appears once, with all applicable relationship labels. Membership and references remain distinct: following or creating a reference does not attach or copy its target. Artifacts only placed on a canvas are not automatically project members or semantic connections.

Search covers names, types, and loaded summaries. Type and relationship filters combine with edited/created/name sorting. List and grid layouts share pagination. Opening a document or canvas and using its Project back button restores the originating overview and its current search, filters, sort, layout, and page. A referenced document keeps its own project context for editing.

New page explicitly creates and opens a project member. Add existing attaches a saved artifact using the existing many-to-many association, keeping a single content and revision history. Connections provides in-place previews, link/unlink, and Undo; changes refresh the overview immediately. Member pages retain their existing page action menus.

The layout follows the Warm Stone + Forest Sage system. The sidebar project overflow menu retains New page, Pin, Duplicate, Rename, and Delete. Reserving its action space prevents controls from shifting the project label on hover.

See the permanent [design policy](../policies/design.md) and [verification policy](../policies/verification.md) for alignment rules and required checks. Current unverified work is tracked in [known limitations](../known-limitations.md).
