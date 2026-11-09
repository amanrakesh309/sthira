# Document tabs

Tabs preserve several working contexts around one active document editor. Normal artifact navigation reuses the active tab; Ctrl/Cmd-click opens another. Existing open artifact identities are selected rather than duplicated, while keeping the explicitly chosen project context. Open another document launches search with a new-tab intention. Project navigation continues to open the overview.

The strip appears for two or more documents. Titles truncate within the tab, full titles remain available as tooltips, and the strip scrolls horizontally. Close removes UI state only. Move-left/right controls reorder tabs. Arrow/Home/End keys navigate the strip; Ctrl/Cmd+Tab cycles, Shift reverses it, and Ctrl/Cmd+W closes the active document tab where the host permits interception. Settings/search/modal states do not receive these document shortcuts.

The active FocusView registers a leave guard. Switching tabs, closing the active tab and shell navigation save a dirty page first; failure leaves that editor/draft active. Each tab tracks exact artifact ID, selected project, originating project, title and scroll position. Sidebar and tab labels follow manual and accepted title changes. Returning to a project preserves that project's current browse preferences within the running app.

Tab metadata and active-document restoration are saved in local UI storage under a vault-path digest supplied by the native backend. No document text is stored in tab metadata. Explicit launch routes take precedence over automatic restoration. Malformed storage recovers to an empty session. Storage unavailability leaves tabs usable in memory. A deleted/unavailable saved document displays the existing load error and can be closed.

Conversation history/model selection remains in its existing per-document database records. Unsent conversation text uses session storage keyed by workspace and artifact ID so switching tabs preserves it without mixing documents or vaults. It is not a claim of recovery after every OS/WebView process termination. Native close/relaunch and accelerator behavior require separate native GUI validation; component remount/browser reload assertions establish only their executed layer.

Split panes, tab groups and detached windows are outside this version.
