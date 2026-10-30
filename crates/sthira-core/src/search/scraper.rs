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
static COMMENT_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?is)<!--.*?-->").unwrap());
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
static A_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?is)<a\b[^>]*href=["']([^"']*)["'][^>]*>(.*?)</a>"#).unwrap()
});
static ALL_TAGS_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<[^>]+>").unwrap());
static MULTI_NEWLINES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\n{3,}").unwrap());

pub fn decode_html_entities(text: &str) -> String {
    text.replace("&quot;", "\"")
        .replace("&#34;", "\"")
        .replace("&apos;", "'")
        .replace("&#39;", "'")
        .replace("&amp;", "&")
        .replace("&#38;", "&")
        .replace("&lt;", "<")
        .replace("&#60;", "<")
        .replace("&gt;", ">")
        .replace("&#62;", ">")
        .replace("&nbsp;", " ")
        .replace("&#160;", " ")
        .replace("&mdash;", "—")
        .replace("&ndash;", "–")
        .replace("&hellip;", "…")
        .replace("&trade;", "™")
        .replace("&copy;", "©")
        .replace("&reg;", "®")
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

    body = PRE_REGEX.replace_all(&body, "\n```\n$1\n```\n").into_owned();
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
        } else if cleaned_lines.last().map(|l: &String| !l.is_empty()).unwrap_or(false) {
            cleaned_lines.push(String::new());
        }
    }
    let cleaned_text = cleaned_lines.join("\n");
    let content_md = MULTI_NEWLINES.replace_all(&cleaned_text, "\n\n").trim().to_string();

    (title, content_md)
}

pub async fn fetch_and_clean_url(url: &str) -> Result<ScrapedWebDocument, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(12))
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36")
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {e}"))?;

    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Network request failed for '{}': {e}", url))?;

    if !resp.status().is_success() {
        return Err(format!("HTTP error {} fetching '{}'", resp.status(), url));
    }

    let html = resp
        .text()
        .await
        .map_err(|e| format!("Failed to read response body from '{}': {e}", url))?;

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
            if let Ok(Some(cached)) = crate::db::search_repo::get_cached_web_document(&reader, url) {
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
