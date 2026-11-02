# Focus/document workspace — implemented phase 2

Reference: `docs/design/references/document-workspace.png` (1672×941). Baseline audit: [focus-baseline.md](focus-baseline.md). Work was performed in the existing frontend at `crates/sthira-desktop/ui`; the prior shell/Home changes remain in the working tree.

## Implemented composition

- Compact document header with actual project/document breadcrumbs and one global search entry while the full sidebar is visible.
- Continuous warm document sheet, 1120px maximum width; readable 800px prose measure; 34px editorial document title, 24px section headings, 16px/1.7 body text, short amber title rule.
- Flat formatting toolbar with grouped Edit/Preview, Refine/Derive and page actions, retaining existing handlers and keyboard shortcuts. A matching leading Markdown H1 is hidden only in Preview; the saved Markdown stays intact.
- Resume brief with clear purpose/findings columns, explicit provenance and source navigation. Compact visible copy is capped at 75 words; Level 2 exposes every returned finding, conflict and source. The backend's show/collapse hint is honored.
- A 320px evidence margin, closed by default. It becomes an overlay at smaller workspace widths. Source counts include distinct primary and Markdown URLs; sparse evidence gets an honest empty state.
- Theme-aware history drawer with narrative, real dates, provenance, evidence disclosures, artifact navigation, Escape dismissal and a keyboard focus loop including disclosures.
- Visible document-load/save recovery, protection for dirty drafts when changing projects, and synchronized title/content when following resume/history links.

The native check exposed an existing frontend/Rust history DTO mismatch. `getProjectHistory` now maps Rust's `segments`, attribution and timestamps into the existing frontend view model. This restores the narrative and correct recorded/source-backed/inferred labels without altering the Rust command or contract. Only substantive-edit evidence is treated as an artifact link; other record IDs remain evidence references.

## Screenshot comparison and second pass

All files below are unmodified captures, relative to this directory. Native checks use the isolated `/private/tmp/sthira-visual-qa-vault`, not the owner's vault. Browser screenshots use the application's existing browser fallback with a manually entered QA document, so they demonstrate layout rather than live backend/provider connectivity.

| Capture | Size | Evidence |
|---|---|---|
| [Before: preview and margin](focus-before-preview-margin-native.jpg) | 1920×1080 | Small prose, duplicated H1, boxed toolbar, decorative brief overlap and large unused strip beside the margin. |
| [Before: raw Edit](focus-before-edit-native.jpg) | 1920×1080 | Original textarea styling; its scroll position is below the top of the sheet. |
| [First pass: native preview](focus-pass1-preview-margin-native.jpg) | 1920×1080 | Rebuilt brief/title/toolbar and document/margin layout. |
| [First pass: reference-size preview](focus-pass1-preview-1672.jpg) | 1672×941 | Browser geometry comparison. |
| [Final: native preview and brief](focus-after-preview-margin-native.jpg) | 1920×1080 | Real saved document and actual resume brief; matches baseline window size. |
| [Final: native sources](focus-after-sources-native.jpg) | 1920×1080 | Actual saved Markdown URL, source count and navigation target. |
| [Final: native history](focus-after-history-native.jpg) | 1920×1080 | Real narrative, dates, recorded attribution; disclosure opened using Tab then Return. |
| [Final: reference-size preview](focus-after-preview-margin-1672.jpg) | 1672×941 | Clear browser render, active Preview, single H1, typography and docked sources margin. |
| [Final: compact desktop](focus-after-preview-margin-1366.jpg) | 1366×768 | Wrapped toolbar and readable document beside the margin. |
| [Final: dark](focus-after-dark-1366.jpg) | 1366×768 | Surface separation, prose and control contrast. |
| [Final: native dark](focus-after-dark-native.jpg) | 1200×850 | Real brief and overlay margin in the connected native app. |
| [Final: compact document](focus-after-preview-900.jpg) | 900×768 | Closed margin; two-row header and wrapped controls. |
| [Final: overlay margin](focus-after-margin-overlay-900.jpg) | 900×768 | Margin fits inside the workspace without squeezing the document. |
| [Final: long title](focus-after-long-title-1366.jpg) | 1366×768 | Truncation without horizontal overflow. |
| [Final: Markdown Edit](focus-after-edit-1366.jpg) | 1366×768 | Raw editable Markdown retained. |

The second pass refined toolbar wrapping, compact brief/source placement, truthful provenance/empty states, decision-callout styling, source counts and load/save feedback. The final native history recheck additionally corrected DTO mapping and keyboard access to evidence disclosures.

Measured browser geometry (CSS pixels, sidebar 264px):

| Viewport | Header | Sheet | Prose | Margin | Horizontal overflow |
|---|---|---|---|---|---|
| 1672×941 | 57px including border | 1082px | 800px | 320px docked | None in page or document scroll region |
| 1366×768 | 57px | 776px | 726px | 320px docked | None |
| 900×768 | 105px, two rows | 630px | 588px | Closed / 340px overlay | None |

At 900px the overlay was bounded by x=548…888 and y=117…756. The toolbar is one row at 1672px and wraps at 1366px with a docked margin. Home has no `aria-current` while Focus is active. The browser viewport override was reset after testing.

## Validation

| Check | Result |
|---|---|
| Baseline `npm test` | PASS — 19 files, 92 tests before this phase |
| Final `npm test` | PASS — 21 files, 104 tests |
| Final `npm run build` (`tsc && vite build`) | PASS — TypeScript and production frontend build |
| `cargo test --workspace --offline` | PASS — 119 tests; Rust unchanged |
| `git diff --check` | PASS |
| Lint | No lint script is defined in this package |

Added behavior coverage: compact/expanded brief and every evidence destination; backend collapse hint and default-closed margin; failed load/retry; failed save and dirty-text retention; saving under the original project before switching; distinct source counts; duplicate H1 suppression without changing checkbox-written Markdown; history empty state, Escape/focus restoration and disclosure keyboard loop; raw Rust history DTO mapping and legacy view-model compatibility.

Existing tests continue to cover slash commands, rich document behavior, typed-object/workspace organization, document stream placement, model controls, settings and autosave. Native UI verification reconfirmed saved Markdown through Home → Continue and application reload, title synchronization, Edit/Preview, brief expansion, margin tabs, actual history, keyboard disclosure and Escape. Browser Cmd+E, light/dark, long titles and responsive layouts were inspected.

No Rust/backend, vault format, provider configuration, secrets or owner research data was changed. Live AI/provider inference was not executed; existing mocked streaming/refine/derive coverage passed. A packaged Tauri release build was not run; the actual native development app was exercised against the live frontend.

Logs: `/private/tmp/sthira-focus-tests.log`, `/private/tmp/sthira-focus-build.log`, `/private/tmp/sthira-focus-rust-tests.log`. Vite retains the existing >500kB main-chunk warning (about 887kB minified); code splitting is outside this visual phase.

## Exact source/test files changed in this phase

All paths below are relative to repository root:

- `crates/sthira-desktop/ui/src/components/FocusView.tsx`
- `crates/sthira-desktop/ui/src/components/ContextualResumePane.tsx`
- `crates/sthira-desktop/ui/src/components/ProjectHistoryDrawer.tsx`
- `crates/sthira-desktop/ui/src/components/RichDocumentRenderer.tsx`
- `crates/sthira-desktop/ui/src/styles/focus-workspace.css` (new)
- `crates/sthira-desktop/ui/src/main.tsx` (Focus stylesheet import)
- `crates/sthira-desktop/ui/src/api/ipc.ts` (history response normalization)
- `crates/sthira-desktop/ui/src/types/sthira.ts` (history DTO/view-model metadata)
- `crates/sthira-desktop/ui/src/test/focus_slash_layout.test.tsx`
- `crates/sthira-desktop/ui/src/test/research_center_workspace.test.tsx`
- `crates/sthira-desktop/ui/src/test/focus_workspace_fidelity.test.tsx` (new)
- `crates/sthira-desktop/ui/src/test/history_ipc_mapping.test.ts` (new)
- `docs/design/visual-audit/focus-baseline.md`, this report, and `focus-*.jpg` captures

Other modified shell/Home files in `git status` belong to the preceding phase and were preserved. Theme tokens remain imported once; no additional palette, external font, illustration or dependency was added.

## Remaining differences and limits

1. The mockup's landscape washes and exact display font are absent. Existing Georgia/system fallbacks keep offline startup; no substitute artwork was invented.
2. Findings, takeaways, related items and resume height depend on actual vault evidence. The QA vault has no extracted claims. It deliberately does not reproduce the mockup's illustrative findings/actions. The margin is closed on entry and the brief follows the backend hint, even though the mockup shows both open.
3. The sidebar follows the specified 264px default rather than the image's wider illustrated sidebar. Prose uses the design system's readable sans body; Georgia supplies editorial headings. Narrow windows wrap controls and overlay the margin rather than retaining the wide mockup geometry.
4. Native captures intermittently retained stale transition/selection paint, especially after theme switches; some earlier native captures also masked the top toolbar. The final reload/fullscreen captures restored the header and main control styling, but muted metadata/icon styling in the selected Notes row still needs shared-sidebar contrast polish. Browser captures provide the clearest visual-state evidence. No screenshot was edited to hide these issues.
5. On the user's final connection recheck, native accessibility interaction and screenshot capture succeeded, including real history and keyboard disclosure. The existing IAB tab remained listed but its browser binding could not be reopened (`browser.user is unavailable`); prior completed responsive/dark screenshots remain the browser evidence. No further browser validation is claimed after that interruption.
6. Native Linux rendering, OS-level display scaling and live AI inference remain unverified in this macOS environment.

Next scoped visual phase: All Files and projects/custom objects, followed by Knowledge Graph/Canvas. Shared sidebar selection contrast should be included in the next shell-polish pass.
