# Rich text and retired page blocks

Generative/Open UI is deferred. Pages and streamed responses use ordinary Markdown in the existing rich-text editor. New generation prompts and schemas no longer request widget definitions; slash insertion and automatic Markdown-to-card hydration are removed.

Legacy valid plan, comparison and choice fences display as headings, task lists and paragraphs. Saved completion and choice state remain visible as ordinary document content. Opening does not write to the vault; the first subsequent user edit serializes the readable Markdown through normal save/history protections. Unsupported, malformed and future-version data remain literal code rather than being discarded. Other saved fields remain readable text.

The streaming protocol retains its existing message types for compatibility. New streams emit text and completion metrics without extracting or removing paragraphs. No structured-block control records decisions, creates projects or modifies documents. Existing explicitly authored objects, diagrams, normal task lists, links and images retain their separate contracts.
