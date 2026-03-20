use crate::constants::{TAG_CODE_BLOCK, TAG_HEADER, TAG_IMAGE, TAG_ITEM, TAG_LIST, TAG_P};
use crate::models::Result;
use std::{collections::HashMap, fs, path::Path};
use toypeg::{Node, TPTag};

/// {{ key }} を HashMap の値で置換するテンプレートエンジン
pub struct HtmlRenderer;

impl HtmlRenderer {
    pub fn render_template<P: AsRef<Path>>(
        template_path: P,
        vars: &HashMap<&str, &str>,
    ) -> Result<String> {
        let mut html = fs::read_to_string(template_path)?;
        for (key, value) in vars {
            let pattern = format!("{{{{ {} }}}}", key);
            html = html.replace(&pattern, value);
        }
        Ok(html)
    }

    pub fn convert_ast_to_html(node: &Node) -> String {
        match node.tag {
            TPTag::Tag(TAG_HEADER) => {
                let level = Self::get_child_token(node, "#Level").len();
                let text = Self::get_child_token(node, "#Text");
                format!("<h{level}>{}</h{level}>\n", Self::render_inline(&text))
            }
            TPTag::Tag(TAG_P) => {
                let text: String = node
                    .nodes
                    .iter()
                    .filter_map(|n| n.borrow().token.as_ref().map(|t| t.to_string()))
                    .collect();
                let clean = text.trim();
                if clean.is_empty() {
                    String::new()
                } else {
                    let rendered = Self::render_inline(clean);
                    format!("<p>{}</p>\n", rendered.replace('\n', "<br>\n"))
                }
            }
            TPTag::Tag(TAG_CODE_BLOCK) => {
                let lang = Self::get_child_token(node, "#Lang").trim().to_string();
                let code = Self::get_child_token(node, "#Code");
                format!("<pre><code class=\"language-{lang}\">{code}</code></pre>\n")
            }
            TPTag::Tag(TAG_LIST) => {
                let items: String = node
                    .nodes
                    .iter()
                    .map(|n| Self::convert_ast_to_html(&n.borrow()))
                    .collect();
                format!("<ul>\n{items}</ul>\n")
            }
            TPTag::Tag(TAG_ITEM) => {
                let text = Self::get_child_token(node, "#Text");
                format!("  <li>{}</li>\n", Self::render_inline(text.trim()))
            }
            TPTag::Tag(TAG_IMAGE) => {
                let url = Self::get_child_token(node, "#Url");
                format!("<img src=\"{url}\" alt=\"\" loading=\"lazy\">\n")
            }
            // コンテナノード（#Doc, #Bodyなど）
            _ => node
                .nodes
                .iter()
                .map(|n| Self::convert_ast_to_html(&n.borrow()))
                .collect(),
        }
    }

    fn render_inline(input: &str) -> String {
        let mut out = input.to_string();

        Self::replace_pattern(&mut out, "**", "<b>", "</b>");
        Self::replace_pattern(&mut out, "~~", "<s>", "</s>");
        Self::replace_links(&mut out);
        Self::replace_footnote_refs(&mut out);

        out
    }

    fn replace_pattern(out: &mut String, delimiter: &str, open: &str, close: &str) {
        while let Some(start) = out.find(delimiter) {
            let rest = &out[start + delimiter.len()..];
            if let Some(end) = rest.find(delimiter) {
                let content_end = start + delimiter.len() + end;
                let content = &out[start + delimiter.len()..content_end];
                let replacement = format!("{}{}{}", open, content, close);
                out.replace_range(start..content_end + delimiter.len(), &replacement);
            } else {
                break;
            }
        }
    }

    fn replace_links(out: &mut String) {
        while let Some(start) = out.find('[') {
            if let Some(mid) = out[start..].find("](") {
                let mid_pos = start + mid;
                if let Some(end) = out[mid_pos..].find(')') {
                    let end_pos = mid_pos + end;
                    let text = &out[start + 1..mid_pos];
                    let url = &out[mid_pos + 2..end_pos];
                    out.replace_range(start..end_pos + 1, &format!("<a href=\"{url}\">{text}</a>"));
                    continue;
                }
            }
            break;
        }
    }

    fn replace_footnote_refs(out: &mut String) {
        let mut i = 0;
        while let Some(start) = out[i..].find("[^") {
            let start_pos = i + start;
            if let Some(end) = out[start_pos..].find(']') {
                let end_pos = start_pos + end;
                // 定義行 ( [^1]: ) は無視してリンクにしない
                if !out[end_pos + 1..].starts_with(": ") {
                    let id = &out[start_pos + 2..end_pos];
                    let link =
                        format!("<sup><a href=\"#fn-{id}\" id=\"fnref-{id}\">[{id}]</a></sup>");
                    out.replace_range(start_pos..end_pos + 1, &link);
                    i = start_pos + link.len();
                    continue;
                }
            }
            i = start_pos + 2;
        }
    }

    pub fn extract_footnotes(html: &str) -> (String, String) {
        let mut main_content = Vec::new();
        let mut notes = Vec::new();

        for line in html.lines() {
            if line.starts_with("<p>[^") && line.contains("]: ") {
                let trimmed = line.trim_start_matches("<p>").trim_end_matches("</p>");
                if let Some(idx) = trimmed.find("]: ") {
                    let id = &trimmed[2..idx];
                    let content = &trimmed[idx + 3..];
                    notes.push(format!(
                        r##"<li id="fn-{id}">{content} <a href="#fnref-{id}">↩</a></li>"##
                    ));
                    continue;
                }
            }
            main_content.push(line);
        }

        let footer = if notes.is_empty() {
            String::new()
        } else {
            format!(
                r#"<hr><section class="footnotes"><ol>{}</ol></section>"#,
                notes.concat()
            )
        };

        (main_content.join("\n"), footer)
    }

    fn get_child_token(node: &Node, tag_str: &'static str) -> String {
        node.nodes
            .iter()
            .find(|n| n.borrow().tag == TPTag::Tag(tag_str))
            .and_then(|n| n.borrow().token.as_ref().map(|t| t.to_string()))
            .unwrap_or_default()
    }
}
