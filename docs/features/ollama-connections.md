# Ollama connections and Research

Ollama's native API and its OpenAI-compatible API use different base paths. A connection saved as Ollama may contain a root URL, `/api`, `/v1`, or a copied native endpoint. `llm::endpoints::ollama_api_url` maps those saved addresses to exactly one native `/api/<operation>` path. It preserves the host and any reverse-proxy prefix. It does not change a connection's saved record, model choice, or API format. OpenAI-compatible connections retain their existing routing.

Discovery (`tags`), JSON synthesis (`generate`), embeddings (`embed`) and progressive generation (`generate`) share that builder. Native requests use the explicitly configured provider key, when present. Model/HTTP failures stay failures; they do not silently switch to another model or persist a fabricated generated report.

Settings → Web Search → Test Search checks the search provider only. A successful result does not prove that the selected AI model can synthesize a report. Zero results remain visible after the test. Model-side errors in the editor say Research failed and show the complete readable message rather than implying that DuckDuckGo failed. The saved inquiry/draft remains available.

The regression fixture deliberately allows DuckDuckGo-style search HTML while optionally returning a model endpoint 404. It verifies both successful research synthesis and error/draft preservation. Other fixtures cover discovery, selected model identity, authentication, embeddings and streaming for a saved `/v1` base, as well as root/native/proxy URL variants.

An explicit ignored smoke test uses an existing Ollama service and public research inquiry with a temporary vault only: set `STHIRA_QA_OLLAMA_ENDPOINT` and `STHIRA_QA_OLLAMA_MODEL`, then run `cargo test -p sthira-desktop --test ollama_endpoint_test live_existing_ollama_and_duckduckgo_research_smoke -- --ignored --nocapture`. It never installs/downloads models or opens the owner's vault. A run may receive no DuckDuckGo results; in that case, model generation must still complete with an honestly labeled local fallback. A pass of that fallback is not a live web-grounded result or a native GUI test.

See the official [API base URLs](https://docs.ollama.com/api/introduction) and [OpenAI compatibility](https://docs.ollama.com/api/openai-compatibility).
