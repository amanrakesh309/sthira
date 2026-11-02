# Home baseline — 2 November 2026

Active checkout: `/Volumes/Aman Ext/Projects/sthira`; frontend: `crates/sthira-desktop/ui`.
Approved anchor: `docs/design/references/home.png` (1672×941, including illustrative native chrome).
Observed before editing: Vite browser preview at 1× display scale, empty workspace:
`home-before-1672.jpg`, `home-before-1366.jpg` in this directory. The large captured raster is 1664×941 despite a requested/DOM-reported 1672×941 viewport; the smaller raster is exactly 1366×768. This is the real browser fallback, not the owner's Linux vault. Its model catalog is an existing preview fallback, not evidence of connected providers.

| Rank | Difference | Owner | Kind |
|---|---|---|---|
| 1 | At 1672px the main column begins at x488 and is only 976px wide; reference uses a broad workspace column. | HomeView | Geometry |
| 2 | Giant smooth SVG wave sits behind hero/input instead of a restrained empty header. No approved landscape asset exists. | HomeView | Decoration |
| 3 | Duplicate global search, two settings controls and extra Workspace label compete with the hero. | HomeView, LeftSidebar | Structure |
| 4 | Input has a strong divider and tightly packed colored mode/model/badge pills; model text crowds smaller widths. | UniversalInputBar | Hierarchy |
| 5 | Project icon sits above title, producing tall cards instead of the reference's compact horizontal title group. | HomeView | Structure |
| 6 | Resume rows have excess padding/gaps, inert overflow buttons, and pointer-only row navigation. | HomeView | Density, interaction |
| 7 | Empty project panel is 176px tall and pushes resumption below the 768px fold. | HomeView | Geometry |
| 8 | Sidebar defaults to 280px, has tiny 10–12px labels, weak group boundaries and inconsistent emoji/vector icons. | LeftSidebar | Typography, hierarchy |
| 9 | Serif section titles are 20px and UI text is often 12px, below the design's hierarchy. | HomeView, LeftSidebar | Typography |
| 10 | Hardcoded light/dark colors bypass loaded semantic tokens; main Home uses app background instead of canvas. | App, HomeView, UniversalInputBar, LeftSidebar | Color |
| 11 | Model fallback claims Ollama even with an empty catalog; footer claims synced without a sync signal. | UniversalInputBar, LeftSidebar | Status accuracy |
| 12 | Home excludes recent loose captures even though it does not render a loose-capture section. | HomeView | Resume hierarchy |

Baseline validation: `npm test`: 18 files / 85 tests pass. `npm run build`: pass, existing >500kB chunk warning. `cargo test --workspace`: pass. No lint script is configured. Tokens are imported exactly once by `src/main.tsx`, after Tailwind/global CSS; Tailwind 3 uses `html.dark` and theme context also sets `data-theme`. No AGENTS.md applies in the active checkout or its parent directories. No local licensed fonts or standalone landscape assets are bundled; use offline system sans + Georgia.
