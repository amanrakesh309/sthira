use super::ScrapedWebDocument;
use regex::Regex;
use std::sync::LazyLock;

static SCRIPT_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?is)<script\b[^>]*>.*?</script>").unwrap());
static STYLE_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?is)<style\b[^>]*>.*?</style>").unwrap());
static NAV_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?is)<nav\b[^>]*>.*?</nav>").unwrap());
static HEADER_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?is)<header\b[^>]*>.*?</header>").unwrap());
static FOOTER_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?is)<footer\b[^>]*>.*?</footer>").unwrap());
static SVG_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?is)<svg\b[^>]*>.*?</svg>").unwrap());
static ASIDE_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?is)<aside\b[^>]*>.*?</aside>").unwrap());
static NOSCRIPT_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?is)<noscript\b[^>]*>.*?</noscript>").unwrap());
static COMMENT_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?is)<!--.*?-->").unwrap());
static TITLE_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?is)<title\b[^>]*>(.*?)</title>").unwrap());

static H1_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?is)<h1\b[^>]*>(.*?)</h1>").unwrap());
static H2_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?is)<h2\b[^>]*>(.*?)</h2>").unwrap());
static H3_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?is)<h3\b[^>]*>(.*?)</h3>").unwrap());
static H4_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?is)<h4\b[^>]*>(.*?)</h4>").unwrap());
static H5_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?is)<h5\b[^>]*>(.*?)</h5>").unwrap());
static H6_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?is)<h6\b[^>]*>(.*?)</h6>").unwrap());

static BOLD_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?is)<(?:strong|b)\b[^>]*>(.*?)</(?:strong|b)>").unwrap());
static ITALIC_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?is)<(?:em|i)\b[^>]*>(.*?)</(?:em|i)>").unwrap());
static CODE_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?is)<code\b[^>]*>(.*?)</code>").unwrap());
static PRE_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?is)<pre\b[^>]*>(.*?)</pre>").unwrap());
static LI_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?is)<li\b[^>]*>(.*?)</li>").unwrap());
static BR_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?is)<br\b\s*/?>").unwrap());
static P_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?is)<p\b[^>]*>(.*?)</p>").unwrap());
static DIV_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?is)<div\b[^>]*>(.*?)</div>").unwrap());
static A_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?is)<a\b[^>]*href=["']([^"']*)["'][^>]*>(.*?)</a>"#).unwrap());
static ALL_TAGS_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<[^>]+>").unwrap());
static MULTI_NEWLINES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\n{3,}").unwrap());
static FORM_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?is)<form\b[^>]*>.*?</form>").unwrap());
static CODE_PRE_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?is)<(?:code|pre)\b[^>]*>.*?</(?:code|pre)>").unwrap());
static ARTICLE_MAIN_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?is)<(?:article|main)\b[^>]*>(.*?)</(?:article|main)>").unwrap()
});
static WHITESPACE_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+").unwrap());
static MD_FENCE_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?is)```[\s\S]*?```").unwrap());
static ACTIVE_CF_DOM_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?is)(?:\bid=["'](?:cf-challenge-running|challenge-running|challenge-error-title|cf-wrapper)["']|class=["'][^"']*\bcf-browser-verification\b|<script\b[^>]+src=["'][^"']*/cdn-cgi/challenge-platform/|<form\b[^>]+action=["'][^"']*(?:/__cf_chl|/cdn-cgi/l/chk_))"#).unwrap()
});
static CAPTCHA_WIDGET_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?is)(?:class=["'][^"']*\b(?:g-recaptcha|h-captcha|cf-turnstile)\b|\bdata-sitekey\s*=|<script\b[^>]+src=["'][^"']*(?:recaptcha/api\.js|hcaptcha\.com/1/api\.js|challenges\.cloudflare\.com/turnstile))"#).unwrap()
});
static ACTIVE_CAPTCHA_FORM_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?is)<form\b[^>]*(?:\bid|\bclass)=["'][^"']*(?:challenge-form|captcha-form)[^"']*["']"#,
    )
    .unwrap()
});

pub fn decode_html_entities(text: &str) -> String {
    let mut s = text
        .replace("&quot;", "\"")
        .replace("&#34;", "\"")
        .replace("&#x22;", "\"")
        .replace("&apos;", "'")
        .replace("&#39;", "'")
        .replace("&#x27;", "'")
        .replace("&amp;", "&")
        .replace("&#38;", "&")
        .replace("&#x26;", "&")
        .replace("&lt;", "<")
        .replace("&#60;", "<")
        .replace("&#x3C;", "<")
        .replace("&#x3c;", "<")
        .replace("&gt;", ">")
        .replace("&#62;", ">")
        .replace("&#x3E;", ">")
        .replace("&#x3e;", ">")
        .replace("&nbsp;", " ")
        .replace("&#160;", " ")
        .replace("&mdash;", "—")
        .replace("&ndash;", "–")
        .replace("&hellip;", "…")
        .replace("&trade;", "™")
        .replace("&copy;", "©")
        .replace("&reg;", "®");

    if s.contains("&#") {
        s = s
            .replace("&#8211;", "–")
            .replace("&#8212;", "—")
            .replace("&#8216;", "‘")
            .replace("&#8217;", "’")
            .replace("&#8220;", "“")
            .replace("&#8221;", "”")
            .replace("&#8230;", "…");
    }
    s
}

fn extract_readable_text(html_fragment: &str, strip_forms: bool) -> String {
    let mut s = SCRIPT_REGEX.replace_all(html_fragment, " ").into_owned();
    s = STYLE_REGEX.replace_all(&s, " ").into_owned();
    s = NAV_REGEX.replace_all(&s, " ").into_owned();
    s = HEADER_REGEX.replace_all(&s, " ").into_owned();
    s = FOOTER_REGEX.replace_all(&s, " ").into_owned();
    s = SVG_REGEX.replace_all(&s, " ").into_owned();
    s = ASIDE_REGEX.replace_all(&s, " ").into_owned();
    s = NOSCRIPT_REGEX.replace_all(&s, " ").into_owned();
    s = COMMENT_REGEX.replace_all(&s, " ").into_owned();
    if strip_forms {
        s = FORM_REGEX.replace_all(&s, " ").into_owned();
    }
    s = ALL_TAGS_REGEX.replace_all(&s, " ").into_owned();
    s = decode_html_entities(&s);
    WHITESPACE_REGEX.replace_all(&s, " ").trim().to_string()
}

fn has_strong_gate_title(title_lower: &str) -> bool {
    title_lower == "just a moment..."
        || title_lower.starts_with("attention required! | cloudflare")
        || title_lower == "access denied | cloudflare"
        || title_lower == "403 forbidden | cloudflare"
        || title_lower == "security check | cloudflare"
        || title_lower == "sorry, you have been blocked"
        || title_lower == "you have been blocked"
        || title_lower.contains("access to this page has been denied")
        || title_lower == "403 forbidden"
        || title_lower == "bot verification"
        || title_lower == "access denied"
        || title_lower == "security check"
}

fn has_ambiguous_gate_title(title_lower: &str) -> bool {
    const CUES: &[&str] = &[
        "access denied",
        "403 forbidden",
        "security check",
        "bot verification",
        "just a moment",
    ];
    CUES.iter().any(|cue| {
        if title_lower == *cue {
            return true;
        }
        if let Some(rest) = title_lower.strip_prefix(cue) {
            let trimmed = rest.trim_start();
            if trimmed.starts_with(':')
                || trimmed.starts_with('|')
                || trimmed.starts_with('—')
                || trimmed.starts_with('–')
                || (rest.starts_with(" -") || trimmed.starts_with("- "))
            {
                return true;
            }
        }
        if let Some(prefix) = title_lower.strip_suffix(cue) {
            let trimmed = prefix.trim_end();
            if trimmed.ends_with(':')
                || trimmed.ends_with('|')
                || trimmed.ends_with('—')
                || trimmed.ends_with('–')
                || prefix.ends_with("- ")
            {
                return true;
            }
        }
        false
    })
}

fn has_ambiguous_gate_context(readable_text: &str) -> bool {
    let lower = WHITESPACE_REGEX
        .replace_all(readable_text.trim(), " ")
        .to_ascii_lowercase();
    const GATE_CONTEXT_PHRASES: &[&str] = &[
        "you don't have permission to access",
        "you do not have permission to access",
        "you are not authorized to access",
        "you are unable to access this website",
        "your access to this site",
        "your request has been blocked",
        "your request was blocked",
        "has been blocked from accessing",
        "owner of this website has banned",
        "sorry, you have been blocked",
        "you have been blocked",
        "why have i been blocked",
        "security service to protect itself",
        "please complete the security check",
        "please confirm you are a human",
        "please verify you are human",
        "please verify you are a human",
        "checking your browser",
        "checking if the site connection is secure",
        "needs to review the security of your connection",
        "enable javascript and cookies to continue",
        "please enable cookies",
        "cloudflare ray id",
        "reference #",
        "incident id:",
        "support id:",
        "the request could not be satisfied",
        "generated by cloudfront",
        "unusual traffic from your computer",
        "automated requests from your",
    ];
    GATE_CONTEXT_PHRASES.iter().any(|p| lower.contains(p))
}

fn has_strong_block_copy(readable_text: &str) -> bool {
    let trimmed = readable_text.trim();
    if trimmed.is_empty() || trimmed.len() > 8000 {
        return false;
    }
    let lower = WHITESPACE_REGEX
        .replace_all(trimmed, " ")
        .to_ascii_lowercase();

    let cloudflare_block = lower.contains("sorry, you have been blocked")
        && (lower.contains("unable to access this website")
            || lower.contains("why have i been blocked")
            || lower.contains("security service to protect itself")
            || lower.contains("email the site owner")
            || lower.contains("cloudflare ray id"));

    let cloudflare_security = lower
        .contains("this website is using a security service to protect itself")
        && (lower.contains("triggered the security solution")
            || lower.contains("you have been blocked")
            || lower.contains("unable to access"));

    let browser_interstitial = lower.contains("checking if the site connection is secure")
        && lower.contains("needs to review the security of your connection");

    let explicit_permission_gate = lower.contains("you don't have permission to access")
        && (lower.contains("on this server")
            || lower.contains("reference #")
            || lower.contains("incident id")
            || lower.contains("contact your administrator"));

    let security_gate_instruction = (lower
        .contains("please complete the security check to access")
        || lower.contains("please confirm you are a human by completing")
        || lower.contains("please stand by, while we are checking your browser"))
        && (lower.contains("security check")
            || lower.contains("cloudflare")
            || lower.contains("blocked")
            || lower.contains("human"));

    let ddg_anomaly_copy = lower.contains("bots use duckduckgo too")
        && (lower.contains("complete the following challenge")
            || lower.contains("confirm this search was made by a human"));

    cloudflare_block
        || cloudflare_security
        || browser_interstitial
        || explicit_permission_gate
        || security_gate_instruction
        || ddg_anomaly_copy
}

fn looks_purely_like_challenge_copy(readable_text: &str) -> bool {
    let lower = readable_text.trim().to_ascii_lowercase();
    if lower.len() >= 320 {
        return false;
    }
    const GATE_PHRASES: &[&str] = &[
        "please complete the security check",
        "checking your browser before accessing",
        "verify you are human",
        "please verify you are a human",
        "confirm this search was made by a human",
        "complete the following challenge",
        "you don't have permission to access",
        "enable javascript and cookies to continue",
    ];
    GATE_PHRASES.iter().any(|p| lower.contains(p))
        || lower.starts_with("checking your browser")
        || lower.starts_with("just a moment")
        || lower.starts_with("bot verification")
        || lower.starts_with("access denied")
        || lower.starts_with("security check")
}

fn has_accessible_article_body(html_without_code: &str, non_form_readable: &str) -> bool {
    let mut article_buf = String::new();
    for cap in ARTICLE_MAIN_REGEX.captures_iter(html_without_code) {
        if let Some(inner) = cap.get(1) {
            let chunk = extract_readable_text(inner.as_str(), true);
            if !chunk.is_empty() {
                if !article_buf.is_empty() {
                    article_buf.push(' ');
                }
                article_buf.push_str(&chunk);
            }
        }
    }
    if article_buf.len() >= 60
        && !looks_purely_like_challenge_copy(&article_buf)
        && !has_strong_block_copy(&article_buf)
    {
        return true;
    }
    non_form_readable.len() >= 180
        && !looks_purely_like_challenge_copy(non_form_readable)
        && !has_strong_block_copy(non_form_readable)
}

pub fn is_challenge_html(html: &str) -> bool {
    let no_code = CODE_PRE_REGEX.replace_all(html, " ");
    let no_code_comments = COMMENT_REGEX.replace_all(&no_code, " ");
    let non_form_readable = extract_readable_text(&no_code_comments, true);

    let title_lower = TITLE_REGEX
        .captures(html)
        .and_then(|c| c.get(1))
        .map(|m| {
            WHITESPACE_REGEX
                .replace_all(&decode_html_entities(m.as_str()), " ")
                .trim()
                .to_ascii_lowercase()
        })
        .unwrap_or_default();

    // Tier 1: Strong active blocking signals — never bypassed by long troubleshooting copy.
    if has_strong_gate_title(&title_lower)
        || ACTIVE_CF_DOM_REGEX.is_match(&no_code_comments)
        || super::is_duckduckgo_challenge(html)
        || has_strong_block_copy(&non_form_readable)
    {
        return true;
    }

    // Ambiguous gate-title cues (e.g. "Access Denied: ...", "Security Check | ...") only
    // block when paired with active blocking context, unaccessible/stub body, or active block copy.
    if has_ambiguous_gate_title(&title_lower) && has_ambiguous_gate_context(&non_form_readable) {
        return true;
    }

    // Tier 2: Widget / captcha-form / short copy / ambiguous title without accessible article body.
    let without_scripts = SCRIPT_REGEX.replace_all(&no_code_comments, " ");
    if has_ambiguous_gate_title(&title_lower)
        && ACTIVE_CAPTCHA_FORM_REGEX.is_match(&without_scripts)
    {
        return true;
    }

    let accessible = has_accessible_article_body(&no_code_comments, &non_form_readable);
    if has_ambiguous_gate_title(&title_lower) && !accessible {
        return true;
    }

    if (CAPTCHA_WIDGET_REGEX.is_match(&no_code_comments)
        || ACTIVE_CAPTCHA_FORM_REGEX.is_match(&without_scripts))
        && !accessible
    {
        return true;
    }

    !accessible
        && !non_form_readable.is_empty()
        && looks_purely_like_challenge_copy(&non_form_readable)
}

pub fn is_blocked_cached_document(title: &str, content_md: &str) -> bool {
    let body_trim = content_md.trim();
    if body_trim.is_empty() {
        return true;
    }
    let non_code_md = MD_FENCE_REGEX.replace_all(body_trim, " ");
    let non_code_trim = non_code_md.trim();
    let title_lower = WHITESPACE_REGEX
        .replace_all(title.trim(), " ")
        .to_ascii_lowercase();

    if has_strong_gate_title(&title_lower) || has_strong_block_copy(non_code_trim) {
        return true;
    }

    if has_ambiguous_gate_title(&title_lower)
        && (non_code_trim.len() < 80
            || has_ambiguous_gate_context(non_code_trim)
            || looks_purely_like_challenge_copy(non_code_trim))
    {
        return true;
    }

    if body_trim.len() < 500 && super::is_duckduckgo_challenge(non_code_trim) {
        return true;
    }

    let body_lower = non_code_trim.to_ascii_lowercase();
    body_trim.len() < 320
        && looks_purely_like_challenge_copy(non_code_trim)
        && (body_lower.contains("cf-browser-verification")
            || body_lower.contains("anomaly-modal")
            || body_lower.contains("bots use duckduckgo too")
            || body_lower.contains("security check"))
}

pub fn clean_html_to_markdown(html: &str, url: &str) -> (String, String) {
    let raw_title = TITLE_REGEX
        .captures(html)
        .and_then(|c| c.get(1))
        .map(|m| decode_html_entities(m.as_str().trim()))
        .unwrap_or_default();

    let title = if raw_title.is_empty() {
        url.trim_start_matches("https://")
            .trim_start_matches("http://")
            .split('/')
            .next()
            .unwrap_or("Web Document")
            .to_string()
    } else {
        raw_title
    };

    let mut body = html.to_string();

    body = SCRIPT_REGEX.replace_all(&body, "").into_owned();
    body = STYLE_REGEX.replace_all(&body, "").into_owned();
    body = NAV_REGEX.replace_all(&body, "").into_owned();
    body = HEADER_REGEX.replace_all(&body, "").into_owned();
    body = FOOTER_REGEX.replace_all(&body, "").into_owned();
    body = SVG_REGEX.replace_all(&body, "").into_owned();
    body = ASIDE_REGEX.replace_all(&body, "").into_owned();
    body = NOSCRIPT_REGEX.replace_all(&body, "").into_owned();
    body = COMMENT_REGEX.replace_all(&body, "").into_owned();

    body = H1_REGEX.replace_all(&body, "\n# $1\n").into_owned();
    body = H2_REGEX.replace_all(&body, "\n## $1\n").into_owned();
    body = H3_REGEX.replace_all(&body, "\n### $1\n").into_owned();
    body = H4_REGEX.replace_all(&body, "\n#### $1\n").into_owned();
    body = H5_REGEX.replace_all(&body, "\n##### $1\n").into_owned();
    body = H6_REGEX.replace_all(&body, "\n###### $1\n").into_owned();

    body = PRE_REGEX
        .replace_all(&body, "\n```\n$1\n```\n")
        .into_owned();
    body = CODE_REGEX.replace_all(&body, "`$1`").into_owned();
    body = BOLD_REGEX.replace_all(&body, "**$1**").into_owned();
    body = ITALIC_REGEX.replace_all(&body, "*$1*").into_owned();
    body = LI_REGEX.replace_all(&body, "\n- $1").into_owned();
    body = BR_REGEX.replace_all(&body, "\n").into_owned();
    body = P_REGEX.replace_all(&body, "\n\n$1\n\n").into_owned();
    body = DIV_REGEX.replace_all(&body, "\n$1\n").into_owned();
    body = A_REGEX.replace_all(&body, " [$2]($1) ").into_owned();

    body = ALL_TAGS_REGEX.replace_all(&body, " ").into_owned();
    body = decode_html_entities(&body);

    let mut cleaned_lines = Vec::new();
    for line in body.lines() {
        let trimmed = line.trim();
        if !trimmed.is_empty() {
            cleaned_lines.push(trimmed.to_string());
        } else if cleaned_lines
            .last()
            .map(|l: &String| !l.is_empty())
            .unwrap_or(false)
        {
            cleaned_lines.push(String::new());
        }
    }
    let cleaned_text = cleaned_lines.join("\n");
    let content_md = MULTI_NEWLINES
        .replace_all(&cleaned_text, "\n\n")
        .trim()
        .to_string();

    (title, content_md)
}

pub async fn fetch_and_clean_url(url: &str) -> Result<ScrapedWebDocument, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(12))
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36")
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {e}"))?;

    let resp = client.get(url).send().await.map_err(|e| {
        if super::is_reqwest_timeout(&e) {
            format!("Request timed out fetching '{}': {e}", url)
        } else {
            format!("Network request failed for '{}': {e}", url)
        }
    })?;

    if !resp.status().is_success() {
        return Err(format!("HTTP error {} fetching '{}'", resp.status(), url));
    }

    let html = resp.text().await.map_err(|e| {
        if super::is_reqwest_timeout(&e) {
            format!("Response body read timed out for '{}': {e}", url)
        } else {
            format!("Failed to read response body from '{}': {e}", url)
        }
    })?;

    if is_challenge_html(&html) {
        return Err(format!(
            "Fetched URL '{}' returned an anti-bot challenge page",
            url
        ));
    }

    let (title, content_md) = clean_html_to_markdown(&html, url);

    Ok(ScrapedWebDocument {
        url: url.to_string(),
        title,
        content_md,
        published_date: None,
    })
}

pub async fn scrape_and_clean_url(
    url: &str,
    db: Option<&crate::db::Db>,
    now_ms: i64,
) -> Result<ScrapedWebDocument, String> {
    if let Some(database) = db {
        if let Ok(reader) = database.reader() {
            if let Ok(Some(cached)) = crate::db::search_repo::get_cached_web_document(&reader, url)
            {
                return Ok(cached);
            }
        }
    }

    let doc = fetch_and_clean_url(url).await?;

    if let Some(database) = db {
        let writer = database.writer();
        let _ = crate::db::search_repo::save_cached_web_document(&writer, &doc, now_ms);
    }

    Ok(doc)
}
