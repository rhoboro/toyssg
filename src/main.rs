use std::{collections::HashMap, fs, path::Path};
use toypeg::{
    GrammarBuilder, MatchResult, Node, TPAny, TPChar, TPContext, TPExpr, TPMany, TPNode, TPNot,
    TPOneMany, TPOr, TPRange, TPSeq, TPTag,
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Debug)]
struct PostMeta {
    title: String,
    published_at: String,
    slug: String,
    tags: String,
    description: String,
}

impl PostMeta {
    fn from_map(mut map: HashMap<String, String>) -> Self {
        Self {
            title: map.remove("title").unwrap_or_default(),
            published_at: map.remove("published_at").unwrap_or_default(),
            slug: map.remove("slug").unwrap_or_default(),
            tags: map.remove("tags").unwrap_or_default(),
            description: map.remove("description").unwrap_or_default(),
        }
    }
}

#[derive(Debug)]
struct PostEntry {
    meta: PostMeta,
    file_name: String,
    content: String, // ここにはHTML変換後の文字列が入る
}

fn main() -> Result<()> {
    prepare_dist()?;

    // 1. 固定ページの処理
    let mut page_files = Vec::new();
    collect_md_files(Path::new("contents/pages"), &mut page_files)?;
    for page in page_files {
        let post = load_post(page.as_path())?;
        render_single_file(&post, "dist", "./")?;
    }

    // 2. ブログ投稿の処理
    let mut posts = Vec::new();
    let mut post_files = Vec::new();
    collect_md_files(Path::new("contents/posts"), &mut post_files)?;
    for post in post_files {
        posts.push(load_post(post.as_path())?);
    }
    posts.sort_by(|a, b| b.meta.published_at.cmp(&a.meta.published_at));
    render_blog_collection(&posts)?;

    Ok(())
}

fn load_post(path: &Path) -> Result<PostEntry> {
    let raw = fs::read_to_string(path)?;
    let ctx = parse_markdown_with_toypeg(&raw)?;
    let root = ctx.tree.borrow();

    let mut meta_map = HashMap::new();
    let mut _dummy_body = String::new();

    // メタデータの抽出
    extract_content(&root, &mut meta_map, &mut _dummy_body);

    // 本文のHTML変換 (ASTから直接生成)
    let html_content = find_and_render_body(&root);

    let meta = PostMeta::from_map(meta_map);
    let file_name = if meta.published_at.is_empty() {
        meta.slug.clone()
    } else {
        format!("{}-{}", meta.published_at, meta.slug)
    };

    Ok(PostEntry {
        file_name,
        content: html_content,
        meta,
    })
}

fn find_and_render_body(node: &Node) -> String {
    if node.tag == TPTag::Tag("#Body") {
        return ast_to_html(node);
    }
    for child in &node.nodes {
        let res = find_and_render_body(&child.borrow());
        if !res.is_empty() {
            return res;
        }
    }
    String::new()
}

fn render_single_file(post: &PostEntry, out_dir: &str, rel_path: &str) -> Result<()> {
    // 1. まず記事パーツ (post.html) をレンダリング
    let (clean_body, footer) = process_footnotes(&post.content);
    let final_content = format!("{}{}", clean_body, footer);

    let mut post_vars = HashMap::new();
    post_vars.insert("title", post.meta.title.as_str());
    post_vars.insert("published_at", post.meta.published_at.as_str());
    post_vars.insert("html_content", final_content.as_str());

    // タグのリンク HTML を Rust 側で組み立てる
    let mut tags_html = String::new();
    for t in post.meta.tags.split(',') {
        let t = t.trim();
        if !t.is_empty() {
            tags_html.push_str(&format!(
                r#"<a href="{}tags/{}.html">#{}</a> "#,
                rel_path, t, t
            ));
        }
    }
    post_vars.insert("tags_html", tags_html.as_str());

    let content_body = render_simple(Path::new("contents/templates/post.html"), &post_vars)?;

    // 2. base.html に埋め込む
    let mut base_vars = HashMap::new();
    base_vars.insert("title", post.meta.title.as_str());
    base_vars.insert("description", post.meta.description.as_str());
    base_vars.insert("og_type", "article");
    base_vars.insert("rel_path", rel_path);
    base_vars.insert("content", content_body.as_str());
    base_vars.insert("extra_head", r#"<link rel="stylesheet" href="https://cdnjs.cloudflare.com/ajax/libs/prism/1.29.0/themes/prism-tomorrow.min.css">"#);

    let final_html = render_simple(Path::new("contents/templates/base.html"), &base_vars)?;

    fs::write(format!("{}/{}.html", out_dir, post.file_name), final_html)?;
    Ok(())
}

fn render_blog_collection(posts: &[PostEntry]) -> Result<()> {
    let mut tags_map: HashMap<&str, Vec<&PostEntry>> = HashMap::new();

    // 1. 各個別記事のレンダリング
    for post in posts {
        render_single_file(post, "dist/posts", "../")?;
        for tag in post.meta.tags.split(',') {
            let t = tag.trim();
            if !t.is_empty() {
                tags_map.entry(t).or_default().push(post);
            }
        }
    }

    // 2. Index ページの生成
    let mut list_html = String::new();
    for post in posts {
        list_html.push_str(&format!(
            "<li><span>{}</span><a href='posts/{}.html'>{}</a></li>\n",
            post.meta.published_at, post.file_name, post.meta.title
        ));
    }

    // パーツ (index.html) の作成
    let mut idx_parts = HashMap::new();
    idx_parts.insert("posts", list_html.as_str());
    let index_body = render_simple(Path::new("contents/templates/index.html"), &idx_parts)?;

    // 全体 (base.html) への埋め込み
    let mut idx_base = HashMap::new();
    idx_base.insert("title", "Articles");
    idx_base.insert("description", "rhoboro's microblog記事一覧");
    idx_base.insert("og_type", "website");
    idx_base.insert("rel_path", "./");
    idx_base.insert("content", index_body.as_str());
    idx_base.insert("extra_head", "");

    let final_index = render_simple(Path::new("contents/templates/base.html"), &idx_base)?;
    fs::write("dist/index.html", final_index)?;

    // 3. Tag ページの生成
    for (tag, tag_posts) in tags_map {
        let mut t_list_html = String::new();
        for p in tag_posts {
            t_list_html.push_str(&format!(
                "<li><span>{}</span><a href='../posts/{}.html'>{}</a></li>\n",
                p.meta.published_at, p.file_name, p.meta.title
            ));
        }

        // パーツ (tag.html) の作成
        let mut t_parts = HashMap::new();
        t_parts.insert("posts", t_list_html.as_str());
        t_parts.insert("tag_name", tag);
        let tag_body = render_simple(Path::new("contents/templates/tag.html"), &t_parts)?;

        // 全体 (base.html) への埋め込み
        let desc = format!("Tag: {} の記事一覧", tag);
        let mut t_base = HashMap::new();
        t_base.insert("title", tag); // タグ名をタイトルに
        t_base.insert("description", &desc);
        t_base.insert("og_type", "website");
        t_base.insert("rel_path", "../");
        t_base.insert("content", tag_body.as_str());
        t_base.insert("extra_head", "");

        let final_tag = render_simple(Path::new("contents/templates/base.html"), &t_base)?;
        fs::write(format!("dist/tags/{}.html", tag), final_tag)?;
    }

    if Path::new("contents/static").exists() {
        copy_dir_all("contents/static", "dist")?;
    }
    Ok(())
}

/// toypegを使ってMarkdownファイルを(フロントマター, 本文)に分解する
fn parse_markdown_with_toypeg(input: &str) -> Result<TPContext> {
    let builder = GrammarBuilder::new();
    let g = builder.clone();

    let grammar = builder
        .insert_as_node(
            "MARKDOWN",
            TPSeq::builder()
                .seq(g.reference("FRONT_MATTER"))
                .seq(TPNode::new(
                    TPMany::new(g.reference("BLOCK_WRAPPER")),
                    TPTag::Tag("#Body"),
                ))
                .build(),
            TPTag::Tag("#Doc"),
        )
        // ... (FRONT_MATTER, YAML_ENTRY は前回同様) ...
        .insert_as_node(
            "FRONT_MATTER",
            TPSeq::builder()
                .seq(TPChar::new("---"))
                .seq(g.reference("NEWLINE"))
                .seq(TPMany::new(g.reference("YAML_ENTRY")))
                .seq(TPChar::new("---"))
                .seq(g.reference("NEWLINE"))
                .build(),
            TPTag::Tag("#FrontMatter"),
        )
        .insert_as_node(
            "YAML_ENTRY",
            TPSeq::builder()
                .seq(TPNode::new(g.reference("IDENTIFIER"), TPTag::Tag("#Key")))
                .seq(TPChar::new(": "))
                .seq(TPNode::new(g.reference("VALUE"), TPTag::Tag("#Value")))
                .seq(g.reference("NEWLINE"))
                .build(),
            TPTag::Tag("#Entry"),
        )
        .insert(
            "BLOCK_WRAPPER",
            TPSeq::builder()
                .seq(TPMany::new(g.reference("NEWLINE")))
                .seq(g.reference("BLOCK"))
                .seq(TPMany::new(g.reference("NEWLINE")))
                .build(),
        )
        // BLOCKの優先順位に LIST を追加
        .insert(
            "BLOCK",
            TPOr::builder()
                .or(g.reference("HEADER"))
                .or(g.reference("CODE_BLOCK"))
                .or(g.reference("IMAGE"))
                .or(g.reference("LIST"))
                .or(g.reference("PARAGRAPH"))
                .build()
                .unwrap(),
        )
        // リスト: 箇条書きアイテムの連続
        .insert_as_node(
            "LIST",
            TPOneMany::new(g.reference("LIST_ITEM")),
            TPTag::Tag("#List"),
        )
        // 箇条書きアイテム: - スペース 内容 \n
        .insert_as_node(
            "LIST_ITEM",
            TPSeq::builder()
                .seq(TPChar::new("- "))
                .seq(TPNode::new(g.reference("VALUE"), TPTag::Tag("#Text")))
                .seq(g.reference("NEWLINE"))
                .build(),
            TPTag::Tag("#Item"),
        )
        .insert_as_node(
            "IMAGE",
            TPSeq::builder()
                .seq(TPChar::new("![]("))
                .seq(TPNode::new(
                    TPMany::new(TPSeq::new(
                        TPNot::new(TPChar::new(")")),
                        g.reference("ANY_CHAR"),
                    )),
                    TPTag::Tag("#Url"),
                ))
                .seq(TPChar::new(")"))
                .seq(g.reference("NEWLINE"))
                .build(),
            TPTag::Tag("#Image"),
        )
        .insert_as_node(
            "HEADER",
            TPSeq::builder()
                .seq(TPNode::new(
                    TPOneMany::new(TPChar::new("#")),
                    TPTag::Tag("#Level"),
                ))
                .seq(TPChar::new(" "))
                .seq(TPNode::new(g.reference("VALUE"), TPTag::Tag("#Text")))
                .seq(g.reference("NEWLINE"))
                .build(),
            TPTag::Tag("#Header"),
        )
        .insert_as_node(
            "CODE_BLOCK",
            TPSeq::builder()
                .seq(TPChar::new("```"))
                .seq(TPNode::new(g.reference("VALUE"), TPTag::Tag("#Lang")))
                .seq(g.reference("NEWLINE"))
                .seq(TPNode::new(
                    TPMany::new(TPSeq::new(
                        TPNot::new(TPChar::new("```")),
                        g.reference("ANY_CHAR"),
                    )),
                    TPTag::Tag("#Code"),
                ))
                .seq(TPChar::new("```"))
                .seq(g.reference("NEWLINE"))
                .build(),
            TPTag::Tag("#CodeBlock"),
        )
        .insert_as_node(
            "PARAGRAPH",
            TPOneMany::new(g.reference("PLAIN_LINE")),
            TPTag::Tag("#P"),
        )
        .insert_as_node(
            "PLAIN_LINE",
            TPSeq::builder()
                .seq(TPNot::new(
                    TPOr::builder()
                        .or(g.reference("NEWLINE"))
                        .or(TPChar::new("#"))
                        .or(TPChar::new("```"))
                        .or(TPChar::new("![]("))
                        .or(TPChar::new("- ")) // リスト開始記号をガード
                        .build()
                        .unwrap(),
                ))
                .seq(g.reference("VALUE"))
                .seq(g.reference("NEWLINE"))
                .build(),
            TPTag::Tag("#Line"),
        )
        .insert(
            "IDENTIFIER",
            TPOneMany::new(TPOr::new(
                TPRange::new("abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789"),
                TPRange::new("_-"),
            )),
        )
        .insert(
            "VALUE",
            TPMany::new(TPSeq::new(
                TPNot::new(g.reference("NEWLINE")),
                g.reference("ANY_CHAR"),
            )),
        )
        .insert("ANY_CHAR", TPAny::new())
        .insert("NEWLINE", TPOr::new(TPChar::new("\r\n"), TPChar::new("\n")))
        .build();

    let mut raw = input.to_owned();
    if !raw.ends_with('\n') {
        raw.push('\n');
    }
    let context = TPContext::new(raw);
    let result = grammar.borrow().get("MARKDOWN").unwrap().matches(context);

    match result? {
        MatchResult::Success(c) => Ok(c),
        MatchResult::Failure(f) => panic!(
            "Parse failed at pos: {}\nNear: {:?}",
            f.pos,
            &f.text[f.pos..(f.pos + 40).min(f.text.len())]
        ),
    }
}

fn extract_content(node: &Node, meta: &mut HashMap<String, String>, body: &mut String) {
    match node.tag {
        TPTag::Tag("#Entry") => {
            let mut key = String::new();
            let mut value = String::new();
            for child_rc in &node.nodes {
                let child = child_rc.borrow();
                if let Some(ref token) = child.token {
                    match child.tag {
                        TPTag::Tag("#Key") => key = format!("{token}"),
                        TPTag::Tag("#Value") => value = format!("{token}").trim().to_string(),
                        _ => {}
                    }
                }
            }
            if !key.is_empty() {
                meta.insert(key, value);
            }
        }
        TPTag::Tag("#Body") => {
            if let Some(ref token) = node.token {
                *body = format!("{token}");
            }
        }
        _ => {
            for child_rc in &node.nodes {
                extract_content(&child_rc.borrow(), meta, body);
            }
        }
    }
}

fn ast_to_html(node: &Node) -> String {
    match node.tag {
        TPTag::Tag("#List") => {
            let mut items_html = String::new();
            for child_rc in &node.nodes {
                items_html.push_str(&ast_to_html(&child_rc.borrow()));
            }
            format!("<ul>\n{items_html}</ul>\n")
        }
        TPTag::Tag("#Item") => {
            let mut text = String::new();
            for child_rc in &node.nodes {
                let child = child_rc.borrow();
                if child.tag == TPTag::Tag("#Text") {
                    if let Some(ref token) = child.token {
                        text = format!("{token}");
                    }
                }
            }
            format!("  <li>{}</li>\n", text.trim())
        }
        TPTag::Tag("#Image") => {
            let mut url = String::new();
            for child_rc in &node.nodes {
                let child = child_rc.borrow();
                if child.tag == TPTag::Tag("#Url") {
                    if let Some(ref token) = child.token {
                        url = format!("{token}");
                    }
                }
            }
            format!("<img src=\"{url}\" alt=\"\" loading=\"lazy\">\n")
        }
        TPTag::Tag("#Header") => {
            let mut level = 0;
            let mut text = String::new();
            for child_rc in &node.nodes {
                let child = child_rc.borrow();
                if child.tag == TPTag::Tag("#Level") {
                    level = format!("{}", child.token.as_ref().unwrap()).len();
                }
                if child.tag == TPTag::Tag("#Text") {
                    text = child
                        .token
                        .as_ref()
                        .map(|t| format!("{t}"))
                        .unwrap_or_default();
                }
            }
            format!("<h{level}>{text}</h{level}>\n")
        }
        TPTag::Tag("#CodeBlock") => {
            let mut lang = String::new();
            let mut code = String::new();
            for child_rc in &node.nodes {
                let child = child_rc.borrow();
                if child.tag == TPTag::Tag("#Lang") {
                    lang = child
                        .token
                        .as_ref()
                        .map(|t| format!("{t}"))
                        .unwrap_or_default()
                        .trim()
                        .to_string();
                }
                if child.tag == TPTag::Tag("#Code") {
                    code = child
                        .token
                        .as_ref()
                        .map(|t| format!("{t}"))
                        .unwrap_or_default();
                }
            }
            format!("<pre><code class=\"language-{lang}\">{code}</code></pre>\n")
        }
        TPTag::Tag("#P") => {
            let mut text = String::new();
            for child_rc in &node.nodes {
                let child = child_rc.borrow();
                if let Some(ref token) = child.token {
                    text.push_str(&format!("{token}"));
                }
            }
            let clean_text = text.trim();
            if clean_text.is_empty() {
                String::new()
            } else {
                // ここでインライン要素を置換！
                let rendered = render_inline(clean_text);
                format!("<p>{}</p>\n", rendered.replace("\n", "<br>\n"))
            }
        }
        _ => {
            let mut html = String::new();
            for child_rc in &node.nodes {
                html.push_str(&ast_to_html(&child_rc.borrow()));
            }
            html
        }
    }
}

fn render_inline(input: &str) -> String {
    let mut out = input.to_string();

    // 強調 (太字)
    // 複雑な入れ子を考慮しないシンプルな置換
    while let Some(start) = out.find("**") {
        if let Some(end) = out[start + 2..].find("**") {
            let actual_end = start + 2 + end;
            let content = &out[start + 2..actual_end];
            let new_text = format!("<b>{}</b>", content);
            out.replace_range(start..actual_end + 2, &new_text);
        } else {
            break;
        }
    }

    // 取り消し線
    while let Some(start) = out.find("~~") {
        if let Some(end) = out[start + 2..].find("~~") {
            let actual_end = start + 2 + end;
            let content = &out[start + 2..actual_end];
            let new_text = format!("<s>{}</s>", content);
            out.replace_range(start..actual_end + 2, &new_text);
        } else {
            break;
        }
    }

    // リンク [text](url)
    while let Some(start) = out.find("[") {
        if let Some(mid) = out[start..].find("](") {
            let mid_pos = start + mid;
            if let Some(end) = out[mid_pos..].find(")") {
                let end_pos = mid_pos + end;
                let text = &out[start + 1..mid_pos];
                let url = &out[mid_pos + 2..end_pos];
                let new_link = format!("<a href=\"{}\">{}</a>", url, text);
                out.replace_range(start..end_pos + 1, &new_link);
                continue;
            }
        }
        break;
    }

    while let Some(start) = out.find("[^") {
        if let Some(end) = out[start..].find("]") {
            let end_pos = start + end;
            let id = &out[start + 2..end_pos];
            // HTMLの <sup> タグで上付きリンクにする
            let link = format!("<sup><a href=\"#fn-{id}\" id=\"fnref-{id}\">[{id}]</a></sup>");
            out.replace_range(start..end_pos + 1, &link);
            continue;
        }
        break;
    }

    out
}

fn process_footnotes(html: &str) -> (String, String) {
    let mut main_content = String::new();
    let mut footer_html = String::new();
    let mut notes = Vec::new();

    for line in html.lines() {
        if line.starts_with("<p>[^") && line.contains("]: ") {
            // 脚注の定義行: <p>[^1]: foobar</p> のような形を想定
            let trimmed = line.trim_start_matches("<p>").trim_end_matches("</p>");
            if let Some(idx) = trimmed.find("]: ") {
                let id = &trimmed[2..idx];
                let content = &trimmed[idx + 3..];
                notes.push(format!(
                    "<li id=\"fn-{id}\">{content} <a href=\"#fnref-{id}\">↩</a></li>"
                ));
                continue;
            }
        }
        main_content.push_str(line);
        main_content.push('\n');
    }

    if !notes.is_empty() {
        footer_html = format!(
            "<hr><section class=\"footnotes\"><ol>{}</ol></section>",
            notes.join("")
        );
    }

    (main_content, footer_html)
}

fn render_simple(template_path: &Path, vars: &HashMap<&str, &str>) -> Result<String> {
    let mut html = fs::read_to_string(template_path)?;
    for (key, value) in vars {
        let pattern = format!("{{{{ {} }}}}", key);
        html = html.replace(&pattern, value);
    }
    Ok(html)
}

fn prepare_dist() -> Result<()> {
    let _ = fs::remove_dir_all("dist");
    fs::create_dir_all("dist/posts")?;
    fs::create_dir_all("dist/tags")?;
    Ok(())
}

fn copy_dir_all(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> Result<()> {
    fs::create_dir_all(&dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            copy_dir_all(entry.path(), dst.as_ref().join(entry.file_name()))?;
        } else {
            fs::copy(entry.path(), dst.as_ref().join(entry.file_name()))?;
        }
    }
    Ok(())
}

fn collect_md_files(dir: &Path, files: &mut Vec<std::path::PathBuf>) -> Result<()> {
    if dir.is_dir() {
        for entry in fs::read_dir(dir)? {
            let path = entry?.path();
            if path.is_dir() {
                collect_md_files(&path, files)?;
            } else if path.extension().is_some_and(|ext| ext == "md") {
                files.push(path);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_extract_content() -> Result<()> {
        let path = Path::new("contents/posts/2026/second.md");
        let raw = fs::read_to_string(path)?;
        let ctx = parse_markdown_with_toypeg(&raw)?;
        let mut meta = HashMap::new();
        let mut body = String::new();
        extract_content(&ctx.tree.borrow(), &mut meta, &mut body);
        println!("{meta:?}");
        println!("{body:?}");
        Ok(())
    }
}
