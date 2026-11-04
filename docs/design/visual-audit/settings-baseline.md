# Model & Workspace Settings Baseline Visual Audit

**Reference Image**: `docs/design/references/settings.png` (1920×1080 / 1672×941)  
**Baseline Screenshots**:
- `docs/design/visual-audit/settings-before-1672.jpg` (1672×941)
- `docs/design/visual-audit/settings-before-1366.jpg` (1366×768)

## Overview & Scope

The Model & Workspace Settings modal (`ModelSettingsModal.tsx`) manages model provider configurations, available models, discovery, capabilities, search providers, appearance, and vault storage. The approved reference displays a spacious, elegant 2-column modal dialog (~1150-1200px wide, ~24px rounded corners) over a dimmed backdrop, with brand identity in the header, horizontal pill tabs, guidance sidebar, structured provider cards with accurate connection states, searchable model tables with capability badges, and clear footer controls.

## Ranked Discrepancies and Gap Analysis

| Priority | Dimension | Approved Reference (`settings.png`) | Existing Implementation (`ModelSettingsModal.tsx`) | Defect Impact & Action Required |
|:---:|---|---|---|---|
| **1** | **Modal Proportions & Layout Architecture** | Spacious ~1150–1200px wide modal with 2-column layout: Left column (~280px) contains "Your AI workspace, your way." guidance card + Quick Setup navigation shortcuts (`Web Search Providers`, `Appearance`, `Vault & Storage`). Right column contains provider cards and model listings. | Narrow single-column modal (`max-w-3xl`, ~768px wide) with vertically stacked provider blocks. Completely lacks the left guidance sidebar and quick setup shortcuts. | Critical: Expand modal max-width to ~1160px (`max-w-6xl`) and implement responsive 2-column architecture with guidance sidebar. |
| **2** | **Header Hierarchy & Brand Presence** | Left brand badge box with leaf icon + `Sthira` logo. Main title area features a distinctive small amber accent rule (`32px` wide, `3px` high), followed by Fraunces serif heading `Model & Workspace Settings` and subtitle (`Configure your models, tools, and workspace experience.`). Close button `✕` is a crisp circular button in the top right. | Header lacks brand badge, amber rule, and editorial serif font. Displays an inline generic sliders icon and cramped sans heading. Close button is a simple flat icon. | High: Rebuild header with Sthira brand badge, amber accent rule, editorial typography, and circular close affordance. |
| **3** | **Horizontal Navigation Tabs** | Discrete pill button tabs: `[cube] Connected Providers & Models` (dark olive green active pill), `[globe] Web Search Providers`, `[sun] Appearance`, `[layers] Vault & Storage`. Clean spacing below the header. | Traditional border-bottom underline tabs jammed into the header bar alongside the title, lacking pill containers. | High: Convert tabs into horizontal pill row with active `--sthira-primary` styling and clear icons. |
| **4** | **Provider Card Composition & Connection State** | Rounded card with provider avatar/logo, title, model count badge (`4 models`), and right-aligned connection badge (`✓ Connected` in green tint or `Not Connected`), plus `···` menu. Endpoint URL field with monospace font, accompanied by `Test Connection` and `Save` buttons. | Card header displays inline badge with provider type name and a dark `Fetch Models` button. Lacks the connection state indicator pill and `Test Connection` button styling. | High: Add accurate connection status badges, refine endpoint URL row with icon, Test Connection, and Save buttons. |
| **5** | **Available Models Table & Capabilities** | Section header with model count badge, search input (`🔍 Search models...`), and capability filter dropdown (`All Capabilities ⌄`). Each model row features: selection radio/indicator, model ID & display name, capability badges (`✨ General Purpose`, `⚡ Fast & Lightweight`, `✨ High Capability`), web search capability badge (`[globe] Web Search`), context window (`[layers] 128K context`), `Default` badge, and `···` menu. | Models are listed with simple text and an inline select dropdown for web scrape modes, lacking the searchable table layout, capability badges, and context window indicators. | High: Implement model search and capability filter, format model rows with structured capability chips and context window indicators. |
| **6** | **Modal Footer & Action Affordances** | Clean footer with `Cancel` button (neutral text/border) and `✓ Save Changes` primary button (`--sthira-primary`), preserving existing save/autosave logic. | Footer displays `OmniRouter Compute Fabric` subtext and a single `Done` button. | Medium: Align footer controls to Cancel and Save Changes while strictly preserving existing autosave and backend save triggers. |

## Baseline Verification Checks
- `npm test`: 134 tests passed across 25 test files.
- `npm run build`: 0 TypeScript errors, built in 1.10s.
- `cargo test --workspace --offline`: 119 tests passed.
- `git diff --check`: clean whitespace.
