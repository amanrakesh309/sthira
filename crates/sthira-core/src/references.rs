//! Stable workspace URLs shared by research citations and reference lookup.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkspaceReference {
    pub kind: String,
    pub id: String,
}

pub fn parse_workspace_reference(value: &str) -> Option<WorkspaceReference> {
    let value = value.trim();
    let value = value
        .strip_prefix('`')
        .and_then(|v| v.strip_suffix('`'))
        .or_else(|| value.strip_prefix('<').and_then(|v| v.strip_suffix('>')))
        .unwrap_or(value);
    let path = value
        .strip_prefix("sthira://")
        .or_else(|| value.strip_prefix("#/sthira/"))?;
    let (kind, encoded) = path.split_once('/')?;
    if !matches!(kind, "artifact" | "project") || encoded.is_empty() {
        return None;
    }
    let mut decoded = Vec::new();
    let mut bytes = encoded.bytes();
    while let Some(byte) = bytes.next() {
        if byte == b'%' {
            let high = (bytes.next()? as char).to_digit(16)?;
            let low = (bytes.next()? as char).to_digit(16)?;
            decoded.push((high * 16 + low) as u8);
        } else {
            decoded.push(byte);
        }
    }
    let id = String::from_utf8(decoded).ok()?;
    if id.trim().is_empty()
        || id.len() > 1536
        || id
            .chars()
            .any(|c| c.is_control() || matches!(c, '/' | '\\' | '?' | '#'))
    {
        return None;
    }
    Some(WorkspaceReference {
        kind: kind.into(),
        id,
    })
}

pub fn markdown_reference(title: &str, url: &str) -> String {
    let Some(reference) = parse_workspace_reference(url) else {
        return title.to_string();
    };
    let mut label = String::new();
    for c in title.chars() {
        if matches!(c, '\\' | '[' | ']' | '`' | '*' | '_' | '<' | '>') {
            label.push('\\');
        }
        label.push(if c.is_control() { ' ' } else { c });
    }
    let mut id = String::new();
    for byte in reference.id.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            id.push(byte as char);
        } else {
            id.push_str(&format!("%{byte:02X}"));
        }
    }
    format!("[{}](sthira://{}/{id})", label.trim(), reference.kind)
}
