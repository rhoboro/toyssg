mod constants;
mod models;
mod parser;
mod renderer;

use crate::models::{
    PostEntry, PostMeta, Result, collect_markdown_files, copy_dir_recursive, reset_dist_dir,
};
use crate::parser::MarkdownParser;
use crate::renderer::HtmlRenderer;
use std::{collections::HashMap, fs, path::Path};

fn main() -> Result<()> {
    // 1. 環境のクリーンアップ（distディレクトリの再作成）
    reset_dist_dir()?;

    // 2. 固定ページ (contents/pages) のパースとレンダリング
    let page_files = collect_markdown_files("contents/pages")?;
    for path in page_files {
        let post = load_post(&path)?;
        render_single_file(&post, "dist", "./")?;
    }

    // 3. ブログ記事 (contents/posts) の一括読み込み
    let post_files = collect_markdown_files("contents/posts")?;
    let mut posts: Vec<PostEntry> = Vec::new();
    for path in post_files {
        posts.push(load_post(&path)?);
    }

    // 公開日順にソート（新しい順）
    posts.sort_by(|a, b| b.meta.published_at.cmp(&a.meta.published_at));

    // 4. 個別記事、一覧、タグページの生成
    render_blog_collection(&posts)?;

    Ok(())
}

/// ファイルを読み込み、AST解析を経て PostEntry を生成する
fn load_post(path: &Path) -> Result<PostEntry> {
    let raw = fs::read_to_string(path)?;
    let ctx = MarkdownParser::parse(&raw)?;
    let root = ctx.tree.borrow();

    // メタデータの抽出
    let meta_map = MarkdownParser::extract_metadata(&root);
    let meta = PostMeta::from_hashmap(meta_map);

    // 本文のHTML変換（Bodyノードを探索）
    let html_content = find_and_render_body(&root);

    Ok(PostEntry::new(meta, html_content))
}

fn find_and_render_body(node: &toypeg::Node) -> String {
    if node.tag == toypeg::TPTag::Tag("#Body") {
        return HtmlRenderer::convert_ast_to_html(node);
    }
    for child in &node.nodes {
        let res = find_and_render_body(&child.borrow());
        if !res.is_empty() {
            return res;
        }
    }
    String::new()
}

/// 個別HTMLファイルの書き出し
fn render_single_file(post: &PostEntry, out_dir: &str, rel_path: &str) -> Result<()> {
    let (clean_body, footer) = HtmlRenderer::extract_footnotes(&post.content);
    let final_content = format!("{}{}", clean_body, footer);

    let mut post_vars = HashMap::new();
    post_vars.insert("title", post.meta.title.as_str());
    post_vars.insert("published_at", post.meta.published_at.as_str());
    post_vars.insert("html_content", final_content.as_str());

    let tags_html: String = post
        .meta
        .tags
        .iter()
        .map(|t| format!(r#"<a href="{}tags/{}.html">#{}</a> "#, rel_path, t, t))
        .collect();
    post_vars.insert("tags_html", tags_html.as_str());

    let content_body =
        HtmlRenderer::render_template(Path::new("contents/templates/post.html"), &post_vars)?;

    let mut base_vars = HashMap::new();
    base_vars.insert("title", post.meta.title.as_str());
    base_vars.insert("description", post.meta.description.as_str());
    base_vars.insert("og_type", "article");
    base_vars.insert("rel_path", rel_path);
    base_vars.insert("content", content_body.as_str());
    base_vars.insert("extra_head", r#"<link rel="stylesheet" href="https://cdnjs.cloudflare.com/ajax/libs/prism/1.29.0/themes/prism-tomorrow.min.css">"#);

    let final_html =
        HtmlRenderer::render_template(Path::new("contents/templates/base.html"), &base_vars)?;
    fs::write(format!("{}/{}.html", out_dir, post.file_name), final_html)?;
    Ok(())
}

fn render_blog_collection(posts: &[PostEntry]) -> Result<()> {
    let mut tags_map: HashMap<&str, Vec<&PostEntry>> = HashMap::new();

    for post in posts {
        render_single_file(post, "dist/posts", "../")?;
        for tag in &post.meta.tags {
            tags_map.entry(tag).or_default().push(post);
        }
    }

    // Index ページ生成
    let list_html: String = posts
        .iter()
        .map(|p| {
            format!(
                "<li><span>{}</span><a href='posts/{}.html'>{}</a></li>\n",
                p.meta.published_at, p.file_name, p.meta.title
            )
        })
        .collect();

    let mut idx_vars = HashMap::new();
    idx_vars.insert("posts", list_html.as_str());
    let index_body =
        HtmlRenderer::render_template(Path::new("contents/templates/index.html"), &idx_vars)?;

    let mut base_vars = HashMap::new();
    base_vars.insert("title", "Articles");
    base_vars.insert("description", "rhoboro's ToySSG記事一覧");
    base_vars.insert("og_type", "website");
    base_vars.insert("rel_path", "./");
    base_vars.insert("content", index_body.as_str());
    base_vars.insert("extra_head", "");

    let final_index =
        HtmlRenderer::render_template(Path::new("contents/templates/base.html"), &base_vars)?;
    fs::write("dist/index.html", final_index)?;

    // タグ別ページ生成
    for (tag, tag_posts) in tags_map {
        let t_list_html: String = tag_posts
            .iter()
            .map(|p| {
                format!(
                    "<li><span>{}</span><a href='../posts/{}.html'>{}</a></li>\n",
                    p.meta.published_at, p.file_name, p.meta.title
                )
            })
            .collect();

        let mut t_vars = HashMap::new();
        t_vars.insert("posts", t_list_html.as_str());
        t_vars.insert("tag_name", tag);
        let tag_body =
            HtmlRenderer::render_template(Path::new("contents/templates/tag.html"), &t_vars)?;

        let desc = format!("Tag: {} の記事一覧", tag);
        let mut t_base = HashMap::new();
        t_base.insert("title", tag);
        t_base.insert("description", &desc);
        t_base.insert("og_type", "website");
        t_base.insert("rel_path", "../");
        t_base.insert("content", tag_body.as_str());
        t_base.insert("extra_head", "");

        let final_tag =
            HtmlRenderer::render_template(Path::new("contents/templates/base.html"), &t_base)?;
        fs::write(format!("dist/tags/{}.html", tag), final_tag)?;
    }

    if Path::new("contents/static").exists() {
        copy_dir_recursive("contents/static", "dist")?;
    }
    Ok(())
}
