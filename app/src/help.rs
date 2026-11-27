//! The user documentation (`docs/*.md`), compiled into the program for the
//! Help window.

use serde_json::{Value, json};

/// File name and Markdown text of each page, in the order of the contents.
const PAGES: [(&str, &str); 6] = [
    ("README.md", include_str!("../../docs/README.md")),
    ("setup-and-data.md", include_str!("../../docs/setup-and-data.md")),
    ("designs.md", include_str!("../../docs/designs.md")),
    ("models.md", include_str!("../../docs/models.md")),
    ("analysis.md", include_str!("../../docs/analysis.md")),
    ("optimisation.md", include_str!("../../docs/optimisation.md")),
];

/// JSON array of the pages: `{file, title, text, headings}`. `text` is the
/// Markdown with links to files outside the documentation reduced to their
/// text; `headings` lists `{level, text, slug}` with the heading text as
/// displayed and the anchor that links use.
pub fn pages_json() -> String {
    let pages: Vec<Value> = PAGES
        .iter()
        .map(|(file, md)| {
            let headings = headings(md);
            let title = headings.iter().find(|h| h.0 == 1).map(|h| h.1.clone()).unwrap_or_else(|| file.to_string());
            json!({
                "file": file,
                "title": title,
                "text": local_links(md),
                "headings": headings.iter().map(|(level, text)| json!({"level": level, "text": text, "slug": slug(text)})).collect::<Vec<_>>(),
            })
        })
        .collect();
    Value::Array(pages).to_string()
}

/// Lines of `md` outside fenced code blocks, with a flag for each line that
/// is outside.
fn prose_lines(md: &str) -> impl Iterator<Item = (bool, &str)> {
    let mut fenced = false;
    md.split_inclusive('\n').map(move |line| {
        if line.trim_start().starts_with("```") {
            fenced = !fenced;
            return (false, line);
        }
        (!fenced, line)
    })
}

/// Headings outside code blocks: level and displayed text.
fn headings(md: &str) -> Vec<(usize, String)> {
    prose_lines(md)
        .filter(|(prose, _)| *prose)
        .filter_map(|(_, line)| {
            let line = line.trim_end();
            let level = line.chars().take_while(|&c| c == '#').count();
            let rest = line.get(level..)?;
            (level > 0 && level <= 6 && rest.starts_with(' ')).then(|| (level, inline_text(rest.trim())))
        })
        .collect()
}

/// Text of inline Markdown as displayed: links reduced to their text,
/// code spans, emphasis markers and backslash escapes removed.
fn inline_text(s: &str) -> String {
    let s = replace_links(s, |text, _| Some(text.to_string()));
    let mut out = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                if let Some(n) = chars.next() {
                    out.push(n);
                }
            }
            '`' | '*' => {}
            _ => out.push(c),
        }
    }
    out
}

/// GitHub anchor of a heading: lower case, spaces as hyphens, other
/// characters than letters, digits, '-' and '_' removed.
fn slug(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .filter_map(|c| match c {
            ' ' => Some('-'),
            c if c.is_alphanumeric() || c == '-' || c == '_' => Some(c),
            _ => None,
        })
        .collect()
}

/// Replaces each `[text](target)` in `s` with the result of `f(text,
/// target)`; `None` keeps the link.
fn replace_links(s: &str, f: impl Fn(&str, &str) -> Option<String>) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(open) = rest.find('[') {
        let after = &rest[open + 1..];
        let link = after.find("](").and_then(|mid| {
            let target_start = mid + 2;
            let close = after[target_start..].find(')')?;
            let text = &after[..mid];
            let target = &after[target_start..target_start + close];
            (!text.contains('[') && !target.contains(' ')).then_some((text, target, target_start + close + 1))
        });
        match link {
            Some((text, target, len)) => {
                out.push_str(&rest[..open]);
                match f(text, target) {
                    Some(r) => out.push_str(&r),
                    None => out.push_str(&rest[open..open + 1 + len]),
                }
                rest = &after[len..];
            }
            None => {
                out.push_str(&rest[..open + 1]);
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// The Markdown with links to files that are not documentation pages (such
/// as `../API.md`) reduced to their text, without a leading `../`.
fn local_links(md: &str) -> String {
    let is_page = |target: &str| {
        let file = target.split('#').next().unwrap_or("");
        file.is_empty() || PAGES.iter().any(|(f, _)| *f == file) || target.contains("://")
    };
    prose_lines(md)
        .map(|(prose, line)| {
            if !prose {
                return line.to_string();
            }
            replace_links(line, |text, target| (!is_page(target)).then(|| text.trim_start_matches("../").to_string()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_outside_the_documentation_become_text() {
        let md = "See [API.md](../API.md#project-file-format) and [models](models.md#hierarchy).\n```\n[x](../y.md)\n```\n";
        assert_eq!(local_links(md), "See API.md and [models](models.md#hierarchy).\n```\n[x](../y.md)\n```\n");
    }

    #[test]
    fn headings_and_anchors_match_github() {
        let h = headings("# A `b` c\n```\n# not\n```\n## Environment × factor: [x](y.md)\n");
        assert_eq!(h, vec![(1, "A b c".to_string()), (2, "Environment × factor: x".to_string())]);
        assert_eq!(slug("Effects tab: screening"), "effects-tab-screening");
        assert_eq!(slug("Environment x factor interaction note"), "environment-x-factor-interaction-note");
    }

    #[test]
    fn every_link_between_pages_has_a_target() {
        let pages: Vec<Value> = serde_json::from_str(&pages_json()).unwrap();
        for p in &pages {
            let text = p["text"].as_str().unwrap();
            replace_links(text, |_, target| {
                if target.contains("://") {
                    return None;
                }
                let (file, anchor) = target.split_once('#').unwrap_or((target, ""));
                let file = if file.is_empty() { p["file"].as_str().unwrap() } else { file };
                let page = pages.iter().find(|q| q["file"] == file).unwrap_or_else(|| panic!("{}: no page {file}", p["file"]));
                if !anchor.is_empty() {
                    assert!(page["headings"].as_array().unwrap().iter().any(|h| h["slug"] == anchor), "{}: no heading #{anchor} in {file}", p["file"]);
                }
                None
            });
        }
    }
}
