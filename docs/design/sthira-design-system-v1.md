# Sthira — Visual Design System & Redesign Blueprint (v1)

**Status:** Detailed design reference. The permanent [design policy](../policies/design.md) and [product contract](../policies/product.md) govern current decisions. This document is not a verification report.
**Date:** 2 November 2026  
**Inputs:** `sthira_product_understanding.md` (v4), `plan(1).md`, the current screenshots, and the generated Sthira concept mockups.

## 1. North star

**Sthira should feel like a calm, dependable studio for thinking, not an AI control panel.**

Design adjectives: **quiet, assured, precise, warm, legible, spacious, trustworthy**. Make everyday actions obvious to non-technical users while retaining power-user controls through progressive disclosure.

Existing functionality is the contract: the redesign must preserve the universal input (Capture / Research / Plan / Act), source grounding and authorship, reversible AI actions, resume Levels 1/2, history, canvas, all-files filters, typed objects, providers, web research, themes, and keyboard workflows. A rendered design mockup is **not proof of a feature, relationship, or claim**.

### Critical constraints from the current product documentation

- **Resume-first:** concise grounded Level 1 brief (up to 75 visible words); expanded Level 2 review on demand.
- **Knowledge is durable:** show evidence paths and ownership/AI-provenance distinctions; never invent progress, history, sources, or graph edges.
- **Context first:** project navigation opens the overview; selecting a document opens the editor. Optional evidence contextualizes work without dominating it.
- **Local-first:** do not require online fonts, image assets, cloud search, or model access for the app shell to render.
- **Calm AI:** no persistent popups, competing CTAs, or automatically accepted structural changes.
- **UI architecture:** existing Tauri v2 + React 19 + TypeScript + Tailwind CSS + Lucide icons. Keep Rust APIs and storage behavior unchanged during the visual pass.

## 2. Color system — Warm Stone + Forest Sage

Use **semantic tokens** from `sthira-theme-tokens.css` rather than arbitrary colors inside components. The values below are implementation suggestions informed by the current light/dark theme defaults and the mockups.

### Light mode

| Role | Hex | Usage |
|---|---|---|
| App background | `#F5F3EC` | Outside panels, app shell |
| Main workspace | `#FAF8F3` | Main canvas, knowledge views |
| Surface | `#FFFFFF` | Input, cards, editor sheet, settings panels |
| Warm raised surface | `#FDFBF7` | Hover-neutral or quieter panels |
| Secondary surface | `#F1EEE6` | Navigation hover, segmented background |
| Primary text | `#1E201C` | Body text, titles |
| Muted text | `#62665D` | Descriptions, timestamps, metadata |
| Subtle text | `#73766D` | Tertiary supporting text only |
| Border | `#E8E5DC` | Inputs, tables, dividers |
| Strong border | `#D8D4C8` | Hover/focus-adjacent panel edges |
| Primary green | `#49583D` | Primary CTA, selected navigation |
| Green hover | `#39472F` | Primary hover |
| Primary tint | `#EEF1E8` | Selected rows, quiet affirmations |
| Gold accent | `#BE8B41` | Small editorial accents, noncritical decoration |
| Warm warning background | `#FFF7E8` | Research fallback notices |
| Warm warning text | `#7E581D` | Warning text |
| Danger text | `#9D3F36` | Destructive actions or error text |
| Focus ring | `#6E835C` | Keyboard-visible focus indication |

**Color hierarchy:** 80–85% warm neutrals, 10–15% sage, 2–5% amber. Green conveys selection and clear primary action; amber highlights subtle editorial details or cautions. Avoid coloring every icon, filter, and title.

### Dark mode — Deep Obsidian Stone

| Role | Hex |
|---|---|
| App background | `#121310` |
| Main workspace | `#161714` |
| Surface | `#1C1D19` |
| Raised surface | `#23251F` |
| Hover/secondary | `#292B25` |
| Primary text | `#EDEAE2` |
| Muted text | `#B1B5AB` |
| Border | `#2C2E27` |
| Primary action | `#879E78` |
| On-primary text | `#121710` |
| Selection tint | `#283427` |
| Focus ring | `#A1BE8B` |

Dark mode should be deliberately tuned, **not** a CSS-inverted light mode. Maintain source, inference, and status distinctions without relying on color alone.

## 3. Typography

- **UI / controls / tables:** `Inter`, then system sans-serif fallback (`-apple-system, BlinkMacSystemFont, Segoe UI, sans-serif`). Use an installed or bundled *licensed* webfont only if already available; otherwise system fonts work offline.
- **Editorial headings (optional):** `Fraunces` or `Source Serif 4`, then Georgia/serif fallback. Use sparingly for Home hero, section headings, and page titles; not in form labels, graph metadata, chips, or dense tables.
- **Document prose:** comfortable reading line length **65–80 characters** (approximately `720–820px`), 16px / 1.7 line-height; long-form text may offer a serif preference later.
- **Type scale:** hero 40–44px/1.12; page title 30–34px/1.2; section 22–24px/1.25; card title 16–17px/1.4; body 14px/1.55; navigation 13–14px; metadata 12–13px. Don't go below 12px for essential labels.
- **Weights:** body 400; controls 500; navigation active/title 600; limited 700. Avoid bolding every row.
- **Case:** sentence case for interface actions; uppercase small labels only sparingly, with added tracking.

**Important:** The generated mockups use very large serif titles; these work for visual exploration but must scale down in production at 1366×768 and at increased font sizes.

## 4. Geometry & spacing

- Base spacing: **4px**, with practical rhythm of **8 / 12 / 16 / 24 / 32 / 48px**.
- Sidebar: **264px default**; user-resizable ~232–320px. Support collapse.
- Main content: centered with an approximately **1120–1240px** maximum for Home; the editor prose column **720–820px** for readability.
- Header height: ~**56px**. Don't make both titlebar and content toolbar oversized.
- Compact nav row: **36–40px**; normal input: **40–44px**; large universal input: **100–140px** (responsive).
- Card padding: **20–24px**; list row height: **56–68px**, depending on secondary descriptions.
- Radii: **8px** for input/chips; **12px** for menus/rows; **16px** for cards; **20px** for prominent Home input. Avoid arbitrary new radii.
- Elevation: most surfaces use borders, not shadows. Only floating popovers/modals and selected hero surfaces get a soft shadow.
- Illustration: subtle optional textured / mountain wash only in generous **empty header** areas, with low contrast. Never place art behind body copy, source citations, diagrams, or data tables. Treat this as a visual asset, not a prerequisite for using Sthira offline.

## 5. Components — consistent states and semantics

### Buttons

- **Primary:** sage filled, high-contrast label. One dominant action per region.
- **Secondary:** white/raised with border.
- **Tertiary/ghost:** plain text and icon; clear hover and keyboard focus.
- **Destructive:** separate red-toned treatment with confirmation where warranted.
- Minimum interactive hit area: **40×40px**, aim for **44×44px** in key workflows.
- Every action has loading, disabled, error and focus-visible states; disabled affordance should not look identical to unconfigured or disconnected.

### Navigation

- Show **one active destination**, not Home selected while displaying Graph or All Files (a defect in a generated mockup).
- Clarify hierarchy: Home / Explore & Plan / Knowledge Graph / All Files, then Projects, Notes & Pages, Custom Objects.
- Sidebar is navigation, **not a second All Files directory**: show pinned/recent items and a small capped list, with View all links.
- Prefer **one global search action** (visible in sidebar or header plus Cmd/Ctrl+K), and page-specific local filtering where relevant. Do not duplicate two identical global searches.

### Chips and badges

- Distinguish **active filters** (interactive), **labels/types** (informational), **statuses** (Connected, Local, Draft), and **provenance** (Recorded, Source-backed, AI suggestion). They must not all look like clickable buttons.
- Avoid decorative green “success” badges on ordinary rows.
- Explain model search using user-friendly labels: **Built-in search**, **Search via tools**, **Web context provided**, **Offline only**. Technical capability classification remains available in detail.

### Inputs, menus, and overlays

- Labels persist for configuration inputs; placeholder is not the only label.
- Menus have keyboard navigation, proper focus trapping for modals, Escape close, and visible validation.
- Endpoint and API-key fields are in **Advanced** provider details; normal users see connection status and model choice first.
- If settings require many fields (as current screenshots show), prefer a **full settings page or large responsive dialog with a left section navigation**, not four squeezed top tabs and double-scroll containers.
- Clearly distinguish Saved / Saving / Failed from model “Connected” and vault “Synced”; these are separate states.

## 6. Screen-by-screen redesign decisions

### 6.1 Home — highest priority

**What works already:** Home hero, universal input, project cards, pick-up list.

**Changes:**
1. Reduce competition between model, web search, mode controls and input. The question/capture area is the focus; advanced model and search settings sit in a quieter second row.
2. Keep Home to **max 3 project cards and max 3 meaningful continuation rows**, consistent with the product understanding.
3. Show a **short continuation sentence** grounded in real work; never invent a proposed next step as a fact. If no continuation exists, use an honest placeholder.
4. “Continue” is the main action. Avoid adding a third invented project or a suggested next step simply to fill space.
5. Add generous vertical separation between hero, projects and pick-up list; remove unnecessary shaded containers.
6. Make captured thought / Research / Plan / Act states easily identifiable, with keyboard access.

### 6.2 Project & Focus document view — highest priority

**Current contract:** Project overview and explicit document selection, one rich Markdown-backed editor, Refine/Derive, History, Connections, and save state. Canvas is in sidebar navigation; evidence opens through Page actions → Sources & insights.

**Changes:**
1. Keep the document as the primary workspace. Resume Quick Brief is **collapsible and up to 75 words**, with one “Expanded review” action.
2. Resume does not require the user to read a large card before editing. On active frequent return it may stay collapsed; on cold return follow product hint behavior.
3. Limit document measure to 720–820px. Use one formatted editable document with Markdown storage; do not restore separate Edit/Preview modes. Reference previews remain read-only.
4. Separate formatting actions from AI actions visually; Refine and Derive should look secondary, with preview/undo before substantive mutations.
5. Sources & insights opens on demand and displays actual sourced claims, questions, and relevant artifacts. Connections has its own contextual rail/drawer behavior; neither may invent takeaways or task lists.
6. Protect long titles, wide tables, code blocks, lineage chips, and small-window layouts.

### 6.3 All Files

**What works:** unified directory, top categories, scope filters, type filters, sort and actions.

**Changes:**
1. Improve clear distinction between **category tabs** and **scope/type filters** (they should not all be indistinguishable pills).
2. Default to a dense but breathable list with consistent column alignment; preserve grid/card modes where appropriate.
3. Title and actual content type are distinct. Do not label a generic note as Source or Idea merely because a generated mockup did.
4. Hover actions can be subtle, but essential actions must remain discoverable for keyboard/touchpad users.
5. On narrow desktops, allow optional columns to collapse before truncating titles.

### 6.4 Knowledge Graph

**What works:** graph visualization, filters, zoom and node navigation.

**Changes:**
1. **Never draw decorative/fake links**. Structural attachments and semantic relationships must originate from actual persisted edges.
2. Clearly encode edge types, including semantic vs project membership. Provide accessible legend and selection detail panel.
3. Improve node labels/selection/hover, fit-to-view, graph empty states and zoom controls.
4. Do not make every node a shadowed card: graph should remain visually legible at scale.
5. Hide art/textures behind graph data; preserve performance with many nodes.

### 6.5 Model, Web Search, Appearance & Vault settings

**What works:** multiple providers, editable endpoints, model selection, provider testing, search mode setup, appearance, vault.

**Changes:**
1. Replace engineering label **“OmniRouter Compute Fabric”** in consumer-facing entry points with **“AI & Model Settings”** or **“Models & Providers.”** The internal engine identity can remain in About/Advanced.
2. Group settings by user intent: Models, Web Search, Appearance, Storage & Privacy, Advanced.
3. One model row = name, locality, relevant capabilities, current/default indication and row action menu. Low-level raw IDs/URLs are subordinate.
4. Provider card: clear state **Connected / Needs setup / Unavailable / Testing**; only claim connected when tested.
5. Separate selected model from default model, and web-search *provider* state from model *tool capability*.
6. Use one vertical settings scroll region; avoid independently scrolling stacked nested cards unless necessary.
7. Preserve dark/light/system themes and full keyboard operation.

### 6.6 Typed objects & Explore & Plan

- Make object type properties visually distinct from freeform Markdown body. Preserve template and type-management functionality.
- Research, Plan, Trackers, and Digests should have one consistent subnavigation system; treat research stage updates as content progress, not alert popups.
- Evidence and confidence/provenance should be visible through explicit words and source links, not green-check decoration alone.

## 7. Accessibility and platform details

- Aim for **WCAG AA**: 4.5:1 contrast for normal text, 3:1 for large text and meaningful control boundaries.
- Primary ink `#1E201C` on canvas `#FAF8F3` is approximately **15.5:1**; muted text `#62665D` on canvas ~**5.5:1**; white on primary green `#49583D` ~**7.7:1**.
- Focus-visible ring must be apparent on buttons, tabs, inputs, table rows, graph controls and menus.
- No essential information solely conveyed by color. Status must include icon+text or label.
- Respect reduced-motion preferences. Keep transitions ~120–180ms; avoid parallax and animated wallpaper.
- Verify at 1366×768, 1600×900, 1920×1080, and at 125% browser/system scaling; test normal and long text, empty data, errors, slow inference, and offline state.
- Support Linux/Windows key hints, not only `⌘`. Display `Ctrl+K` or platform-appropriate shortcut.

## 8. Implementation order (visual-only, low regression risk)

1. **Baseline:** inventory current shared UI components, screenshot each current screen, save before/after references, ensure current tests pass.
2. **Tokens:** add `sthira-theme-tokens.css`; replace repeated hard-coded colors with semantic tokens; verify existing light/dark/system theme switching.
3. **Primitive components:** Button, IconButton, Input, Select, Tabs/Segmented, Badge, Panel/Card, Dialog, Toolbar, TableRow, EmptyState, Tooltip/Toast. Align radius, padding, type, focus.
4. **App shell:** sidebar, global header, search hierarchy, navigation active states. No route/IPC changes.
5. **Home:** universal input, cards, resume-first list; real data only.
6. **Focus:** resume card, editor typography, actions, collapsed margin; preserve autosave, Markdown, refine/derive history.
7. **All Files and custom objects:** filters, tables/cards, metadata, menus.
8. **Knowledge Graph:** node treatments and legend, without changing edge semantics.
9. **Settings and Explore & Plan:** progressive disclosure, provider configuration, tracking and research states.
10. **Regression & accessibility:** screenshots across both themes/viewport sizes, interaction tests, build, app-launch smoke test.

**Definition of done per screen:** all existing functions work; no new static fake data; loading/empty/error/success states are designed; keyboard operation and focus are correct; layout scales; light and dark modes both pass; screenshots compared against the visual anchor.

## 9. Implementation and verification

Follow the permanent [policies](../policies/README.md). Inspect current callbacks, state, API/storage boundaries, and tests before changing a surface. Reuse semantic tokens and coherent shared components; preserve actual data and behavior. Verify affected surfaces with fresh rendered evidence and relevant functional tests. Keep generated output in ignored temporary storage rather than adding another execution prompt or audit album.

## 10. Known mockup limitations (don't reproduce blindly)

- Generated mockups sometimes show **an extra project**, fabricated research findings, invented nodes or links, or category badges unrelated to real data.
- Some show **Home active** on a different screen.
- Some show obsolete Edit/Preview modes and header controls. The current product uses one rich editable surface and the simplified header defined in policy.
- Decorative mountain textures and oversized serif headings look good at full-screen but need aggressive moderation in the real density and responsive states.
- The mockups do not cover all dark mode, loading, failure, privacy, provenance, keyboard or narrow-display conditions.

---

**Maintenance:** use current policies and actual implementation; earlier phase order does not imply that a surface is finished or verified.
