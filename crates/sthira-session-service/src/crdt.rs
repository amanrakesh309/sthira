//! Yrs/Yjs binary update verification, state-vector reconciliation, and TipTap
//! ProseMirror `XmlFragment("default")` -> CommonMark/GFM serialization & verification.

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use markdown::{
    mdast::{AlignKind, Node},
    to_mdast, ParseOptions,
};
use std::collections::HashMap;
use yrs::{
    types::text::YChange,
    updates::{decoder::Decode, encoder::Encode},
    Any, Doc, GetString, Out, ReadTxn, StateVector, Text, Transact, Update, Xml, XmlElementPrelim,
    XmlElementRef, XmlFragment, XmlOut, XmlTextPrelim, XmlTextRef,
};

pub const MAX_YJS_STATE_BYTES: usize = 512 * 1024;
pub const MAX_YJS_UPDATE_BYTES: usize = 128 * 1024;

/// Decodes and applies one or more Yjs v1 binary updates on top of an optional
/// existing Yjs v1 snapshot, returning the merged canonical v1 snapshot bytes.
pub fn merge_yjs_updates(base_state: Option<&[u8]>, updates: &[&[u8]]) -> Result<Vec<u8>, String> {
    let doc = Doc::new();
    {
        let mut txn = doc.transact_mut();
        if let Some(base) = base_state {
            if !base.is_empty() {
                if base.len() > MAX_YJS_STATE_BYTES {
                    return Err("Live draft state exceeds maximum allowed size.".into());
                }
                let u = Update::decode_v1(base)
                    .map_err(|e| format!("Corrupt stored live draft state: {e}"))?;
                txn.apply_update(u)
                    .map_err(|e| format!("Failed to load live draft state: {e}"))?;
            }
        }
        for raw in updates {
            if raw.is_empty() {
                continue;
            }
            if raw.len() > MAX_YJS_UPDATE_BYTES {
                return Err("Live draft edit update exceeds maximum size.".into());
            }
            let u = Update::decode_v1(raw)
                .map_err(|e| format!("Malformed collaborative update: {e}"))?;
            txn.apply_update(u)
                .map_err(|e| format!("Failed to apply collaborative update: {e}"))?;
        }
        let encoded = txn.encode_state_as_update_v1(&StateVector::default());
        if encoded.len() > MAX_YJS_STATE_BYTES {
            return Err("Live draft state exceeds maximum allowed size.".into());
        }
        Ok(encoded)
    }
}

/// Validates a single client update bytes slice by decoding it as Yjs v1.
pub fn validate_yjs_update(update_bytes: &[u8]) -> Result<(), String> {
    if update_bytes.is_empty() {
        return Err("Empty collaborative update.".into());
    }
    if update_bytes.len() > MAX_YJS_UPDATE_BYTES {
        return Err("Collaborative update exceeds maximum allowed size.".into());
    }
    Update::decode_v1(update_bytes).map_err(|e| format!("Malformed collaborative update: {e}"))?;
    Ok(())
}

/// Computes the missing Yjs v1 update diff relative to a client's encoded v1 state vector.
pub fn diff_from_state_vector(
    state_bytes: &[u8],
    client_sv_bytes: &[u8],
) -> Result<Vec<u8>, String> {
    let doc = load_doc(state_bytes)?;
    let sv = if client_sv_bytes.is_empty() {
        StateVector::default()
    } else {
        StateVector::decode_v1(client_sv_bytes)
            .map_err(|e| format!("Malformed client state vector: {e}"))?
    };
    let txn = doc.transact();
    Ok(txn.encode_diff_v1(&sv))
}

/// Returns the Yjs v1 state vector encoded bytes for the stored snapshot.
pub fn state_vector_bytes(state_bytes: &[u8]) -> Result<Vec<u8>, String> {
    let doc = load_doc(state_bytes)?;
    let txn = doc.transact();
    Ok(txn.state_vector().encode_v1())
}

pub fn decode_b64(raw: &str) -> Result<Vec<u8>, String> {
    B64.decode(raw.trim())
        .map_err(|_| "Malformed base64 collaborative payload.".into())
}

pub fn encode_b64(bytes: &[u8]) -> String {
    B64.encode(bytes)
}

fn load_doc(state_bytes: &[u8]) -> Result<Doc, String> {
    if state_bytes.len() > MAX_YJS_STATE_BYTES {
        return Err("Live draft state exceeds maximum allowed size.".into());
    }
    let doc = Doc::new();
    if !state_bytes.is_empty() {
        let mut txn = doc.transact_mut();
        let u = Update::decode_v1(state_bytes)
            .map_err(|e| format!("Malformed collaborative state: {e}"))?;
        txn.apply_update(u)
            .map_err(|e| format!("Failed to apply collaborative state: {e}"))?;
    }
    Ok(doc)
}

/// Extracts `(title, content_md)` directly from the authoritative Yrs `Doc`
/// containing `Y.Text("title")` and TipTap's `Y.XmlFragment("default")`.
pub fn extract_title_and_markdown(state_bytes: &[u8]) -> Result<(String, String), String> {
    let doc = load_doc(state_bytes)?;
    let title_ref = doc.get_or_insert_text("title");
    let frag_ref = doc.get_or_insert_xml_fragment("default");
    let txn = doc.transact();
    let title = title_ref.get_string(&txn);

    let mut blocks = Vec::new();
    for child in frag_ref.children(&txn) {
        if let Some(rendered) = render_block(&txn, &child, 0) {
            if !rendered.is_empty() {
                blocks.push(rendered);
            }
        }
    }
    // Trim trailing empty paragraph TipTap inserts at end of document.
    while blocks.last().is_some_and(|b| b.trim().is_empty()) {
        blocks.pop();
    }
    Ok((title.trim().to_owned(), blocks.join("\n\n")))
}

/// Verifies that a client-supplied initial Yjs seed matches the exact published
/// revision's title and Markdown semantics cleanly before storing it unattributed.
pub fn verify_seed_matches_revision(
    seed_bytes: &[u8],
    expected_title: &str,
    expected_content_md: &str,
) -> Result<(), String> {
    validate_yjs_update(seed_bytes)?;
    let (extracted_title, extracted_md) = extract_title_and_markdown(seed_bytes)?;
    if extracted_title.trim() != expected_title.trim() {
        return Err(
            "Initial collaborative seed title does not match the published page revision.".into(),
        );
    }
    if !markdown_ast_equivalent(&extracted_md, expected_content_md)? {
        return Err(
            "Initial collaborative seed content does not match the published page revision.".into(),
        );
    }
    Ok(())
}

/// Returns true iff `left_md` and `right_md` parse to identical CommonMark/GFM semantics.
pub fn markdown_ast_equivalent(left_md: &str, right_md: &str) -> Result<bool, String> {
    let left_ast = to_mdast(left_md.trim(), &ParseOptions::gfm())
        .map_err(|e| format!("Failed to parse left Markdown: {e}"))?;
    let right_ast = to_mdast(right_md.trim(), &ParseOptions::gfm())
        .map_err(|e| format!("Failed to parse right Markdown: {e}"))?;
    Ok(normalize_mdast(&left_ast) == normalize_mdast(&right_ast))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NormAlign {
    Left,
    Center,
    Right,
    None,
}

impl From<AlignKind> for NormAlign {
    fn from(value: AlignKind) -> Self {
        match value {
            AlignKind::Left => Self::Left,
            AlignKind::Center => Self::Center,
            AlignKind::Right => Self::Right,
            AlignKind::None => Self::None,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum NormBlock {
    Heading(u8, Vec<NormInline>),
    Paragraph(Vec<NormInline>),
    Code(Option<String>, Option<String>, String),
    Blockquote(Vec<NormBlock>),
    List {
        ordered: bool,
        start: Option<u32>,
        items: Vec<NormListItem>,
    },
    Table {
        align: Vec<NormAlign>,
        rows: Vec<Vec<Vec<NormInline>>>,
    },
    Rule,
    Html(String),
}

#[derive(Debug, PartialEq, Eq)]
struct NormListItem {
    checked: Option<bool>,
    blocks: Vec<NormBlock>,
}

#[derive(Debug, PartialEq, Eq)]
enum NormInline {
    Text(String),
    Code(String),
    Break,
    Strong(Vec<NormInline>),
    Emphasis(Vec<NormInline>),
    Delete(Vec<NormInline>),
    Link(String, Option<String>, Vec<NormInline>),
    Image(String, String, Option<String>),
    Html(String),
}

type RefDefMap = HashMap<String, (String, Option<String>)>;

fn normalize_ref_id(id: &str) -> String {
    id.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

fn collect_mdast_definitions(node: &Node, defs: &mut RefDefMap) {
    if let Node::Definition(d) = node {
        let key = normalize_ref_id(&d.identifier);
        if !key.is_empty() {
            defs.entry(key).or_insert_with(|| {
                (
                    d.url.clone(),
                    d.title.clone().filter(|t| !t.trim().is_empty()),
                )
            });
        }
    }
    if let Some(children) = node.children() {
        for ch in children {
            collect_mdast_definitions(ch, defs);
        }
    }
}

fn push_norm_text(out: &mut Vec<NormInline>, s: &str) {
    if s.is_empty() {
        return;
    }
    if let Some(NormInline::Text(prev)) = out.last_mut() {
        prev.push_str(s);
    } else {
        out.push(NormInline::Text(s.to_owned()));
    }
}

fn normalize_inline_nodes(nodes: &[Node], defs: &RefDefMap, out: &mut Vec<NormInline>) {
    for n in nodes {
        match n {
            Node::Text(t) => push_norm_text(out, &t.value),
            Node::InlineCode(c) => out.push(NormInline::Code(c.value.clone())),
            Node::Break(_) => out.push(NormInline::Break),
            Node::Strong(s) => {
                let mut inner = Vec::new();
                normalize_inline_nodes(&s.children, defs, &mut inner);
                out.push(NormInline::Strong(inner));
            }
            Node::Emphasis(e) => {
                let mut inner = Vec::new();
                normalize_inline_nodes(&e.children, defs, &mut inner);
                out.push(NormInline::Emphasis(inner));
            }
            Node::Delete(d) => {
                let mut inner = Vec::new();
                normalize_inline_nodes(&d.children, defs, &mut inner);
                out.push(NormInline::Delete(inner));
            }
            Node::Link(l) => {
                let mut inner = Vec::new();
                normalize_inline_nodes(&l.children, defs, &mut inner);
                out.push(NormInline::Link(
                    l.url.clone(),
                    l.title.clone().filter(|t| !t.is_empty()),
                    inner,
                ));
            }
            Node::LinkReference(lr) => {
                let key = normalize_ref_id(&lr.identifier);
                if let Some((url, title)) = defs.get(&key) {
                    let mut inner = Vec::new();
                    normalize_inline_nodes(&lr.children, defs, &mut inner);
                    out.push(NormInline::Link(url.clone(), title.clone(), inner));
                } else {
                    push_norm_text(out, "[");
                    normalize_inline_nodes(&lr.children, defs, out);
                    push_norm_text(out, "]");
                }
            }
            Node::Image(img) => {
                out.push(NormInline::Image(
                    img.url.clone(),
                    img.alt.clone(),
                    img.title.clone().filter(|t| !t.is_empty()),
                ));
            }
            Node::ImageReference(ir) => {
                let key = normalize_ref_id(&ir.identifier);
                if let Some((url, title)) = defs.get(&key) {
                    out.push(NormInline::Image(
                        url.clone(),
                        ir.alt.clone(),
                        title.clone(),
                    ));
                } else {
                    push_norm_text(out, &format!("![{}]", ir.alt));
                }
            }
            Node::Html(h) => out.push(NormInline::Html(h.value.trim().to_owned())),
            _ => {
                if let Some(children) = n.children() {
                    normalize_inline_nodes(children, defs, out);
                }
            }
        }
    }
}

fn normalize_blocks(nodes: &[Node], defs: &RefDefMap) -> Vec<NormBlock> {
    let mut out = Vec::new();
    for n in nodes {
        match n {
            Node::Root(r) => out.extend(normalize_blocks(&r.children, defs)),
            Node::Heading(h) => {
                let mut inner = Vec::new();
                normalize_inline_nodes(&h.children, defs, &mut inner);
                out.push(NormBlock::Heading(h.depth, inner));
            }
            Node::Paragraph(p) => {
                // In GFM, standalone `<img>` may appear inside a Paragraph; preserve consistently.
                let mut inner = Vec::new();
                normalize_inline_nodes(&p.children, defs, &mut inner);
                if !inner.is_empty() {
                    out.push(NormBlock::Paragraph(inner));
                }
            }
            Node::Code(c) => {
                let lang = c
                    .lang
                    .clone()
                    .map(|l| l.trim().to_owned())
                    .filter(|l| !l.is_empty());
                out.push(NormBlock::Code(lang, c.meta.clone(), c.value.clone()));
            }
            Node::Blockquote(b) => {
                out.push(NormBlock::Blockquote(normalize_blocks(&b.children, defs)));
            }
            Node::List(l) => {
                let mut items = Vec::new();
                for child in &l.children {
                    if let Node::ListItem(li) = child {
                        items.push(NormListItem {
                            checked: li.checked,
                            blocks: normalize_blocks(&li.children, defs),
                        });
                    }
                }
                let start = if l.ordered {
                    Some(l.start.unwrap_or(1))
                } else {
                    None
                };
                out.push(NormBlock::List {
                    ordered: l.ordered,
                    start,
                    items,
                });
            }
            Node::Table(t) => {
                let mut rows = Vec::new();
                for row_node in &t.children {
                    if let Node::TableRow(tr) = row_node {
                        let mut cells = Vec::new();
                        for cell_node in &tr.children {
                            if let Node::TableCell(tc) = cell_node {
                                let mut inner = Vec::new();
                                normalize_inline_nodes(&tc.children, defs, &mut inner);
                                cells.push(inner);
                            }
                        }
                        rows.push(cells);
                    }
                }
                let cols = rows.iter().map(|r| r.len()).max().unwrap_or(t.align.len());
                let mut align: Vec<NormAlign> =
                    t.align.iter().copied().map(NormAlign::from).collect();
                align.resize(cols, NormAlign::None);
                out.push(NormBlock::Table { align, rows });
            }
            Node::ThematicBreak(_) => out.push(NormBlock::Rule),
            Node::Html(h) => out.push(NormBlock::Html(h.value.trim().to_owned())),
            Node::Definition(_) => {}
            _ => {
                if let Some(children) = n.children() {
                    out.extend(normalize_blocks(children, defs));
                }
            }
        }
    }
    out
}

fn normalize_mdast(root: &Node) -> Vec<NormBlock> {
    let mut defs = RefDefMap::new();
    collect_mdast_definitions(root, &mut defs);
    normalize_blocks(std::slice::from_ref(root), &defs)
}

/// Seeds a TipTap-compatible Yrs snapshot directly from `(title, content_md)` on the
/// server without trusting client-supplied initial state.
pub fn seed_basic_yjs_update(title: &str, content_md: &str) -> Vec<u8> {
    let doc = Doc::new();
    let title_ref = doc.get_or_insert_text("title");
    let frag_ref = doc.get_or_insert_xml_fragment("default");
    let mut txn = doc.transact_mut();
    if !title.is_empty() {
        title_ref.insert(&mut txn, 0, title);
    }
    if let Ok(root_node) = to_mdast(content_md, &ParseOptions::gfm()) {
        let mut defs = RefDefMap::new();
        collect_mdast_definitions(&root_node, &mut defs);
        if let Node::Root(root) = root_node {
            for block in &root.children {
                append_mdast_block(&mut txn, XmlContainer::Fragment(&frag_ref), block, &defs);
            }
        }
    }
    if frag_ref.len(&txn) == 0 {
        let idx = frag_ref.len(&txn);
        frag_ref.insert(&mut txn, idx, XmlElementPrelim::empty("paragraph"));
    }
    txn.encode_state_as_update_v1(&StateVector::default())
}

#[derive(Clone, Default)]
struct SeedMarkState {
    bold: bool,
    italic: bool,
    strike: bool,
    code: bool,
    link: Option<(String, Option<String>)>,
}

impl SeedMarkState {
    fn is_empty(&self) -> bool {
        !self.bold && !self.italic && !self.strike && !self.code && self.link.is_none()
    }

    fn to_attr_map(&self) -> std::collections::HashMap<std::sync::Arc<str>, Any> {
        let mut map = std::collections::HashMap::new();
        if self.bold {
            map.insert(
                std::sync::Arc::from("bold"),
                Any::from(std::collections::HashMap::<String, Any>::new()),
            );
        }
        if self.italic {
            map.insert(
                std::sync::Arc::from("italic"),
                Any::from(std::collections::HashMap::<String, Any>::new()),
            );
        }
        if self.strike {
            map.insert(
                std::sync::Arc::from("strike"),
                Any::from(std::collections::HashMap::<String, Any>::new()),
            );
        }
        if self.code {
            map.insert(
                std::sync::Arc::from("code"),
                Any::from(std::collections::HashMap::<String, Any>::new()),
            );
        }
        if let Some((href, title)) = &self.link {
            let mut m = std::collections::HashMap::new();
            m.insert("href".into(), Any::String(href.clone().into()));
            if let Some(t) = title {
                m.insert("title".into(), Any::String(t.clone().into()));
            }
            map.insert(std::sync::Arc::from("link"), Any::from(m));
        }
        map
    }
}

enum InlineSegment {
    Text {
        text: String,
        marks: SeedMarkState,
    },
    Break,
    Image {
        url: String,
        alt: String,
        title: Option<String>,
        link: Option<(String, Option<String>)>,
    },
    RawHtml(String),
}

fn collect_mdast_inlines(
    nodes: &[Node],
    marks: &SeedMarkState,
    defs: &RefDefMap,
    out: &mut Vec<InlineSegment>,
) {
    for n in nodes {
        match n {
            Node::Text(t) => {
                if !t.value.is_empty() {
                    out.push(InlineSegment::Text {
                        text: t.value.clone(),
                        marks: marks.clone(),
                    });
                }
            }
            Node::InlineCode(c) => {
                let mut m = marks.clone();
                m.code = true;
                out.push(InlineSegment::Text {
                    text: c.value.clone(),
                    marks: m,
                });
            }
            Node::Strong(s) => {
                let mut m = marks.clone();
                m.bold = true;
                collect_mdast_inlines(&s.children, &m, defs, out);
            }
            Node::Emphasis(e) => {
                let mut m = marks.clone();
                m.italic = true;
                collect_mdast_inlines(&e.children, &m, defs, out);
            }
            Node::Delete(d) => {
                let mut m = marks.clone();
                m.strike = true;
                collect_mdast_inlines(&d.children, &m, defs, out);
            }
            Node::Link(l) => {
                let mut m = marks.clone();
                m.link = Some((l.url.clone(), l.title.clone()));
                collect_mdast_inlines(&l.children, &m, defs, out);
            }
            Node::LinkReference(lr) => {
                let key = normalize_ref_id(&lr.identifier);
                if let Some((url, title)) = defs.get(&key) {
                    let mut m = marks.clone();
                    m.link = Some((url.clone(), title.clone()));
                    collect_mdast_inlines(&lr.children, &m, defs, out);
                } else {
                    out.push(InlineSegment::Text {
                        text: "[".into(),
                        marks: marks.clone(),
                    });
                    collect_mdast_inlines(&lr.children, marks, defs, out);
                    out.push(InlineSegment::Text {
                        text: "]".into(),
                        marks: marks.clone(),
                    });
                }
            }
            Node::Break(_) => out.push(InlineSegment::Break),
            Node::Image(img) => out.push(InlineSegment::Image {
                url: img.url.clone(),
                alt: img.alt.clone(),
                title: img.title.clone(),
                link: marks.link.clone(),
            }),
            Node::ImageReference(ir) => {
                let key = normalize_ref_id(&ir.identifier);
                if let Some((url, title)) = defs.get(&key) {
                    out.push(InlineSegment::Image {
                        url: url.clone(),
                        alt: ir.alt.clone(),
                        title: title.clone(),
                        link: marks.link.clone(),
                    });
                } else {
                    out.push(InlineSegment::Text {
                        text: format!("![{}]", ir.alt),
                        marks: marks.clone(),
                    });
                }
            }
            Node::Html(h) => out.push(InlineSegment::RawHtml(h.value.clone())),
            _ => {
                if let Some(children) = n.children() {
                    collect_mdast_inlines(children, marks, defs, out);
                }
            }
        }
    }
}

fn fill_inline_container(
    txn: &mut yrs::TransactionMut<'_>,
    parent: &XmlElementRef,
    children: &[Node],
    defs: &RefDefMap,
) {
    let mut segs = Vec::new();
    collect_mdast_inlines(children, &SeedMarkState::default(), defs, &mut segs);

    // Group adjacent text spans into single XmlText nodes, inserting ALL plain characters first
    // before formatting ranges so Yrs never inherits formatting attributes onto trailing runs.
    let mut run_buf: Vec<(String, SeedMarkState)> = Vec::new();
    let flush_text_run = |txn: &mut yrs::TransactionMut<'_>,
                          parent: &XmlElementRef,
                          buf: &mut Vec<(String, SeedMarkState)>| {
        if buf.is_empty() {
            return;
        }
        let idx = parent.len(txn);
        let txt = parent.insert(txn, idx, XmlTextPrelim::new(""));
        let mut combined = String::new();
        for (s, _) in buf.iter() {
            combined.push_str(s);
        }
        txt.insert(txn, 0, &combined);
        let mut offset = 0u32;
        for (s, marks) in buf.drain(..) {
            // Measure length via UTF-8/doc encoding? Wait: Yrs Doc uses default OffsetKind::Bytes cleanly
            // if we measure via Byte/UTF16 or sub-doc! Wait: yrs Doc::new() defaults to OffsetKind::Bytes (len in bytes).
            let step = s.len() as u32;
            if !marks.is_empty() && step > 0 {
                txt.format(txn, offset, step, marks.to_attr_map());
            }
            offset += step;
        }
    };

    for seg in segs {
        match seg {
            InlineSegment::Text { text, marks } => run_buf.push((text, marks)),
            InlineSegment::Break => {
                flush_text_run(txn, parent, &mut run_buf);
                let idx = parent.len(txn);
                parent.insert(txn, idx, XmlElementPrelim::empty("hardBreak"));
            }
            InlineSegment::Image {
                url,
                alt,
                title,
                link,
            } => {
                flush_text_run(txn, parent, &mut run_buf);
                let idx = parent.len(txn);
                let el = parent.insert(txn, idx, XmlElementPrelim::empty("image"));
                el.insert_attribute(txn, "src", url);
                if !alt.is_empty() {
                    el.insert_attribute(txn, "alt", alt);
                }
                if let Some(t) = title {
                    el.insert_attribute(txn, "title", t);
                }
                if let Some((href, link_title)) = link {
                    el.insert_attribute(txn, "linkHref", href);
                    if let Some(t) = link_title {
                        el.insert_attribute(txn, "linkTitle", t);
                    }
                }
            }
            InlineSegment::RawHtml(raw) => {
                flush_text_run(txn, parent, &mut run_buf);
                let idx = parent.len(txn);
                let el = parent.insert(txn, idx, XmlElementPrelim::empty("rawInlineHtml"));
                el.insert_attribute(txn, "raw", raw);
            }
        }
    }
    flush_text_run(txn, parent, &mut run_buf);
}

#[derive(Clone, Copy)]
enum XmlContainer<'a> {
    Fragment(&'a yrs::XmlFragmentRef),
    Element(&'a XmlElementRef),
}

impl<'a> XmlContainer<'a> {
    fn len(self, txn: &yrs::TransactionMut<'_>) -> u32 {
        match self {
            Self::Fragment(f) => f.len(txn),
            Self::Element(e) => e.len(txn),
        }
    }

    fn insert_elem(self, txn: &mut yrs::TransactionMut<'_>, idx: u32, tag: &str) -> XmlElementRef {
        match self {
            Self::Fragment(f) => f.insert(txn, idx, XmlElementPrelim::empty(tag)),
            Self::Element(e) => e.insert(txn, idx, XmlElementPrelim::empty(tag)),
        }
    }
}

fn encode_table_align(align: &[AlignKind]) -> Option<String> {
    if align.is_empty() || align.iter().all(|a| matches!(a, AlignKind::None)) {
        return None;
    }
    Some(
        align
            .iter()
            .map(|a| match a {
                AlignKind::Left => "left",
                AlignKind::Center => "center",
                AlignKind::Right => "right",
                AlignKind::None => "none",
            })
            .collect::<Vec<_>>()
            .join(","),
    )
}

fn append_mdast_block(
    txn: &mut yrs::TransactionMut<'_>,
    parent: XmlContainer<'_>,
    node: &Node,
    defs: &RefDefMap,
) {
    let idx = parent.len(txn);
    match node {
        Node::Paragraph(p) => {
            let el = parent.insert_elem(txn, idx, "paragraph");
            fill_inline_container(txn, &el, &p.children, defs);
        }
        Node::Heading(h) => {
            let el = parent.insert_elem(txn, idx, "heading");
            el.insert_attribute(txn, "level", Any::Number(yrs::Number::Int(h.depth as i64)));
            fill_inline_container(txn, &el, &h.children, defs);
        }
        Node::Code(c) => {
            let el = parent.insert_elem(txn, idx, "codeBlock");
            if let Some(lang) = &c.lang {
                if !lang.trim().is_empty() {
                    el.insert_attribute(txn, "language", lang.trim().to_owned());
                }
            }
            if let Some(meta) = &c.meta {
                el.insert_attribute(txn, "fenceMeta", meta.clone());
            }
            if !c.value.is_empty() {
                el.insert(txn, 0, XmlTextPrelim::new(&c.value));
            }
        }
        Node::Blockquote(b) => {
            let el = parent.insert_elem(txn, idx, "blockquote");
            for ch in &b.children {
                append_mdast_block(txn, XmlContainer::Element(&el), ch, defs);
            }
        }
        Node::List(l) => {
            let is_task = l
                .children
                .iter()
                .all(|c| matches!(c, Node::ListItem(li) if li.checked.is_some()));
            let list_tag = if is_task {
                "taskList"
            } else if l.ordered {
                "orderedList"
            } else {
                "bulletList"
            };
            let list_el = parent.insert_elem(txn, idx, list_tag);
            if l.ordered && !is_task {
                if let Some(start) = l.start {
                    if start > 1 {
                        list_el.insert_attribute(
                            txn,
                            "start",
                            Any::Number(yrs::Number::Int(start as i64)),
                        );
                    }
                }
            }
            for item_node in &l.children {
                if let Node::ListItem(li) = item_node {
                    let item_tag = if is_task { "taskItem" } else { "listItem" };
                    let item_idx = list_el.len(txn);
                    let item_el = list_el.insert(txn, item_idx, XmlElementPrelim::empty(item_tag));
                    if is_task {
                        item_el.insert_attribute(
                            txn,
                            "checked",
                            Any::Bool(li.checked.unwrap_or(false)),
                        );
                    }
                    for sub in &li.children {
                        append_mdast_block(txn, XmlContainer::Element(&item_el), sub, defs);
                    }
                }
            }
        }
        Node::Table(t) => {
            let tbl = parent.insert_elem(txn, idx, "table");
            if let Some(align_attr) = encode_table_align(&t.align) {
                tbl.insert_attribute(txn, "align", align_attr);
            }
            for (r_idx, row_node) in t.children.iter().enumerate() {
                if let Node::TableRow(tr) = row_node {
                    let row_idx = tbl.len(txn);
                    let row_el = tbl.insert(txn, row_idx, XmlElementPrelim::empty("tableRow"));
                    let cell_tag = if r_idx == 0 {
                        "tableHeader"
                    } else {
                        "tableCell"
                    };
                    for cell_node in &tr.children {
                        if let Node::TableCell(tc) = cell_node {
                            let c_idx = row_el.len(txn);
                            let c_el = row_el.insert(txn, c_idx, XmlElementPrelim::empty(cell_tag));
                            let p_el = c_el.insert(txn, 0, XmlElementPrelim::empty("paragraph"));
                            fill_inline_container(txn, &p_el, &tc.children, defs);
                        }
                    }
                }
            }
        }
        Node::ThematicBreak(_) => {
            parent.insert_elem(txn, idx, "horizontalRule");
        }
        Node::Html(h) => {
            let el = parent.insert_elem(txn, idx, "rawHtml");
            el.insert_attribute(txn, "raw", h.value.clone());
        }
        Node::Definition(_) => {}
        _ => {}
    }
}

fn attr_str<T: ReadTxn>(el: &XmlElementRef, txn: &T, key: &str) -> Option<String> {
    match el.get_attribute(txn, key) {
        Some(Out::Any(Any::String(s))) => Some(s.to_string()),
        Some(Out::Any(Any::Number(yrs::Number::Int(n)))) => Some(n.to_string()),
        Some(Out::Any(Any::Number(yrs::Number::Float(n)))) => Some((n as i64).to_string()),
        _ => None,
    }
}

fn attr_i64<T: ReadTxn>(el: &XmlElementRef, txn: &T, key: &str) -> Option<i64> {
    match el.get_attribute(txn, key) {
        Some(Out::Any(Any::Number(yrs::Number::Int(n)))) => Some(n),
        Some(Out::Any(Any::Number(yrs::Number::Float(n)))) => Some(n as i64),
        Some(Out::Any(Any::String(s))) => s.parse::<i64>().ok(),
        _ => None,
    }
}

fn attr_bool<T: ReadTxn>(el: &XmlElementRef, txn: &T, key: &str) -> Option<bool> {
    match el.get_attribute(txn, key) {
        Some(Out::Any(Any::Bool(b))) => Some(b),
        Some(Out::Any(Any::String(s))) => match s.as_ref() {
            "true" => Some(true),
            "false" => Some(false),
            _ => None,
        },
        _ => None,
    }
}

#[derive(Clone, Copy)]
struct InlineCtx {
    in_table: bool,
    escape_block_prefix: bool,
}

/// Escapes unmarked plain text so literal punctuation (`*`, `_`, `` ` ``, `[`, `]`,
/// `~`, `\`, `<`, `>`, `#`, `+`, `-`, numbered-list markers, and table pipes) stays literal
/// across CommonMark/GFM publish -> parse round-trips.
fn escape_markdown_plain(raw: &str, ctx: InlineCtx) -> String {
    let mut out = String::with_capacity(raw.len());
    for (line_idx, line) in raw.split('\n').enumerate() {
        if line_idx > 0 {
            out.push('\n');
        }
        let escape_start = ctx.escape_block_prefix || line_idx > 0;
        let bytes = line.as_bytes();
        let mut i = 0usize;
        if escape_start {
            // Preserve leading spaces, then check for ATX heading, quote, thematic/bullet/ordered prefix.
            while i < bytes.len() && bytes[i] == b' ' {
                out.push(' ');
                i += 1;
            }
            if i < bytes.len() {
                let b = bytes[i];
                if matches!(b, b'#' | b'>' | b'+' | b'-' | b'=') {
                    out.push('\\');
                    out.push(b as char);
                    i += 1;
                } else if b.is_ascii_digit() {
                    let start_digits = i;
                    while i < bytes.len() && bytes[i].is_ascii_digit() {
                        out.push(bytes[i] as char);
                        i += 1;
                    }
                    if i < bytes.len()
                        && matches!(bytes[i], b'.' | b')')
                        && (i > start_digits)
                        && (i + 1 >= bytes.len() || bytes[i + 1] == b' ' || bytes[i + 1] == b'\t')
                    {
                        out.push('\\');
                        out.push(bytes[i] as char);
                        i += 1;
                    }
                }
            }
        }
        for ch in line[i..].chars() {
            match ch {
                '\\' | '*' | '_' | '`' | '[' | ']' | '~' | '<' | '>' => {
                    out.push('\\');
                    out.push(ch);
                }
                '|' if ctx.in_table => {
                    out.push_str("\\|");
                }
                _ => out.push(ch),
            }
        }
    }
    out
}

fn format_inline_code_span(raw: &str, in_table: bool) -> String {
    let content = if in_table {
        raw.replace('|', "\\|")
    } else {
        raw.to_owned()
    };
    let mut max_run = 0usize;
    let mut cur = 0usize;
    for ch in content.chars() {
        if ch == '`' {
            cur += 1;
            max_run = max_run.max(cur);
        } else {
            cur = 0;
        }
    }
    let delim = "`".repeat(max_run + 1);
    let needs_pad = content.starts_with('`')
        || content.ends_with('`')
        || (content.starts_with(' ')
            && content.ends_with(' ')
            && content.chars().any(|c| c != ' '));
    if needs_pad {
        format!("{delim} {content} {delim}")
    } else {
        format!("{delim}{content}{delim}")
    }
}

fn format_fenced_code_block(info: &str, body: &str) -> String {
    let delimiter = if info.contains('`') { '~' } else { '`' };
    let mut max_run = 2usize;
    let mut cur = 0usize;
    for ch in body.chars() {
        if ch == delimiter {
            cur += 1;
            max_run = max_run.max(cur);
        } else {
            cur = 0;
        }
    }
    let fence = delimiter.to_string().repeat(max_run + 1);
    format!("{fence}{info}\n{body}\n{fence}")
}

fn escape_link_title(title: &str) -> String {
    title.replace('\\', "\\\\").replace('"', "\\\"")
}

fn escape_alt_text(alt: &str) -> String {
    alt.replace('\\', "\\\\")
        .replace('[', "\\[")
        .replace(']', "\\]")
}

fn render_image<T: ReadTxn>(txn: &T, el: &XmlElementRef) -> String {
    fn target(url: &str, title: &str) -> String {
        let destination = if url.contains([' ', '(', ')', '<', '>', '\\']) {
            format!(
                "<{}>",
                url.replace('\\', "\\\\")
                    .replace('<', "\\<")
                    .replace('>', "\\>")
            )
        } else {
            url.to_owned()
        };
        if title.is_empty() {
            destination
        } else {
            format!("{destination} \"{}\"", escape_link_title(title))
        }
    }

    let src = attr_str(el, txn, "src").unwrap_or_default();
    let alt = escape_alt_text(&attr_str(el, txn, "alt").unwrap_or_default());
    let title = attr_str(el, txn, "title").unwrap_or_default();
    let image = format!("![{alt}]({})", target(&src, &title));
    if let Some(href) = attr_str(el, txn, "linkHref").filter(|href| !href.is_empty()) {
        let title = attr_str(el, txn, "linkTitle").unwrap_or_default();
        format!("[{image}]({})", target(&href, &title))
    } else {
        image
    }
}

fn escape_link_label_unmarked(raw: &str, ctx: InlineCtx) -> String {
    // Link label sits inside `[ ... ](url)`, so use inline escaping (already escapes `[`, `]`, `*`, etc.).
    escape_markdown_plain(raw, ctx)
}

fn render_inline_children<T: ReadTxn>(
    txn: &T,
    children: impl Iterator<Item = XmlOut>,
    mut ctx: InlineCtx,
) -> String {
    let mut out = String::new();
    for (idx, child) in children.enumerate() {
        let child_ctx = InlineCtx {
            in_table: ctx.in_table,
            escape_block_prefix: ctx.escape_block_prefix && idx == 0,
        };
        match child {
            XmlOut::Text(txt) => out.push_str(&render_xml_text(txn, &txt, child_ctx)),
            XmlOut::Element(el) => {
                let tag = el.tag().to_ascii_lowercase();
                match tag.as_str() {
                    "hardbreak" | "hard_break" => {
                        if ctx.in_table {
                            out.push(' ');
                        } else {
                            out.push_str("  \n");
                        }
                    }
                    "image" => out.push_str(&render_image(txn, &el)),
                    "rawinlinehtml" => {
                        if let Some(raw) = attr_str(&el, txn, "raw") {
                            out.push_str(&raw);
                        }
                    }
                    "internalreference" => {
                        let label = escape_markdown_plain(
                            &attr_str(&el, txn, "label").unwrap_or_default(),
                            InlineCtx {
                                in_table: ctx.in_table,
                                escape_block_prefix: false,
                            },
                        );
                        let href = attr_str(&el, txn, "href").unwrap_or_default();
                        out.push_str(&format!("[{label}]({href})"));
                    }
                    _ => out.push_str(&render_inline_children(txn, el.children(txn), child_ctx)),
                }
            }
            XmlOut::Fragment(frag) => {
                out.push_str(&render_inline_children(txn, frag.children(txn), child_ctx));
            }
        }
        ctx.escape_block_prefix = false;
    }
    out
}

fn render_xml_text<T: ReadTxn>(txn: &T, txt: &XmlTextRef, ctx: InlineCtx) -> String {
    let chunks = txt.diff(txn, YChange::identity);
    let mut out = String::new();
    for (idx, diff) in chunks.into_iter().enumerate() {
        let piece_ctx = InlineCtx {
            in_table: ctx.in_table,
            escape_block_prefix: ctx.escape_block_prefix && idx == 0,
        };
        let piece = match diff.insert {
            Out::Any(Any::String(s)) => s.to_string(),
            Out::YXmlElement(el) => {
                let tag = el.tag().to_ascii_lowercase();
                if tag == "hardbreak" || tag == "hard_break" {
                    out.push_str(if piece_ctx.in_table { " " } else { "  \n" });
                    continue;
                } else {
                    out.push_str(&render_inline_children(
                        txn,
                        std::iter::once(XmlOut::Element(el)),
                        piece_ctx,
                    ));
                    continue;
                }
            }
            _ => String::new(),
        };
        if piece.is_empty() {
            continue;
        }
        let Some(attrs) = diff.attributes else {
            out.push_str(&escape_markdown_plain(&piece, piece_ctx));
            continue;
        };
        let is_code = attrs.get("code").is_some_and(|v| !matches!(v, Any::Null));
        let is_bold = attrs.get("bold").is_some_and(|v| !matches!(v, Any::Null));
        let is_italic = attrs.get("italic").is_some_and(|v| !matches!(v, Any::Null));
        let is_strike = attrs.get("strike").is_some_and(|v| !matches!(v, Any::Null));
        let link = attrs.get("link").and_then(|val| match val {
            Any::Map(m) => {
                let href = m.get("href").and_then(|v| match v {
                    Any::String(s) => Some(s.to_string()),
                    _ => None,
                })?;
                let title = m.get("title").and_then(|v| match v {
                    Any::String(s) => Some(s.to_string()),
                    _ => None,
                });
                Some((href, title))
            }
            _ => None,
        });

        let escaped_piece = if is_code {
            format_inline_code_span(&piece, piece_ctx.in_table)
        } else {
            escape_link_label_unmarked(
                &piece,
                InlineCtx {
                    in_table: piece_ctx.in_table,
                    escape_block_prefix: piece_ctx.escape_block_prefix
                        && !is_bold
                        && !is_italic
                        && !is_strike
                        && link.is_none(),
                },
            )
        };
        let mut formatted = escaped_piece;
        if is_italic {
            formatted = format!("*{formatted}*");
        }
        if is_bold {
            formatted = format!("**{formatted}**");
        }
        if is_strike {
            formatted = format!("~~{formatted}~~");
        }
        if let Some((href, title)) = link {
            match title {
                Some(t) if !t.is_empty() => {
                    formatted = format!("[{formatted}]({href} \"{}\")", escape_link_title(&t));
                }
                _ => {
                    formatted = format!("[{formatted}]({href})");
                }
            }
        }
        out.push_str(&formatted);
    }
    out
}

fn render_block<T: ReadTxn>(txn: &T, node: &XmlOut, indent_spaces: usize) -> Option<String> {
    let XmlOut::Element(el) = node else {
        return match node {
            XmlOut::Text(txt) => Some(render_xml_text(
                txn,
                txt,
                InlineCtx {
                    in_table: false,
                    escape_block_prefix: true,
                },
            )),
            XmlOut::Fragment(frag) => {
                let parts: Vec<String> = frag
                    .children(txn)
                    .filter_map(|c| render_block(txn, &c, indent_spaces))
                    .collect();
                Some(parts.join("\n\n"))
            }
            _ => None,
        };
    };

    let tag = el.tag().to_ascii_lowercase();
    match tag.as_str() {
        "paragraph" => {
            let text = render_inline_children(
                txn,
                el.children(txn),
                InlineCtx {
                    in_table: false,
                    escape_block_prefix: true,
                },
            );
            Some(text)
        }
        "heading" => {
            let level = attr_i64(el, txn, "level").unwrap_or(1).clamp(1, 6) as usize;
            let text = render_inline_children(
                txn,
                el.children(txn),
                InlineCtx {
                    in_table: false,
                    escape_block_prefix: false,
                },
            );
            Some(format!("{} {}", "#".repeat(level), text.trim()))
        }
        "codeblock" | "code_block" => {
            let lang = attr_str(el, txn, "language").unwrap_or_default();
            let meta = attr_str(el, txn, "fenceMeta").unwrap_or_default();
            let info = format!("{} {}", lang.trim(), meta.trim());
            let mut body = String::new();
            for child in el.children(txn) {
                match child {
                    XmlOut::Text(txt) => body.push_str(&txt.get_string(txn)),
                    _ => body.push_str(&render_raw_plain(txn, &child)),
                }
            }
            Some(format_fenced_code_block(info.trim(), &body))
        }
        "blockquote" => {
            let inner: Vec<String> = el
                .children(txn)
                .filter_map(|c| render_block(txn, &c, 0))
                .collect();
            let joined = inner.join("\n\n");
            let quoted = joined
                .lines()
                .map(|l| {
                    if l.is_empty() {
                        ">".into()
                    } else {
                        format!("> {l}")
                    }
                })
                .collect::<Vec<_>>()
                .join("\n");
            Some(quoted)
        }
        "bulletlist" | "bullet_list" => Some(render_list(txn, el, ListKind::Bullet, indent_spaces)),
        "orderedlist" | "ordered_list" => {
            let start = attr_i64(el, txn, "start").unwrap_or(1).max(1) as usize;
            Some(render_list(
                txn,
                el,
                ListKind::Ordered(start),
                indent_spaces,
            ))
        }
        "tasklist" | "task_list" => Some(render_list(txn, el, ListKind::Task, indent_spaces)),
        "horizontalrule" | "horizontal_rule" => Some("---".into()),
        "table" => Some(render_table(txn, el)),
        "rawhtml" => attr_str(el, txn, "raw"),
        "image" => Some(render_image(txn, el)),
        _ => {
            let inner: Vec<String> = el
                .children(txn)
                .filter_map(|c| render_block(txn, &c, indent_spaces))
                .collect();
            if inner.is_empty() {
                None
            } else {
                Some(inner.join("\n\n"))
            }
        }
    }
}

fn render_raw_plain<T: ReadTxn>(txn: &T, node: &XmlOut) -> String {
    match node {
        XmlOut::Text(txt) => txt.get_string(txn),
        XmlOut::Element(el) => el
            .children(txn)
            .map(|c| render_raw_plain(txn, &c))
            .collect(),
        XmlOut::Fragment(frag) => frag
            .children(txn)
            .map(|c| render_raw_plain(txn, &c))
            .collect(),
    }
}

#[derive(Clone, Copy)]
enum ListKind {
    Bullet,
    Ordered(usize),
    Task,
}

fn render_list<T: ReadTxn>(
    txn: &T,
    list_el: &XmlElementRef,
    kind: ListKind,
    indent_spaces: usize,
) -> String {
    let pad = " ".repeat(indent_spaces);
    let mut lines = Vec::new();
    let mut idx = match kind {
        ListKind::Ordered(s) => s,
        _ => 1,
    };
    for child in list_el.children(txn) {
        let XmlOut::Element(item_el) = child else {
            continue;
        };
        let marker = match kind {
            ListKind::Bullet => "- ".to_owned(),
            ListKind::Ordered(_) => {
                let m = format!("{idx}. ");
                idx += 1;
                m
            }
            ListKind::Task => {
                if attr_bool(&item_el, txn, "checked").unwrap_or(false) {
                    "- [x] ".to_owned()
                } else {
                    "- [ ] ".to_owned()
                }
            }
        };
        let child_indent = indent_spaces + marker.len();
        let mut item_blocks = Vec::new();
        for sub in item_el.children(txn) {
            let is_nested_list = matches!(
                &sub,
                XmlOut::Element(se)
                    if matches!(
                        se.tag().to_ascii_lowercase().as_str(),
                        "bulletlist" | "bullet_list" | "orderedlist" | "ordered_list" | "tasklist" | "task_list"
                    )
            );
            if is_nested_list {
                if let Some(sub_rendered) = render_block(txn, &sub, child_indent) {
                    item_blocks.push((true, sub_rendered));
                }
            } else if let Some(sub_rendered) = render_block(txn, &sub, indent_spaces) {
                item_blocks.push((false, sub_rendered));
            }
        }
        if item_blocks.is_empty() {
            lines.push(format!("{pad}{}", marker.trim_end()));
            continue;
        }
        for (i, (is_nested, text)) in item_blocks.into_iter().enumerate() {
            if i == 0 && !is_nested {
                let first_line_pad = " ".repeat(marker.len());
                let mut sub_lines = text.lines();
                if let Some(first) = sub_lines.next() {
                    lines.push(format!("{pad}{marker}{first}"));
                    for rest in sub_lines {
                        lines.push(format!("{pad}{first_line_pad}{rest}"));
                    }
                }
            } else {
                lines.push(text);
            }
        }
    }
    lines.join("\n")
}

fn render_table<T: ReadTxn>(txn: &T, table_el: &XmlElementRef) -> String {
    let mut rows: Vec<Vec<String>> = Vec::new();
    for child in table_el.children(txn) {
        let XmlOut::Element(row_el) = child else {
            continue;
        };
        let tag = row_el.tag().to_ascii_lowercase();
        if tag == "tablerow" || tag == "table_row" {
            rows.push(collect_table_row_cells(txn, &row_el));
        } else if matches!(tag.as_str(), "thead" | "tbody" | "tablehead" | "tablebody") {
            for r in row_el.children(txn) {
                if let XmlOut::Element(inner_row) = r {
                    rows.push(collect_table_row_cells(txn, &inner_row));
                }
            }
        }
    }
    if rows.is_empty() {
        return String::new();
    }
    let cols = rows.iter().map(|r| r.len()).max().unwrap_or(1).max(1);
    let align_tokens: Vec<&str> = attr_str(table_el, txn, "align")
        .map(|s| {
            s.split(',')
                .map(|part| match part.trim() {
                    "left" => ":---",
                    "center" => ":---:",
                    "right" => "---:",
                    _ => "---",
                })
                .collect()
        })
        .unwrap_or_default();
    let mut out = Vec::new();
    for (r_idx, row) in rows.into_iter().enumerate() {
        let mut padded = row;
        padded.resize(cols, String::new());
        out.push(format!("| {} |", padded.join(" | ")));
        if r_idx == 0 {
            let sep = (0..cols)
                .map(|c_idx| *align_tokens.get(c_idx).unwrap_or(&"---"))
                .collect::<Vec<_>>()
                .join(" | ");
            out.push(format!("| {sep} |"));
        }
    }
    out.join("\n")
}

fn render_table_cell_block<T: ReadTxn>(txn: &T, node: &XmlOut) -> Option<String> {
    match node {
        XmlOut::Element(el) => {
            let tag = el.tag().to_ascii_lowercase();
            if tag == "paragraph" || tag == "heading" {
                Some(render_inline_children(
                    txn,
                    el.children(txn),
                    InlineCtx {
                        in_table: true,
                        escape_block_prefix: false,
                    },
                ))
            } else {
                let parts: Vec<String> = el
                    .children(txn)
                    .filter_map(|c| render_table_cell_block(txn, &c))
                    .collect();
                Some(parts.join(" "))
            }
        }
        XmlOut::Text(txt) => Some(render_xml_text(
            txn,
            txt,
            InlineCtx {
                in_table: true,
                escape_block_prefix: false,
            },
        )),
        XmlOut::Fragment(frag) => {
            let parts: Vec<String> = frag
                .children(txn)
                .filter_map(|c| render_table_cell_block(txn, &c))
                .collect();
            Some(parts.join(" "))
        }
    }
}

fn collect_table_row_cells<T: ReadTxn>(txn: &T, row_el: &XmlElementRef) -> Vec<String> {
    let mut cells = Vec::new();
    for cell_node in row_el.children(txn) {
        let XmlOut::Element(cell_el) = cell_node else {
            continue;
        };
        let parts: Vec<String> = cell_el
            .children(txn)
            .filter_map(|c| render_table_cell_block(txn, &c))
            .collect();
        let text = parts.join(" ").replace('\n', " ").trim().to_owned();
        cells.push(text);
    }
    cells
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared_content::sanitize_page_content;
    use yrs::{types::xml::XmlIn, XmlTextPrelim};

    // Real binary state produced by @tiptap/extension-collaboration@3.32.2 + yjs@13.6.33:
    const TIPTAP_YJS_PROTOTYPE_B64: &str = "AValgcLKCQAEAQV0aXRsZQpUaXRsZSBIZXJlBwEHZGVmYXVsdAMHaGVhZGluZwcApYHCygkKBgQApYHCygkLClRpdGxlIEhlcmUoAKWBwsoJCgVsZXZlbAF9AYelgcLKCQoDCXBhcmFncmFwaAcApYHCygkXBgQApYHCygkYD1BhcmFncmFwaCB3aXRoIIalgcLKCScEYm9sZAJ7fYSlgcLKCSgEYm9sZIalgcLKCSwEYm9sZARudWxshKWBwsoJLQIsIIalgcLKCS8GaXRhbGljAnt9hKWBwsoJMAZpdGFsaWOGpYHCygk2Bml0YWxpYwRudWxshKWBwsoJNwIsIIalgcLKCTkEY29kZQJ7fYSlgcLKCToEY29kZYalgcLKCT4EY29kZQRudWxshKWBwsoJPwYsIGFuZCCGpYHCyglFBGxpbmt0eyJocmVmIjoiaHR0cHM6Ly9leGFtcGxlLm9yZy9zcGVjIiwidGFyZ2V0IjoiX2JsYW5rIiwicmVsIjoibm9vcGVuZXIgbm9yZWZlcnJlciBub2ZvbGxvdyIsImNsYXNzIjpudWxsLCJ0aXRsZSI6bnVsbH2EpYHCyglGBFNwZWOGpYHCyglKBGxpbmsEbnVsbISlgcLKCUsBLoelgcLKCRcDCmJ1bGxldExpc3QHAKWBwsoJTQMIbGlzdEl0ZW0HAKWBwsoJTgMJcGFyYWdyYXBoBwClgcLKCU8GBAClgcLKCVAIQnVsbGV0IEGHpYHCyglOAwhsaXN0SXRlbQcApYHCyglZAwlwYXJhZ3JhcGgHAKWBwsoJWgYEAKWBwsoJWwhCdWxsZXQgQoelgcLKCU0DC29yZGVyZWRMaXN0BwClgcLKCWQDCGxpc3RJdGVtBwClgcLKCWUDCXBhcmFncmFwaAcApYHCyglmBgQApYHCyglnCFN0ZXAgT25lh6WBwsoJZQMIbGlzdEl0ZW0HAKWBwsoJcAMJcGFyYWdyYXBoBwClgcLKCXEGBAClgcLKCXIIU3RlcCBUd28oAKWBwsoJZAVzdGFydAF9AYelgcLKCWQDCHRhc2tMaXN0BwClgcLKCXwDCHRhc2tJdGVtBwClgcLKCX0DCXBhcmFncmFwaAcApYHCygl+BgQApYHCygl/CURvbmUgdGFzaygApYHCygl9B2NoZWNrZWQBeIelgcLKCX0DCHRhc2tJdGVtBwClgcLKCYoBAwlwYXJhZ3JhcGgHAKWBwsoJiwEGBAClgcLKCYwBCU9wZW4gdGFzaygApYHCygmKAQdjaGVja2VkAXmHpYHCygl8AwV0YWJsZQcApYHCygmXAQMIdGFibGVSb3cHAKWBwsoJmAEDC3RhYmxlSGVhZGVyBwClgcLKCZkBAwlwYXJhZ3JhcGgHAKWBwsoJmgEGBAClgcLKCZsBBk1ldHJpYygApYHCygmZAQdjb2xzcGFuAX0BKAClgcLKCZkBB3Jvd3NwYW4BfQGHpYHCygmZAQMLdGFibGVIZWFkZXIHAKWBwsoJpAEDCXBhcmFncmFwaAcApYHCygmlAQYEAKWBwsoJpgEFVmFsdWUoAKWBwsoJpAEHY29sc3BhbgF9ASgApYHCygmkAQdyb3dzcGFuAX0Bh6WBwsoJmAEDCHRhYmxlUm93BwClgcLKCa4BAwl0YWJsZUNlbGwHAKWBwsoJrwEDCXBhcmFncmFwaAcApYHCygmwAQYEAKWBwsoJsQEHTGF0ZW5jeSgApYHCygmvAQdjb2xzcGFuAX0BKAClgcLKCa8BB3Jvd3NwYW4BfQGHpYHCygmvAQMJdGFibGVDZWxsBwClgcLKCbsBAwlwYXJhZ3JhcGgHAKWBwsoJvAEGBAClgcLKCb0BBDEybXMoAKWBwsoJuwEHY29sc3BhbgF9ASgApYHCygm7AQdyb3dzcGFuAX0Bh6WBwsoJlwEDCWNvZGVCbG9jawcApYHCygnEAQYEAKWBwsoJxQEVZm4gb2soKSAtPiBpMzIgeyA0MiB9KAClgcLKCcQBCGxhbmd1YWdlAXcEcnVzdIelgcLKCcQBAwlwYXJhZ3JhcGgA";

    #[test]
    fn tiptap_yjs_update_decodes_in_yrs_and_roundtrips_markdown_safely() {
        let initial_bytes = decode_b64(TIPTAP_YJS_PROTOTYPE_B64).unwrap();
        let merged = merge_yjs_updates(None, &[&initial_bytes]).unwrap();
        let (title, md) = extract_title_and_markdown(&merged).unwrap();
        assert_eq!(title, "Title Here");
        assert!(md.contains("# Title Here"));
        assert!(md.contains(
            "Paragraph with **bold**, *italic*, `code`, and [Spec](https://example.org/spec)."
        ));
        assert!(md.contains("- Bullet A\n- Bullet B"));
        assert!(md.contains("1. Step One\n2. Step Two"));
        assert!(md.contains("- [x] Done task\n- [ ] Open task"));
        assert!(md.contains("| Metric | Value |\n| --- | --- |\n| Latency | 12ms |"));
        assert!(md.contains("```rust\nfn ok() -> i32 { 42 }\n```"));

        // Seed equivalence verification matches rich formatting cleanly:
        assert!(verify_seed_matches_revision(&merged, "Title Here", &md).is_ok());

        // Must pass CW2 canonical Markdown validation cleanly:
        let sanitized = sanitize_page_content(&md).unwrap();
        assert!(sanitized.contains("https://example.org/spec"));

        // Reconcile from an empty client state vector reproduces full state cleanly:
        let diff = diff_from_state_vector(&merged, &[]).unwrap();
        let (_, md_from_diff) = extract_title_and_markdown(&diff).unwrap();
        assert_eq!(md, md_from_diff);

        // Unchanged re-seed round-trips semantically identical AST:
        let reseeded = seed_basic_yjs_update("Title Here", &sanitized);
        let (_, roundtripped) = extract_title_and_markdown(&reseeded).unwrap();
        assert!(
            markdown_ast_equivalent(&sanitized, &roundtripped).unwrap(),
            "Roundtrip mismatch!\nSAN:\n{sanitized}\nROUND:\n{roundtripped}"
        );
    }

    #[test]
    fn yrs_merges_concurrent_updates_commutatively_and_rejects_unsafe_links() {
        let initial = decode_b64(TIPTAP_YJS_PROTOTYPE_B64).unwrap();

        let doc_a = Doc::with_client_id(101);
        let title_a = doc_a.get_or_insert_text("title");
        {
            let mut txn = doc_a.transact_mut();
            txn.apply_update(Update::decode_v1(&initial).unwrap())
                .unwrap();
            title_a.push(&mut txn, " (Edited)");
        }
        let update_a = doc_a.transact().encode_state_as_update_v1(
            &StateVector::decode_v1(&state_vector_bytes(&initial).unwrap()).unwrap(),
        );

        let doc_b = Doc::with_client_id(202);
        let frag_b = doc_b.get_or_insert_xml_fragment("default");
        {
            let mut txn = doc_b.transact_mut();
            txn.apply_update(Update::decode_v1(&initial).unwrap())
                .unwrap();
            let p = yrs::XmlElementPrelim::new(
                "paragraph",
                [XmlIn::Text(XmlTextPrelim::new("Peer B line").into())],
            );
            let len = frag_b.len(&txn);
            frag_b.insert(&mut txn, len, p);
        }
        let update_b = doc_b.transact().encode_state_as_update_v1(
            &StateVector::decode_v1(&state_vector_bytes(&initial).unwrap()).unwrap(),
        );

        let ab = merge_yjs_updates(Some(&initial), &[&update_a, &update_b]).unwrap();
        let ba = merge_yjs_updates(Some(&initial), &[&update_b, &update_a]).unwrap();
        assert_eq!(
            extract_title_and_markdown(&ab).unwrap(),
            extract_title_and_markdown(&ba).unwrap()
        );
        let (merged_title, merged_md) = extract_title_and_markdown(&ab).unwrap();
        assert_eq!(merged_title, "Title Here (Edited)");
        assert!(merged_md.contains("Peer B line"));

        // Unsafe link mark inside Yjs XML fails canonical Markdown sanitization:
        let unsafe_doc = Doc::with_client_id(303);
        let unsafe_frag = unsafe_doc.get_or_insert_xml_fragment("default");
        {
            let mut txn = unsafe_doc.transact_mut();
            let p = unsafe_frag.insert(&mut txn, 0, XmlElementPrelim::empty("paragraph"));
            let txt = p.insert(&mut txn, 0, XmlTextPrelim::new(""));
            let mut link_map = std::collections::HashMap::new();
            link_map.insert("href".into(), Any::String("file:///etc/passwd".into()));
            let mut attrs = std::collections::HashMap::new();
            attrs.insert(std::sync::Arc::from("link"), Any::from(link_map));
            txt.insert_with_attributes(&mut txn, 0, "Secret", attrs);
        }
        let unsafe_bytes = unsafe_doc
            .transact()
            .encode_state_as_update_v1(&StateVector::default());
        let (_, unsafe_md) = extract_title_and_markdown(&unsafe_bytes).unwrap();
        assert_eq!(unsafe_md, "[Secret](file:///etc/passwd)");
        assert!(sanitize_page_content(&unsafe_md).is_err());

        // Literal `[Secret](file:///etc/passwd)` without a link mark is escaped and stays literal text!
        let literal_doc = Doc::with_client_id(304);
        let literal_frag = literal_doc.get_or_insert_xml_fragment("default");
        {
            let mut txn = literal_doc.transact_mut();
            let p = yrs::XmlElementPrelim::new(
                "paragraph",
                [XmlIn::Text(
                    XmlTextPrelim::new("Literal [Secret](https://safe.example)").into(),
                )],
            );
            literal_frag.insert(&mut txn, 0, p);
        }
        let literal_bytes = literal_doc
            .transact()
            .encode_state_as_update_v1(&StateVector::default());
        let (_, literal_md) = extract_title_and_markdown(&literal_bytes).unwrap();
        assert!(literal_md.contains("\\[Secret\\]"));
        let ast = to_mdast(&literal_md, &ParseOptions::gfm()).unwrap();
        assert!(!format!("{ast:?}").contains("Link("));
    }

    #[test]
    fn literal_punctuation_code_fences_links_tables_and_nested_lists_roundtrip_without_semantic_drift(
    ) {
        let doc = Doc::with_client_id(505);
        let title_ref = doc.get_or_insert_text("title");
        let frag = doc.get_or_insert_xml_fragment("default");
        {
            let mut txn = doc.transact_mut();
            title_ref.insert(&mut txn, 0, "Escaping *Spec*");

            // 1. Literal punctuation paragraph (must NOT become italic/bold/code/link/heading/list):
            let p1 = frag.insert(&mut txn, 0, XmlElementPrelim::empty("paragraph"));
            let t1 = p1.insert(&mut txn, 0, XmlTextPrelim::new(""));
            t1.insert(
                &mut txn,
                0,
                "Literal *stars*, _underscores_, `backticks`, [brackets](not-link), and \\backslash",
            );

            // 2. Paragraph starting with block syntax must remain paragraph on reopen:
            let p2 = frag.insert(&mut txn, 1, XmlElementPrelim::empty("paragraph"));
            let t2 = p2.insert(&mut txn, 0, XmlTextPrelim::new(""));
            t2.insert(&mut txn, 0, "# Literal hash and 1. not ordered");

            // 3. Inline code containing backticks + surrounding backticks + link with brackets in text:
            let p3 = frag.insert(&mut txn, 2, XmlElementPrelim::empty("paragraph"));
            let t3 = p3.insert(&mut txn, 0, XmlTextPrelim::new(""));
            t3.insert(&mut txn, 0, "`a``b` and Docs [v2]");
            let mut code_attrs = std::collections::HashMap::new();
            code_attrs.insert(
                std::sync::Arc::from("code"),
                Any::from(std::collections::HashMap::<String, Any>::new()),
            );
            t3.format(&mut txn, 0, 6, code_attrs);
            let mut link_map = std::collections::HashMap::new();
            link_map.insert(
                "href".into(),
                Any::String("https://example.org/docs".into()),
            );
            let mut link_attrs = std::collections::HashMap::new();
            link_attrs.insert(std::sync::Arc::from("link"), Any::from(link_map));
            t3.format(&mut txn, 11, 9, link_attrs);

            // 4. Fenced code block containing 4-backtick fences:
            let cb = frag.insert(&mut txn, 3, XmlElementPrelim::empty("codeBlock"));
            cb.insert_attribute(&mut txn, "language", "markdown");
            cb.insert(
                &mut txn,
                0,
                XmlTextPrelim::new("````text\ninner block\n````\n```\nalso backticks\n```"),
            );

            // 5. Table with pipe characters in cell text & inline code:
            let tbl = frag.insert(&mut txn, 4, XmlElementPrelim::empty("table"));
            let r0 = tbl.insert(&mut txn, 0, XmlElementPrelim::empty("tableRow"));
            for hdr in ["Col | A", "Col B"] {
                let h_idx = r0.len(&txn);
                let h = r0.insert(&mut txn, h_idx, XmlElementPrelim::empty("tableHeader"));
                let hp = h.insert(&mut txn, 0, XmlElementPrelim::empty("paragraph"));
                hp.insert(&mut txn, 0, XmlTextPrelim::new(hdr));
            }
            let r1 = tbl.insert(&mut txn, 1, XmlElementPrelim::empty("tableRow"));
            let c0 = r1.insert(&mut txn, 0, XmlElementPrelim::empty("tableCell"));
            let c0p = c0.insert(&mut txn, 0, XmlElementPrelim::empty("paragraph"));
            c0p.insert(&mut txn, 0, XmlTextPrelim::new("a | b *literal*"));
            let c1 = r1.insert(&mut txn, 1, XmlElementPrelim::empty("tableCell"));
            let c1p = c1.insert(&mut txn, 0, XmlElementPrelim::empty("paragraph"));
            let c1t = c1p.insert(&mut txn, 0, XmlTextPrelim::new(""));
            let mut code_attrs2 = std::collections::HashMap::new();
            code_attrs2.insert(
                std::sync::Arc::from("code"),
                Any::from(std::collections::HashMap::<String, Any>::new()),
            );
            c1t.insert(&mut txn, 0, "x|y");
            let c1_len = c1t.len(&txn);
            c1t.format(&mut txn, 0, c1_len, code_attrs2);

            // 6. Nested ordered -> bullet -> task list:
            let ol = frag.insert(&mut txn, 5, XmlElementPrelim::empty("orderedList"));
            let oi = ol.insert(&mut txn, 0, XmlElementPrelim::empty("listItem"));
            let op = oi.insert(&mut txn, 0, XmlElementPrelim::empty("paragraph"));
            op.insert(&mut txn, 0, XmlTextPrelim::new("Parent Step"));
            let nested_bl = oi.insert(&mut txn, 1, XmlElementPrelim::empty("bulletList"));
            let bi = nested_bl.insert(&mut txn, 0, XmlElementPrelim::empty("listItem"));
            let bp = bi.insert(&mut txn, 0, XmlElementPrelim::empty("paragraph"));
            bp.insert(&mut txn, 0, XmlTextPrelim::new("Child Bullet"));
        }

        let state = doc
            .transact()
            .encode_state_as_update_v1(&StateVector::default());
        let (title, md) = extract_title_and_markdown(&state).unwrap();
        assert_eq!(title, "Escaping *Spec*");
        // Verify fence used >= 5 backticks cleanly:
        assert!(md.contains("`````markdown"), "Actual md:\n{md}");
        assert!(
            md.contains("``` `a``b` ```"),
            "Expected spaced 3-backtick code span, actual md:\n{md}"
        );
        assert!(md.contains("[Docs \\[v2\\]](https://example.org/docs)"));
        // Verify literal pipe escaped and nested bullet aligned under "1. " (3 spaces):
        assert!(md.contains("a \\| b \\*literal\\*"));
        assert!(md.contains("1. Parent Step\n   - Child Bullet"));

        // Sanitize + re-seed into Yrs + extract again: semantic AST MUST be identical!
        let sanitized = sanitize_page_content(&md).unwrap();
        let reseeded = seed_basic_yjs_update(&title, &sanitized);
        let (_, md2) = extract_title_and_markdown(&reseeded).unwrap();
        assert!(
            markdown_ast_equivalent(&sanitized, &md2).unwrap(),
            "Semantic mismatch after round-trip:\n--- ORIGINAL ---\n{sanitized}\n--- ROUNDTRIPPED ---\n{md2}"
        );
    }

    #[test]
    fn seed_verification_rejects_ordered_start_and_reference_destination_changes() {
        let published_list = "7. Seventh step\n8. Eighth step";
        let changed_list_seed =
            seed_basic_yjs_update("List Page", "1. Seventh step\n2. Eighth step");
        let list_err =
            verify_seed_matches_revision(&changed_list_seed, "List Page", published_list)
                .expect_err("restarting ordered list from 7 to 1 must fail seed verification");
        assert!(
            list_err.contains("does not match"),
            "unexpected error: {list_err}"
        );

        let same_list_seed = seed_basic_yjs_update("List Page", published_list);
        verify_seed_matches_revision(&same_list_seed, "List Page", published_list).unwrap();

        let published_ref =
            "Open the [runbook][doc].\n\n[doc]: https://example.org/runbook \"Runbook Title\"";
        let altered_dest_seed = seed_basic_yjs_update(
            "Ref Page",
            "Open the [runbook](https://example.org/wrong \"Runbook Title\").",
        );
        let ref_err = verify_seed_matches_revision(&altered_dest_seed, "Ref Page", published_ref)
            .expect_err("changing reference link destination must fail seed verification");
        assert!(
            ref_err.contains("does not match"),
            "unexpected error: {ref_err}"
        );

        let plain_text_seed = seed_basic_yjs_update("Ref Page", "Open the runbook.");
        assert!(
            verify_seed_matches_revision(&plain_text_seed, "Ref Page", published_ref).is_err(),
            "dropping link from reference-style Markdown must fail seed verification"
        );

        let equivalent_inline_seed = seed_basic_yjs_update(
            "Ref Page",
            "Open the [runbook](https://example.org/runbook \"Runbook Title\").",
        );
        verify_seed_matches_revision(&equivalent_inline_seed, "Ref Page", published_ref).unwrap();

        let aligned_published = "| Left | Center | Right |\n| :--- | :---: | ---: |\n| a | b | c |";
        let unaligned_seed = seed_basic_yjs_update(
            "Table",
            "| Left | Center | Right |\n| --- | --- | --- |\n| a | b | c |",
        );
        assert!(verify_seed_matches_revision(&unaligned_seed, "Table", aligned_published).is_err());
        let aligned_seed = seed_basic_yjs_update("Table", aligned_published);
        verify_seed_matches_revision(&aligned_seed, "Table", aligned_published).unwrap();
    }

    #[test]
    fn seed_preserves_code_body_and_fence_metadata_exactly() {
        let published = "```text title=Original\nalpha\n\n```";
        for changed in [
            "```text title=Original\nalpha\n```",
            "```text title=Changed\nalpha\n\n```",
        ] {
            let seed = seed_basic_yjs_update("Code", changed);
            assert!(verify_seed_matches_revision(&seed, "Code", published).is_err());
        }
        let seed = seed_basic_yjs_update("Code", published);
        assert_eq!(extract_title_and_markdown(&seed).unwrap().1, published);
        verify_seed_matches_revision(&seed, "Code", published).unwrap();
    }

    #[test]
    fn seed_preserves_full_collapsed_and_shortcut_reference_links_and_images() {
        let md = concat!(
            "Read [the full runbook][doc], [collapsed][], [shortcut], and ![diagram][pic].\n\n",
            "[doc]: https://example.org/full \"Full Title\"\n",
            "[collapsed]: https://example.org/collapsed\n",
            "[shortcut]: https://example.org/shortcut\n",
            "[pic]: https://example.org/diagram.png \"Diagram Alt\"\n\n",
            "[![preview][pic]][target]\n\n",
            "[target]: https://example.org/details \"Open detail\""
        );
        let seeded = seed_basic_yjs_update("Refs", md);
        let (title, extracted) = extract_title_and_markdown(&seeded).unwrap();
        assert_eq!(title, "Refs");
        assert!(extracted.contains("https://example.org/full"));
        assert!(extracted.contains("Full Title"));
        assert!(extracted.contains("https://example.org/collapsed"));
        assert!(extracted.contains("https://example.org/shortcut"));
        assert!(extracted.contains("![diagram](https://example.org/diagram.png \"Diagram Alt\")"));
        assert!(extracted.contains(
            "[![preview](https://example.org/diagram.png \"Diagram Alt\")](https://example.org/details \"Open detail\")"
        ));
        assert!(markdown_ast_equivalent(md, &extracted).unwrap());
    }
}
