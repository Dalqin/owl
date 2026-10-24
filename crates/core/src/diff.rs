//! A line diff of a whole file, trimmed to the changes and a few lines around
//! them, with each line's number in the old and the new file. Pure text in,
//! structured lines out, so any front end can draw it.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Unchanged lines kept on each side of a change.
const CONTEXT: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DiffKind {
    /// In both files.
    Context,
    /// Only in the new file.
    Added,
    /// Only in the old file.
    Removed,
    /// Unchanged lines left out between two groups of changes. Carries no text.
    Gap,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DiffLine {
    pub kind: DiffKind,
    /// 1-based line number in the current file; none for added lines and gaps.
    pub old_line: Option<u32>,
    /// 1-based line number in the file as it would be written; none for removed lines and gaps.
    pub new_line: Option<u32>,
    pub text: String,
}

/// Every line of both files, as context / removed / added.
fn full(old: &[&str], new: &[&str]) -> Vec<DiffLine> {
    let prefix = old.iter().zip(new).take_while(|(a, b)| a == b).count();
    let suffix = old[prefix..]
        .iter()
        .rev()
        .zip(new[prefix..].iter().rev())
        .take_while(|(a, b)| a == b)
        .count();
    let (o, n) = (
        &old[prefix..old.len() - suffix],
        &new[prefix..new.len() - suffix],
    );

    // Longest common subsequence of the middle, by table; config files are small.
    let w = n.len() + 1;
    let mut lcs = vec![0u32; (o.len() + 1) * w];
    for i in (0..o.len()).rev() {
        for j in (0..n.len()).rev() {
            lcs[i * w + j] = if o[i] == n[j] {
                lcs[(i + 1) * w + j + 1] + 1
            } else {
                lcs[(i + 1) * w + j].max(lcs[i * w + j + 1])
            };
        }
    }

    let line = |kind, old_line: Option<usize>, new_line: Option<usize>, text: &str| DiffLine {
        kind,
        old_line: old_line.map(|l| l as u32 + 1),
        new_line: new_line.map(|l| l as u32 + 1),
        text: text.to_string(),
    };
    let mut out = Vec::with_capacity(old.len().max(new.len()));
    for (k, text) in old.iter().take(prefix).enumerate() {
        out.push(line(DiffKind::Context, Some(k), Some(k), text));
    }
    let (mut i, mut j) = (0, 0);
    while i < o.len() || j < n.len() {
        if i < o.len() && j < n.len() && o[i] == n[j] {
            out.push(line(
                DiffKind::Context,
                Some(prefix + i),
                Some(prefix + j),
                o[i],
            ));
            i += 1;
            j += 1;
        } else if i < o.len() && (j == n.len() || lcs[(i + 1) * w + j] >= lcs[i * w + j + 1]) {
            out.push(line(DiffKind::Removed, Some(prefix + i), None, o[i]));
            i += 1;
        } else {
            out.push(line(DiffKind::Added, None, Some(prefix + j), n[j]));
            j += 1;
        }
    }
    for k in 0..suffix {
        let (a, b) = (prefix + o.len() + k, prefix + n.len() + k);
        out.push(line(DiffKind::Context, Some(a), Some(b), old[a]));
    }
    out
}

/// The changes between `old` and `new` with [`CONTEXT`] unchanged lines around
/// each, and a [`DiffKind::Gap`] wherever lines were left out. Empty when the
/// two are the same.
pub fn diff(old: &str, new: &str) -> Vec<DiffLine> {
    diff_with_anchors(old, new, &[])
}

/// [`diff`], but any line that equals one of `anchors` (ignoring surrounding
/// spaces) is always shown when there is a change to show, so the reader can
/// see which part of the file the changes are in.
pub fn diff_with_anchors(old: &str, new: &str, anchors: &[&str]) -> Vec<DiffLine> {
    let old: Vec<&str> = old.lines().collect();
    let new: Vec<&str> = new.lines().collect();
    let all = full(&old, &new);

    if all.iter().all(|l| l.kind == DiffKind::Context) {
        return Vec::new();
    }
    let mut keep = vec![false; all.len()];
    for (i, l) in all.iter().enumerate() {
        if anchors.contains(&l.text.trim()) {
            keep[i] = true;
        }
        if l.kind != DiffKind::Context {
            let from = i.saturating_sub(CONTEXT);
            let to = (i + CONTEXT + 1).min(all.len());
            keep[from..to].fill(true);
        }
    }

    let mut out = Vec::new();
    let mut skipping = false;
    for (l, keep) in all.into_iter().zip(keep) {
        if keep {
            skipping = false;
            out.push(l);
        } else if !skipping && !out.is_empty() {
            skipping = true;
            out.push(DiffLine {
                kind: DiffKind::Gap,
                old_line: None,
                new_line: None,
                text: String::new(),
            });
        }
    }
    // A gap after the last change has nothing to separate.
    if out.last().is_some_and(|l| l.kind == DiffKind::Gap) {
        out.pop();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(d: &[DiffLine]) -> Vec<char> {
        d.iter()
            .map(|l| match l.kind {
                DiffKind::Context => ' ',
                DiffKind::Added => '+',
                DiffKind::Removed => '-',
                DiffKind::Gap => '~',
            })
            .collect()
    }

    #[test]
    fn same_text_has_no_diff() {
        assert!(diff("a\nb\n", "a\nb\n").is_empty());
    }

    #[test]
    fn an_insert_shows_context_and_both_line_numbers() {
        let d = diff("a\nb\nc\n", "a\nb\nX\nc\n");
        assert_eq!(kinds(&d), [' ', ' ', '+', ' ']);
        assert_eq!((d[2].old_line, d[2].new_line), (None, Some(3)));
        assert_eq!(
            (d[3].old_line, d[3].new_line, d[3].text.as_str()),
            (Some(3), Some(4), "c")
        );
    }

    #[test]
    fn a_replacement_lists_the_removal_first() {
        let d = diff("a\nold\nz\n", "a\nnew\nz\n");
        assert_eq!(kinds(&d), [' ', '-', '+', ' ']);
        assert_eq!((d[1].old_line, d[1].new_line), (Some(2), None));
    }

    #[test]
    fn distant_changes_are_separated_by_a_gap_and_the_ends_are_trimmed() {
        let old: Vec<String> = (1..=30).map(|n| format!("l{n}")).collect();
        let mut new = old.clone();
        new.insert(1, "first".into());
        new.push("last".into());
        let d = diff(&old.join("\n"), &new.join("\n"));
        assert!(kinds(&d).contains(&'~'));
        assert_eq!(d.first().unwrap().text, "l1");
        assert_eq!(d.last().unwrap().text, "last");
        assert!(d.len() < 16, "{}", d.len());
    }

    #[test]
    fn anchors_are_always_shown_with_a_gap_between_them_and_the_change() {
        let old: Vec<String> = (1..=20).map(|n| format!("l{n}")).collect();
        let mut with_markers = vec!["[begin]".to_string()];
        with_markers.extend(old.clone());
        with_markers.push("[end]".to_string());
        let mut changed = with_markers.clone();
        changed.insert(11, "new".into());
        let d = diff_with_anchors(
            &with_markers.join("\n"),
            &changed.join("\n"),
            &["[begin]", "[end]"],
        );
        assert_eq!(d.first().unwrap().text, "[begin]");
        assert_eq!(d.last().unwrap().text, "[end]");
        assert!(d.iter().any(|l| l.kind == DiffKind::Gap));
        assert!(d.iter().any(|l| l.text == "new"));
        // No change, nothing to show, anchors or not.
        assert!(diff_with_anchors("[begin]\na\n", "[begin]\na\n", &["[begin]"]).is_empty());
    }

    #[test]
    fn a_new_file_is_all_additions() {
        let d = diff("", "a\nb\n");
        assert_eq!(kinds(&d), ['+', '+']);
    }
}
