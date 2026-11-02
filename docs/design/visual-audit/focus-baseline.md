# Focus baseline — Phase 2

Approved reference: `docs/design/references/document-workspace.png` (1672×941). Inspected existing Focus, resume, rich preview, page actions, streaming, typed-object properties, and history owners before editing. Prior shell/Home changes remain in the working tree.

Native baseline: `focus-before-edit-native.jpg` and `focus-before-preview-margin-native.jpg`, captured in the existing temporary Visual QA vault. The current native window is 1920×1080; the latter captures the complete resume/title/preview/margin composition. Content was entered and saved through the real UI and is explicitly QA material, not owner research. Browser/reference-size and compact-window evidence will supplement native comparisons.

| Rank | Difference / evidence | Owner |
|---|---|---|
| 1 | Open margin leaves a large unused strip between the max-width document and docked margin. | FocusView canvas/grid |
| 2 | Header duplicates global search and uses purpose text as a project breadcrumb. | FocusView header |
| 3 | Preview repeats the document title, uses small sans headings and bright green heading bars. | RichDocumentRenderer, FocusView |
| 4 | Preview prose is 12–14px; reading width and spacing lack the reference's editorial rhythm. | RichDocumentRenderer |
| 5 | Boxed format controls and multiple pill surfaces compete with Edit/Preview and AI actions. | FocusView toolbar |
| 6 | Resume typography is tiny; decorative corner obscures the Resume action; provenance is replaced by generic green checks. | ContextualResumePane |
| 7 | Compact resume has no enforced 75-word limit; expanded Level 2 still slices progress to three items and hides conflicts. | ContextualResumePane |
| 8 | Brief ignores backend collapse hint; loading/saving failures have no visible recovery state. | FocusView state/presentation |
| 9 | Margin is dense and generic; source count excludes Markdown citations. Narrow windows squeeze the editor. | FocusView margin |
| 10 | History lacks dark styling, empty states, Escape/focus handling and readable narrative/timeline hierarchy. | ProjectHistoryDrawer |

Behavior guardrails: Markdown Edit remains a textarea; Preview remains rendered Markdown with checkboxes, tables, Mermaid and embedded objects. No fabricated takeaways, findings, sources, milestones, graph links or task status. Right margin remains collapsed by default. Preserve autosave, stream placement, slash commands, typed-object properties/templates, lineage, refine/derive, reversible proposal actions, evidence navigation and existing IPC/Rust behavior. Theme tokens stay imported once.
