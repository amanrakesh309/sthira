# Knowledge Graph Baseline Visual Audit

**Reference Image**: `docs/design/references/knowledge-graph.png` (1920×1080 / 1672×941)  
**Baseline Screenshots**:
- `docs/design/visual-audit/graph-before-1672.jpg` (1672×941)
- `docs/design/visual-audit/graph-before-1366.jpg` (1366×768)

## Overview & Scope

The Knowledge Graph view (`GraphView.tsx`) visualizes connections between projects, notes, and custom objects. The approved reference displays an expansive, warm, editorial workspace featuring distinct project hub halos, typed note and object cards, contextual edge curves, a prominent legend card, floating viewport zoom controls, and a minimap radar preview.

## Ranked Discrepancies and Gap Analysis

| Priority | Dimension | Approved Reference (`knowledge-graph.png`) | Existing Implementation (`GraphView.tsx`) | Defect Impact & Action Required |
|:---:|---|---|---|---|
| **1** | **Header Composition & Typography** | Generous header with `EXPLORE YOUR KNOWLEDGE` eyebrow (11px tracked uppercase), Fraunces editorial serif title `Knowledge Graph` (~30px bold warm black), inline stats pill (`⠸ 11 nodes · 3 edges`), and muted subtitle (`A connected view of your projects, pages, objects and ideas.`). | Cramped single-line white bar with generic `Home / [icon] Knowledge Graph Map` title, small sans font, no eyebrow, and no subtitle. | High: Lacks editorial authority and emotional warmth. Restructure header into open editorial format. |
| **2** | **Filter & Search Toolbar** | Dedicated horizontal toolbar below the header with `FILTERS:` label, `SlidersHorizontal` icon, pill buttons for `📁 Projects ✓` (dark green active state), `📄 Pages`, `💡 Ideas`, `☑ Tasks`, `🔗 Sources`, and dynamic object types. Right side features rounded search pill (`🔍 Search nodes... ⌘F`), fullscreen/fit-to-view button `[ ]`, and overflow button `···`. | Filters and search are compressed directly inside the single top header bar, crowding controls and lacking semantic pill styles. | High: Separate filter toolbar into its own dedicated surface with consistent pill styling and search shortcut badge. |
| **3** | **Canvas Node Rendering (Hubs & Pills)** | **Project Hubs**: Circular forest sage hubs (`#49583D`) with multi-layer concentric glowing rings/halos, gold folder icon, bold serif project title and item count pill below.<br/>**Notes & Objects**: Horizontal rounded card pills (`#FFFFFF`, subtle border, shadow) with icon container (blue document or purple scales), legible title, and draft status indicator dot. | Plain geometric circles drawn directly on canvas without card shapes or concentric glow halos. Note labels are placed in small detached text boxes underneath circles. | Critical: Redesign canvas rendering loop to render concentric glow halos for project hubs, and rounded card pills for pages and custom objects. |
| **4** | **Edge Styling & Hierarchy** | Graceful, restrained organic bezier curves with warm neutral tint (`#C8C2B3`), distinct from text and pills. Smooth connections to hub perimeters and card pill edges. | Stiff straight lines with harsh border colors and prominent text badges on lines that clutter the canvas. | High: Implement organic curved bezier paths with warm beige/taupe strokes and hover highlight states. |
| **5** | **Legend Overlay** | Structured floating card in bottom-left corner with `Legend` header and info `ⓘ` icon, listing `Project` (workspace for focused work), `Page / Note` (knowledge, research, and content), and `Object` (structured data, decisions, custom entities). | Simple horizontal text strip (`● Project ○ Note ● Object ― Link`) lacking descriptive context and card structure. | Medium: Rebuild as structured floating legend card matching reference card hierarchy. |
| **6** | **Viewport Controls & Minimap** | Floating pill toolbar in bottom-right with `−`, `100%`, `+`, `⛶` buttons, paired with an adjacent radar minimap preview showing canvas bounding frame and node dots. | Small inline zoom buttons placed in top header bar; no minimap exists. | High: Move zoom controls to floating bottom-right pill toolbar and implement canvas minimap radar preview. |
| **7** | **Active Navigation Indication** | Sidebar active destination must be `Knowledge Graph` with `--sthira-primary` active indicator. *(Note: Reference mockup mistakenly displayed Home active; the specification explicitly mandates highlighting Knowledge Graph).* | Sidebar currently highlights Knowledge Graph correctly in `currentView === 'graph'`. | Preserve: Ensure sidebar active indicator remains strictly on Knowledge Graph. |

## Baseline Verification Checks
- `npm test`: 134 tests passed (25 files).
- `npm run build`: built in 1.10s with zero errors.
- `cargo test --workspace --offline`: 119 tests passed.
- `git diff --check`: clean whitespace.
