# Design and interaction policy

## Visual authority

Sthira should feel quiet, assured, precise, warm, legible, spacious, and trustworthy. Use the [design system](../design/sthira-design-system-v1.md), the active semantic tokens in `crates/sthira-desktop/ui/src/styles/sthira-theme-tokens.css`, and the [approved references](../design/README.md). Current product decisions override obsolete screenshot controls and example content.

Warm Stone + Forest Sage defines light mode; Deep Obsidian Stone defines dark mode. Use semantic tokens rather than scattered hardcoded values. Every referenced token must resolve in the active theme; an undefined CSS variable may silently remove a color without failing the build. The active UI token file is the implementation source; the design-folder CSS is a reference, not a second runtime import.

## Composition and hierarchy

- Sidebar about 264px by default, with working resize/collapse and one coherent active location. Its capped/pinned lists lead to complete directories. Hover actions reserve space and do not move click targets.
- Use a quiet header around 56px and an intentional page container. Home typically uses 1120–1240px maximum width; prose targets 720–820px or roughly 65–80 characters. Other surfaces choose widths suited to actual tables/graphs, not a universal oversized hero.
- Prioritize useful content: Home's capture and concise continuation; project's brief and contents; editor's document; directory's scannable rows; graph's topology; Settings' intent and truthful setup state.
- Home composer: keep a spacious multiline input above a compact, grouped mode selector. Switching modes must not add a green outline around the composer or textarea. Keep its normal border with a neutral focus cue and preserve keyboard focus indicators on the individual controls. Web search and the bounded model selector are quiet secondary controls; show full model details and optional live preview on demand in the model popover. Wrap the secondary group deliberately on constrained windows.
- Resume is an explicit project/document action, never an automatic substitute for project overview or document loading. Show factual/model/cache state and the producing model, source links, compact/expanded views, explicit summary refresh, and current facts on failure. Resume configuration lives within Workflows, with source-detail controls and a collapsed optional local-pack download. Do not describe excerpts as newly verified conclusions or hide saved facts behind model setup.
- A quiet tab strip appears when multiple documents are open, with bounded title labels, active state, close controls, keyboard navigation and horizontal scrolling. Keep one editor central. The title area offers Suggest a title and Open another document as secondary actions; suggestions show their model, reviewable choices and explicit Use title controls. Resume shows the resolved model and connection before generation as well as attribution afterward.
- Document conversation uses a quiet header toggle and one collapsible auxiliary rail. Keep the document central; share space with Connections/evidence rather than opening competing rails. At constrained widths, use a dismissible drawer. Place a quiet model dropdown in the conversation header context, and show the producing model/provider with each reply. Keep model capability badges and web-search claims out of this selector. Suggested edits open outside the rail in a wide rich-text comparison with explicit Apply/Dismiss controls. Render headings, lists, tasks, tables, links, and code; show read-only documents rather than raw Markdown source. Stack the documents on small windows and keep review actions visible.
- One dominant action per region. Group secondary/formatting/AI/destructive actions meaningfully. Detail opens on demand, with a clear return path.
- Settings use a single category sidebar (horizontal navigation at constrained widths). Reveal workflow model and template controls after enabling the workflow. Capture hides AI model/search controls; Research, Plan and Draft expose the controls relevant to their intent. Disabled generation keeps the user's input and offers a direct Settings action.
- AI & Models keeps saved connection fields collapsed and uses calm model rows with default selection and a labeled information button. Do not show task suggestion badges. Technical metadata, context-capacity guidance, capability overrides, and enable/remove actions appear on demand. Avoid repeated context, web-search, and enabled badges. Show model search only for larger catalogs. Guided app setup offers explicit Ollama, LM Studio and custom choices, concise install/start instructions, optional authentication, and distinct connection/model-discovery outcomes.
- Use the 4/8/12/16/24/32/48px spacing rhythm, subtle borders, and restrained elevation. Typical radii: inputs 8px, menus 12px, cards 16px, prominent Home input 20px. Document a deliberate exception rather than stacking arbitrary overrides.

- Use Pages as the shared document navigation label. Keep generated plans and research in the same rich-text typography as other page content. Generative UI cards and widget/raw-mode toggles are deferred; legacy content must remain readable and selectable.

## Text, labels, and controls

- Serif is for editorial headings and appropriate prose, not dense controls or metadata. UI text uses the established local/system sans-serif stack. Scale headings down at constrained widths and zoom. Essential labels remain at least 12px and readable in both themes.
- Labels belong to their fields in the same layout column and stay attached when wrapping. Use a common label baseline, label-to-field gap, field height, and text inset. Align table headers/cells, dialog titles/body/footer, metadata, badges, and actions intentionally.
- **Project filters:** Type, Relationship, and Sort by align to the selected-value text anchor, including the field's border and padding. With a 1px border and 12px left padding, that anchor is 13px. Recalculate if geometry changes; do not preserve a magic number independent of the control. Extra glyphs must not shift just one label's text anchor.
- Standard controls are roughly 40–44px high with usable hit areas; important targets aim for 44×44px. Search/value text, dropdown arrows, and icons have compatible baselines and padding.
- Search/filter/sort/list-grid/pagination group coherently across widths. Do not force unrelated text to center, hide useful labels, use negative-margin patches, or absolutely position labels to compensate for broken layout.
- Keep long titles/URLs, Unicode, empty values, wide tables/code, and changing statuses usable. Optional columns collapse before primary titles become unreadable. Intentional internal scroll must not create whole-page horizontal overflow.

## Graphics and icons

- Preserve the established mark and icon system. Match icon family, scale, stroke, visual weight, optical center, and text gap. Inspect SVG viewBox whitespace separately from its outer hit box.
- Artwork keeps its aspect ratio and intended bounds. Optional low-contrast header wash belongs in spare space, never behind prose, citations, tables, or diagrams. Offline startup must not depend on remote images/fonts.
- Graph nodes, labels, arrows, legend, minimap, selection, and controls align with actual geometry and remain legible while zooming. Never add decorative knowledge edges or alter fixture data to imitate a mockup.
- Canvas frames/cards/labels/handles and saved layout agree spatially. Graphics/widgets fit their containers, preserve real data, and provide graceful fallback. Visual arrangement must not alter semantic relationships.
- Avoid new ornamental motifs, oversized decorative icons, excessive shadows, and unnecessary illustrations that distract from knowledge work.

## States, accessibility, and tone

- Every surface includes truthful loading, empty, no-results, pending/saving, success, disabled/unconfigured, error/retry, and offline states where applicable. Failure must not resemble success or silently erase input.
- Keyboard users can capture, browse/filter, open, edit, save, connect, preview/back, and return. Visible focus and platform-appropriate shortcuts remain consistent. Drag/graph operations need usable alternative access where practical.
- Dialogs have proper semantics, initial focus, Tab containment, predictable Escape/backdrop behavior, and focus restoration. Search results support the declared arrow/activation model; focusable results must also activate from the keyboard. Async searches ignore stale responses after query/close changes.
- Check actual element/state contrast: 4.5:1 normal text, 3:1 large text and meaningful control boundaries. Status also uses words/icons. Decorative surface separation does not automatically require 3:1; borders/token tables alone do not prove accessibility compliance.
- Respect reduced motion; keep restrained transitions. Verify 125% scaling and long text without clipped controls or inaccessible off-screen actions.
- Copy is concise, factual, neutral, and helpful. No guilt, invented metrics, celebratory noise, engineering branding in ordinary flows, or unsupported certainty. Error reassurance must match actual draft/recovery behavior.

## Page coverage

Review the shell, Home, project overview, Projects/All Files/Notes, unified editor, Connections, Sources & insights/history, object properties/collections/boards/type/template manager, graph, canvas, all Explore & Plan tabs, Settings, global search, and every overlay touched by shared changes.

Apply all six axes to each affected surface: composition; alignment; typography/tone; graphics; tokens/components; states/accessibility. Reference images exist for Home, document workspace, All Files, Knowledge Graph, and Settings. Project overview/Connections follow their documented contracts and the shared system. Canvas and Explore & Plan have no separate approved image; inspect their actual UI against this policy rather than substituting another screen's screenshot.

Visual changes require the fresh rendered comparison and behavior checks in [verification policy](verification.md). A CSS-class assertion is not proof of alignment.
