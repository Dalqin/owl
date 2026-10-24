//! What a mod's author left for the people who install it: the installer's own
//! information (`fomod/info.xml`) and the readme-like files in the mod folder.
//!
//! These are facts about the folder, found by the scanner, and shown as text.
//! Nothing here is interpreted or acted on.

use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

use roxmltree::{Document, Node};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::fomod;
use crate::scan::{find_file_ci, is_engine_dir};

/// Most bytes read from one file.
const MAX_READ: u64 = 1 << 20;
/// Most bytes of text kept per document; the rest is dropped and flagged.
const MAX_TEXT: usize = 256 << 10;
/// Most documents kept; the ones nearest the top of the folder win.
const MAX_DOCUMENTS: usize = 50;

/// File names (lowercase prefixes) that mean "the author wrote this for you".
const NOTE_NAMES: &[&str] = &[
    "readme",
    "read me",
    "changelog",
    "changes",
    "history",
    "install",
    "instructions",
    "credits",
    "notes",
    "faq",
    "permissions",
    "license",
    "licence",
];

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ModNotes {
    /// From the installer's `info.xml`, if the mod has one.
    pub info: Option<ModInfo>,
    /// Readme-like files, nearest the top of the folder first.
    pub documents: Vec<NoteDocument>,
}

impl ModNotes {
    pub fn is_empty(&self) -> bool {
        self.info.is_none() && self.documents.is_empty()
    }
}

/// The installer's `fomod/info.xml`, all of it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ModInfo {
    pub name: Option<String>,
    pub author: Option<String>,
    pub version: Option<String>,
    pub website: Option<String>,
    /// The author's full description, as written.
    pub description: Option<String>,
    /// Any other field the file has, so nothing the author wrote is dropped.
    pub other: Vec<InfoField>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct InfoField {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct NoteDocument {
    /// Relative to the scanned folder.
    pub path: PathBuf,
    pub kind: DocumentKind,
    /// The text, for kinds that have any. HTML is turned into plain text.
    pub text: Option<String>,
    /// The file was longer than what is kept in `text`.
    pub truncated: bool,
    /// Why the file could not be read, if it could not.
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DocumentKind {
    Text,
    Markdown,
    Html,
    /// An internet shortcut; `text` is its address.
    Link,
    /// Something the author left that is not shown as text (PDF, Word, RTF).
    Other,
}

fn document_kind(file_name: &str) -> Option<DocumentKind> {
    let lower = file_name.to_lowercase();
    let path = Path::new(&lower);
    let ext = path.extension().and_then(|e| e.to_str());
    match ext {
        Some("txt") | Some("nfo") => Some(DocumentKind::Text),
        Some("md") | Some("markdown") => Some(DocumentKind::Markdown),
        Some("html") | Some("htm") => Some(DocumentKind::Html),
        Some("url") => Some(DocumentKind::Link),
        Some("pdf") | Some("rtf") | Some("doc") | Some("docx") | Some("odt") => {
            Some(DocumentKind::Other)
        }
        // An unknown extension is not a note (Notes.esp is a plugin), but a bare "readme" is.
        Some(_) => None,
        None => {
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            NOTE_NAMES
                .iter()
                .any(|n| stem.starts_with(n))
                .then_some(DocumentKind::Text)
        }
    }
}

/// Finds and reads what the author left in `root`, looking at most `max_depth`
/// folders down. Engine asset folders are never entered. Never fails: a file
/// that cannot be read is listed with the reason.
pub fn collect(root: &Path, max_depth: usize) -> ModNotes {
    let mut docs: Vec<(usize, PathBuf)> = Vec::new();
    let mut infos: Vec<(usize, PathBuf)> = Vec::new();
    walk(root, Path::new(""), 0, max_depth, &mut docs, &mut infos);

    // Nearest the top first, then by name, so the cap drops the deepest.
    docs.sort_by_key(|(depth, p)| (*depth, p.to_string_lossy().to_lowercase()));
    docs.truncate(MAX_DOCUMENTS);
    infos.sort_by_key(|(depth, p)| (*depth, p.to_string_lossy().to_lowercase()));

    ModNotes {
        info: infos
            .first()
            .and_then(|(_, rel)| fs::read(root.join(rel)).ok())
            .and_then(|bytes| parse_info(&fomod::decode(&bytes))),
        documents: docs.into_iter().map(|(_, rel)| read_document(root, rel)).collect(),
    }
}

fn walk(
    root: &Path,
    rel: &Path,
    depth: usize,
    max_depth: usize,
    docs: &mut Vec<(usize, PathBuf)>,
    infos: &mut Vec<(usize, PathBuf)>,
) {
    let Ok(read) = fs::read_dir(root.join(rel)) else {
        return;
    };
    for entry in read.flatten() {
        // Symlinks are not followed: a link must not lead the scan out of the mod folder.
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        let name = entry.file_name().to_string_lossy().into_owned();
        let lower = name.to_lowercase();
        if file_type.is_dir() {
            if is_engine_dir(&lower) || depth >= max_depth {
                continue;
            }
            let sub = rel.join(&name);
            if lower == "fomod" {
                if let Some(info) = find_file_ci(&entry.path(), "info.xml") {
                    if let Some(file) = info.file_name() {
                        infos.push((depth, sub.join(file)));
                    }
                }
            }
            walk(root, &sub, depth + 1, max_depth, docs, infos);
        } else if file_type.is_file() && document_kind(&name).is_some() {
            docs.push((depth, rel.join(&name)));
        }
    }
}

fn read_document(root: &Path, rel: PathBuf) -> NoteDocument {
    let kind = rel
        .file_name()
        .and_then(|n| n.to_str())
        .and_then(document_kind)
        .unwrap_or(DocumentKind::Other);
    let mut doc = NoteDocument {
        path: rel.clone(),
        kind,
        text: None,
        truncated: false,
        error: None,
    };
    if kind == DocumentKind::Other {
        return doc;
    }
    let mut bytes = Vec::new();
    let read = File::open(root.join(&rel)).and_then(|f| f.take(MAX_READ + 1).read_to_end(&mut bytes));
    if let Err(e) = read {
        doc.error = Some(e.to_string());
        return doc;
    }
    let too_long = bytes.len() as u64 > MAX_READ;
    bytes.truncate(MAX_READ as usize);

    let decoded = decode_text(&bytes);
    let text = match kind {
        DocumentKind::Html => html_to_text(&decoded),
        DocumentKind::Link => shortcut_address(&decoded).unwrap_or(decoded),
        _ => decoded,
    };
    let text = text.replace("\r\n", "\n").replace('\r', "\n");
    let (text, cut) = clip(text, MAX_TEXT);
    doc.truncated = too_long || cut;
    doc.text = Some(text);
    doc
}

/// Keeps at most `max` bytes, cutting at a character boundary.
fn clip(mut text: String, max: usize) -> (String, bool) {
    if text.len() <= max {
        return (text, false);
    }
    let mut end = max;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text.truncate(end);
    (text, true)
}

/// Windows-1252 for bytes 0x80..=0x9F; everything else is the same as Latin-1.
const CP1252: [char; 32] = [
    '\u{20AC}', '\u{0081}', '\u{201A}', '\u{0192}', '\u{201E}', '\u{2026}', '\u{2020}', '\u{2021}',
    '\u{02C6}', '\u{2030}', '\u{0160}', '\u{2039}', '\u{0152}', '\u{008D}', '\u{017D}', '\u{008F}',
    '\u{0090}', '\u{2018}', '\u{2019}', '\u{201C}', '\u{201D}', '\u{2022}', '\u{2013}', '\u{2014}',
    '\u{02DC}', '\u{2122}', '\u{0161}', '\u{203A}', '\u{0153}', '\u{009D}', '\u{017E}', '\u{0178}',
];

/// Turns the bytes of a readme into text. A byte-order mark decides when there
/// is one; otherwise UTF-16 without a mark is recognised by its zero bytes,
/// valid UTF-8 is taken as UTF-8, and anything else is Windows-1252, which is
/// what readmes from the Morrowind era are written in.
pub fn decode_text(bytes: &[u8]) -> String {
    if bytes.starts_with(&[0xFF, 0xFE]) || bytes.starts_with(&[0xFE, 0xFF]) || bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return fomod::decode(bytes);
    }
    // UTF-16 text in a mostly-ASCII language has a zero in every other byte. Zero
    // bytes are valid UTF-8, so this has to be checked first.
    let odd_zeros = bytes.iter().skip(1).step_by(2).filter(|b| **b == 0).count();
    if bytes.len() >= 4 && odd_zeros * 2 >= bytes.len() / 2 {
        let mut with_bom = vec![0xFF, 0xFE];
        with_bom.extend_from_slice(bytes);
        return fomod::decode(&with_bom);
    }
    if let Ok(text) = std::str::from_utf8(bytes) {
        return text.to_string();
    }
    bytes
        .iter()
        .map(|&b| match b {
            0x80..=0x9F => CP1252[(b - 0x80) as usize],
            _ => b as char,
        })
        .collect()
}

/// Plain text from an HTML page: scripts, styles and comments dropped, block
/// elements become line breaks, entities decoded. The result is never HTML.
pub fn html_to_text(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    // ASCII-only lowercasing keeps byte offsets the same as in `html`.
    let lower = html.to_ascii_lowercase();
    let mut i = 0;
    while i < html.len() {
        let rest = &html[i..];
        if rest.starts_with("<!--") {
            i += rest.find("-->").map_or(rest.len(), |n| n + 3);
        } else if rest.starts_with('<') {
            let Some(end) = rest.find('>') else {
                break;
            };
            let tag = lower[i + 1..i + end].trim_start_matches('/').trim();
            let name: String = tag.chars().take_while(|c| c.is_ascii_alphanumeric()).collect();
            let closing = lower[i + 1..].starts_with('/');
            if (name == "script" || name == "style") && !closing {
                let close = format!("</{name}");
                i += lower[i..].find(&close).map_or(rest.len(), |n| {
                    n + lower[i + n..].find('>').map_or(0, |m| m + 1)
                });
                continue;
            }
            match name.as_str() {
                "br" | "p" | "div" | "tr" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "ul" | "ol" | "table" | "pre" => {
                    out.push('\n')
                }
                "li" if !closing => out.push_str("\n- "),
                "td" | "th" if closing => out.push('\t'),
                _ => {}
            }
            i += end + 1;
        } else {
            let next = rest.find('<').unwrap_or(rest.len());
            out.push_str(&rest[..next]);
            i += next;
        }
    }
    tidy(&decode_entities(&out))
}

fn decode_entities(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        let decoded = rest.find(';').filter(|&n| n <= 10).and_then(|n| {
            let name = &rest[1..n];
            let ch = match name {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                "nbsp" => Some(' '),
                _ => name
                    .strip_prefix('#')
                    .and_then(|num| match num.strip_prefix(['x', 'X']) {
                        Some(hex) => u32::from_str_radix(hex, 16).ok(),
                        None => num.parse().ok(),
                    })
                    .and_then(char::from_u32),
            };
            ch.map(|c| (c, n + 1))
        });
        match decoded {
            Some((c, len)) => {
                out.push(c);
                rest = &rest[len..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// Trims each line and squeezes runs of blank lines to one.
fn tidy(text: &str) -> String {
    let mut out = Vec::new();
    let mut blank = false;
    for line in text.lines().map(str::trim_end) {
        if line.trim().is_empty() {
            blank = !out.is_empty();
        } else {
            if blank {
                out.push(String::new());
            }
            blank = false;
            out.push(line.to_string());
        }
    }
    out.join("\n")
}

/// The address in an internet shortcut (`.url`) file.
fn shortcut_address(text: &str) -> Option<String> {
    text.lines()
        .find_map(|l| l.trim().strip_prefix("URL=").map(|u| u.trim().to_string()))
}

/// Reads `fomod/info.xml`. `None` if it is not that kind of file.
pub fn parse_info(xml: &str) -> Option<ModInfo> {
    let doc = Document::parse(xml).ok()?;
    let root = doc.root_element();
    if !root.tag_name().name().eq_ignore_ascii_case("fomod") {
        return None;
    }
    let mut info = ModInfo::default();
    for field in root.children().filter(Node::is_element) {
        let name = field.tag_name().name();
        // A plain field keeps the author's own line breaks (only the ends are trimmed). A
        // field that holds other elements (a list of categories, say) is flattened to the
        // text and attribute values inside it, so nothing the author wrote is dropped.
        let value = if field.children().any(|c| c.is_element()) {
            field
                .descendants()
                .skip(1)
                .flat_map(|d| {
                    if d.is_element() {
                        d.attributes().map(|a| a.value().trim().to_string()).collect::<Vec<_>>()
                    } else {
                        d.text().map(|t| vec![t.trim().to_string()]).unwrap_or_default()
                    }
                })
                .filter(|v| !v.is_empty())
                .collect::<Vec<_>>()
                .join(", ")
        } else {
            field.text().unwrap_or("").replace("\r\n", "\n").trim().to_string()
        };
        let slot = match name.to_lowercase().as_str() {
            "name" => &mut info.name,
            "author" => &mut info.author,
            "version" => &mut info.version,
            "website" => &mut info.website,
            "description" => &mut info.description,
            _ => {
                if !value.is_empty() {
                    info.other.push(InfoField {
                        name: name.to_string(),
                        value,
                    });
                }
                continue;
            }
        };
        if !value.is_empty() {
            *slot = Some(value);
        }
    }
    Some(info)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> ModNotes {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/notes");
        collect(&dir, 6)
    }

    #[test]
    fn decodes_each_encoding_a_readme_can_be_in() {
        assert_eq!(decode_text("plain café".as_bytes()), "plain café");
        assert_eq!(decode_text(b"\xEF\xBB\xBFwith mark"), "with mark");
        let le: Vec<u8> = [0xFF, 0xFE].into_iter().chain("é x".encode_utf16().flat_map(u16::to_le_bytes)).collect();
        assert_eq!(decode_text(&le), "é x");
        let be: Vec<u8> = [0xFE, 0xFF].into_iter().chain("é x".encode_utf16().flat_map(u16::to_be_bytes)).collect();
        assert_eq!(decode_text(&be), "é x");
        // No mark at all: recognised by the zero bytes.
        let bare: Vec<u8> = "Hello there".encode_utf16().flat_map(u16::to_le_bytes).collect();
        assert_eq!(decode_text(&bare), "Hello there");
        // Windows-1252: é, curly quotes, en dash, euro.
        assert_eq!(decode_text(b"caf\xe9 \x93hi\x94 \x96 \x80"), "café “hi” – €");
    }

    #[test]
    fn html_becomes_plain_text_without_scripts_or_comments() {
        let html = "<html><head><style>p{}</style><script>var x='<b>';</script></head><body><h1>Title</h1><p>A &amp; B &lt;c&gt; &#233; &#x41;</p><ul><li>One</li><li>Two</li></ul><!-- gone --><br>End</body></html>";
        assert_eq!(
            html_to_text(html),
            "Title\n\nA & B <c> é A\n\n- One\n- Two\n\nEnd"
        );
    }

    #[test]
    fn an_unknown_entity_is_left_as_written() {
        assert_eq!(html_to_text("a &nosuch; b & c"), "a &nosuch; b & c");
    }

    #[test]
    fn reads_every_field_of_info_xml_including_utf16() {
        let notes = fixture();
        let info = notes.info.expect("fomod/info.xml is found");
        assert_eq!(info.name.as_deref(), Some("Notes Mod"));
        assert_eq!(info.author.as_deref(), Some("An Author"));
        assert_eq!(info.version.as_deref(), Some("1.2.3"));
        assert_eq!(info.website.as_deref(), Some("https://example.org/mods/notes"));
        let description = info.description.unwrap();
        assert!(description.starts_with("Notes Mod\n\nRequirements:\nMorrowind & Tribunal"), "{description:?}");
        assert!(description.ends_with("Known issues:\nNone."));
        // Fields we have no slot for are kept, not dropped.
        assert!(info.other.iter().any(|f| f.name == "Id" && f.value == "12345"));
        assert!(info.other.iter().any(|f| f.name == "Groups" && f.value == "Misc"));
    }

    #[test]
    fn finds_the_readmes_nearest_the_top_first_and_skips_asset_folders() {
        let notes = fixture();
        let paths: Vec<String> = notes
            .documents
            .iter()
            .map(|d| d.path.to_string_lossy().replace('\\', "/"))
            .collect();
        assert_eq!(paths, vec!["Link.url", "README.txt", "Docs/Install.html"]);
    }

    #[test]
    fn readme_text_is_decoded_and_html_is_flattened() {
        let notes = fixture();
        let get = |p: &str| {
            notes
                .documents
                .iter()
                .find(|d| d.path.ends_with(p))
                .unwrap_or_else(|| panic!("{p}"))
        };
        let readme = get("README.txt");
        assert_eq!(readme.kind, DocumentKind::Text);
        let text = readme.text.as_deref().unwrap();
        assert!(text.contains("Café patch for “Morrowind” – read this first."), "{text:?}");
        assert!(!text.contains('\r') && !readme.truncated && readme.error.is_none());

        let install = get("Install.html");
        assert_eq!(install.kind, DocumentKind::Html);
        let text = install.text.as_deref().unwrap();
        assert!(text.contains("Use Notes & friends.") && text.contains("- Two <2>"), "{text:?}");
        assert!(!text.contains("hidden") && !text.contains('<') || text.contains("<2>"));

        let link = get("Link.url");
        assert_eq!(link.kind, DocumentKind::Link);
        assert_eq!(link.text.as_deref(), Some("https://example.org/mods/notes"));
    }

    #[test]
    fn long_files_are_cut_and_say_so() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("readme.txt"), "x".repeat(MAX_TEXT + 100)).unwrap();
        let notes = collect(dir.path(), 3);
        let doc = &notes.documents[0];
        assert!(doc.truncated);
        assert_eq!(doc.text.as_ref().unwrap().len(), MAX_TEXT);
    }

    #[test]
    fn pdfs_are_listed_without_text() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("Manual.pdf"), b"%PDF-1.4").unwrap();
        let notes = collect(dir.path(), 3);
        assert_eq!(notes.documents.len(), 1);
        assert_eq!(notes.documents[0].kind, DocumentKind::Other);
        assert_eq!(notes.documents[0].text, None);
    }

    #[test]
    fn only_a_limited_number_of_documents_are_kept() {
        let dir = tempfile::tempdir().unwrap();
        for i in 0..MAX_DOCUMENTS + 10 {
            fs::write(dir.path().join(format!("readme{i:03}.txt")), "x").unwrap();
        }
        assert_eq!(collect(dir.path(), 3).documents.len(), MAX_DOCUMENTS);
    }

    #[test]
    fn a_folder_with_nothing_is_empty_and_a_missing_one_does_not_fail() {
        let dir = tempfile::tempdir().unwrap();
        assert!(collect(dir.path(), 3).is_empty());
        assert!(collect(&dir.path().join("nope"), 3).is_empty());
    }

    #[test]
    fn info_xml_that_is_not_fomod_is_ignored() {
        assert_eq!(parse_info("<other/>"), None);
        assert_eq!(parse_info("not xml"), None);
    }

    #[test]
    fn json_round_trip() {
        let n = fixture();
        let text = serde_json::to_string(&n).unwrap();
        assert_eq!(serde_json::from_str::<ModNotes>(&text).unwrap(), n);
    }
}
