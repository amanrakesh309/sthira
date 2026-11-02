# Home visual correction — Phase 1 delivery

Implemented on 2 November 2026 in `/Volumes/Aman Ext/Projects/sthira`. This is the verified checkout corresponding to the owner's Linux path hint. Scope ends at the shared shell and Home; Focus, All Files, Graph, Explore & Plan and Settings have not been redesigned.

## Implemented

- Resizable/collapsible sidebar defaults to 264px, retaining saved user widths, the 200–700px persistence contract, pinned/recent caps, tree expansion, creation, renaming and existing page/project menus. Navigation reflects the actual screen, including collapsed navigation. Added keyboard resizing and row activation, larger hit areas, group dividers, readable labels and consistent vector navigation icons.
- Home uses a 56px toolbar and a centered, responsive 1200px content column. Removed duplicate global search/settings controls and the oversized SVG wave. Global search and Cmd/Ctrl+K remain in the shell. Model settings, account, themes and every creation flow remain accessible.
- Editorial hero (44px, scaling down in narrower content), 24px section headings, horizontal project title/icon groups, real descriptions/counts, 144px project cards and 64px continuation rows. Three-column cards become two/one columns as space reduces. Long titles truncate with full accessible names/tooltips.
- Universal input is one card: editorial entry line, submission control, vector mode buttons, quieter web search, bounded model selector and capability details in its popover. Live Stream remains available in Research/Plan. Tab cycles modes, Escape exits into controls, arrows move between modes, Enter submits; IME composition does not submit prematurely. Busy state prevents duplicate submission.
- Empty catalogs say “Select a model”; no invented model identity. Loading, failed workspace loading/retry and input error notices are distinct. Footer says “Local vault”, without claiming a sync result that the UI cannot verify.
- Recent captures remain available in continuation; actual destinations are deduplicated. Continue consistently opens documents, canvases or project-only rows. Sidebar Home refreshes state so newly created projects appear immediately.
- Existing semantic theme tokens are reused; no palette or dependency was added. Tokens still load exactly once after global/Tailwind CSS. New composition CSS loads last and is scoped to shell/Home. Rust, IPC definitions, provider adapters, database and durable user vault are unchanged.

## Screenshot comparison and correction loop

Approved target: `../references/home.png`, 1672×941. Its native titlebar is illustrative; it was not recreated in CSS. The large baseline raster is 8px narrower than the requested viewport; the 1366×768 before/after pair has identical raster dimensions. Screenshots were retained as captured, without padding or image manipulation.

| Evidence | Size / content |
|---|---|
| `home-before-1672.jpg` | Original running browser preview, requested 1672×941 viewport; captured raster 1664×941, empty workspace |
| `home-before-1366.jpg` | Original running browser preview, 1366×768 |
| `home-pass1-1672.jpg` | First implementation comparison, before final sidebar refinements |
| `home-after-1672.jpg` | Final browser Home at the reference viewport, same empty state |
| `home-after-1366.jpg` | Final browser Home, 1366×768 |
| `home-after-dark-1366.jpg` | Dark-mode browser check, 1366×768 |
| `home-after-900.jpg` | Narrow-window check, 900×768 |
| `home-after-1600.jpg`, `home-after-1920.jpg` | Wide-layout comparison passes, 1600×900 and 1920×1080 |
| `home-native-populated.jpg` | Final native Tauri window, 1200×850 including native chrome; real projects/notes created through UI in temporary QA vault |
| `home-native-dark.jpg` | Same populated native Home in dark mode |

Browser previews use the existing `ipc.ts` standalone fallback. Their model names do **not** demonstrate provider connectivity. Native checks used `STHIRA_VAULT_DIR=/private/tmp/sthira-visual-qa-vault`; projects explicitly named Visual QA were created through actual Tauri IPC, not injected into production rendering. No owner vault or credentials were copied. A temporary macOS app bundle enabled native UI automation; neither it nor its vault is part of this patch.

Measured at 1672px: sidebar 280→264px; content 976→1200px; content left edge x488→x368; hero baseline region starts at y114.5; input y230.1 and 138px tall. The reference's much wider illustrated sidebar is intentionally scaled to the specification's 264px default. At 1366px, final input remains 138px tall and the continuation heading starts at y617, while baseline continuation empty content fell below the 768px fold.

First visual pass exposed unnecessary two-row controls at 1366px, a wrapped empty-state button icon, excessive sidebar row height/reserved hover actions, and a crowded Custom Objects heading. Corrected the wrapping threshold, button icon placement, sidebar padding/action disclosure and header actions; then recaptured. Primary controls and sidebar icon controls have 40px minimum targets. Sidebar separator is a narrow drag target and also keyboard-operable.

Observed no document horizontal overflow at 900, 1093, 1366, 1600, 1672 or 1920px. Modes/options stayed within the viewport; model popover at 900px occupied x409–849, y398.6–718.6 within the window. Sidebar resize/collapse and Graph/Home active-state transitions were exercised. A 1093×614 logical viewport checks space comparable to 1366×768 at 125%; actual OS scaling/rasterization was not tested. Vertical scrolling is intentional at small heights.

## Validation

| Check | Result |
|---|---|
| Baseline `npm test` | PASS — 18 files, 85 tests |
| Baseline `npm run build` | PASS — existing large-chunk warning |
| Final `npm test` | PASS — 19 files, 92 tests |
| Final `npm run build` (includes TypeScript) | PASS — existing >500kB chunk warning; main JS ~889kB minified |
| `cargo test --workspace` | PASS — 119 tests; no Rust source changes |
| `cargo run -p sthira-desktop` with isolated vault | PASS — built and launched; native window inspected through temporary app bundle |
| `git diff --check` | PASS |
| Lint | Not configured in package.json |

Existing suites cover Capture/Research/Plan/Act, center streaming, provider capability selection, web search, project/page actions, resume/Focus, themes Light/Dark/System, autosave, objects and deletion safety. Seven added tests cover keyboard project/project-only continuation, recent loose capture/destination deduplication, canvas navigation, loading/retry, New menu flows/Escape, input mode cycling/keyboard exit and no-model configuration access. Updated three existing assertions to match the intentionally consolidated toolbar, semantic model selector and 264px CSS-variable width contract.

Native smoke checks verified project creation→Focus, grounded resume brief, capture→saved document, sidebar Home refresh, Continue→same saved content, long project names and light/dark themes. Live provider inference and external web search were not run; those paths are covered by existing mocked frontend/IPC tests, not an assertion of provider connectivity. Initial dependency downloads/local server launch needed sandbox escalation; both succeeded. Native automation initially timed out, then recovered and completed.

## Remaining visual differences and next phase

1. No licensed landscape/card-wash assets or matching bundled editorial font exist. Header stays quietly empty; system UI sans and Georgia preserve offline startup. The Sprout mark approximates the reference's leaf motif using existing Lucide assets.
2. The actual backend supplies only its most recent document and most recent canvas; Home can show up to three supplied destinations but does not invent a third row or suggested next step. Subtitles remain factual backend metadata; richer grounded continuation copy would need a separate data/API task.
3. At narrower native widths, web/model controls wrap into a second row. Long project titles truncate. This preserves usable controls and the sidebar width contract rather than forcing the full-screen mockup onto a smaller window.
4. The owner's Linux native chrome, fonts, actual vault and 125% OS scale remain unverified. Evidence here covers Chromium preview and the macOS Tauri WebKit window.
5. Existing production bundle size warning remains. Other screens retain their prior presentation and can still contain hardcoded styles; their components were preserved in this phase.

Recommended next design area: Focus/document workspace, including resume brief density, editor toolbar, history and collapsed epistemic margin, against `document-workspace.png`.

## Exact code/test file manifest

All paths are relative to `/Volumes/Aman Ext/Projects/sthira/`:

- `crates/sthira-desktop/ui/src/App.tsx`
- `crates/sthira-desktop/ui/src/components/HomeView.tsx`
- `crates/sthira-desktop/ui/src/components/LeftSidebar.tsx`
- `crates/sthira-desktop/ui/src/components/UniversalInputBar.tsx`
- `crates/sthira-desktop/ui/src/styles/home-shell.css` (new)
- `crates/sthira-desktop/ui/src/index.css`
- `crates/sthira-desktop/ui/src/main.tsx`
- `crates/sthira-desktop/ui/src/test/home_navigation.test.tsx` (new)
- `crates/sthira-desktop/ui/src/test/model_selector_capabilities.test.tsx`
- `crates/sthira-desktop/ui/src/test/settings_autosave_realtime.test.tsx`
- `crates/sthira-desktop/ui/src/test/workspace_organization.test.tsx`

Documentation/evidence: `docs/design/visual-audit/home-baseline.md`, this report, and the screenshots listed above.
