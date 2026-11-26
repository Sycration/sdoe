//! Read-only result tables for display and export.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Table {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    /// Per-row tag for frontend actions (e.g. the model term of a row).
    pub tags: Vec<String>,
    /// Per-row emphasis: "" | good | bad | muted | strong
    pub emphasis: Vec<String>,
    /// Columns holding numbers (right-aligned).
    pub numeric: Vec<bool>,
}

impl Table {
    pub fn new(columns: &[&str], numeric: &[bool]) -> Table {
        Table {
            columns: columns.iter().map(|s| s.to_string()).collect(),
            numeric: numeric.to_vec(),
            ..Default::default()
        }
    }

    pub fn push(&mut self, row: Vec<String>, tag: &str, emphasis: &str) {
        self.rows.push(row);
        self.tags.push(tag.into());
        self.emphasis.push(emphasis.into());
    }

    /// HTML rendering for reports.
    pub fn to_html(&self) -> String {
        let esc = crate::report::esc;
        let mut s = String::from("<table><thead><tr>");
        for (i, c) in self.columns.iter().enumerate() {
            let cls = if self.numeric.get(i).copied().unwrap_or(false) { " class=\"n\"" } else { "" };
            s.push_str(&format!("<th{cls}>{}</th>", esc(c)));
        }
        s.push_str("</tr></thead><tbody>");
        for (r, row) in self.rows.iter().enumerate() {
            let em = self.emphasis.get(r).map(String::as_str).unwrap_or("");
            s.push_str(&format!("<tr class=\"{em}\">"));
            for (i, c) in row.iter().enumerate() {
                let cls = if self.numeric.get(i).copied().unwrap_or(false) { " class=\"n\"" } else { "" };
                s.push_str(&format!("<td{cls}>{}</td>", esc(c)));
            }
            s.push_str("</tr>");
        }
        s.push_str("</tbody></table>");
        s
    }
}
