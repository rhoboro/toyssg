use pulldown_cmark::{Options, Parser, html};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, path::Path};
use tera::{Context, Tera};
use toypeg::{
    GrammarBuilder, MatchResult, Node, TPAny, TPChar, TPContext, TPExpr, TPMany, TPNode, TPNot,
    TPOneMany, TPOr, TPRange, TPSeq, TPTag,
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Debug, Serialize, Deserialize)]
struct PostMeta {
    title: String,
    #[serde(default)]
    published_at: String,
    slug: String,
    #[serde(default)]
    tags: String,
    #[serde(default)]
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

#[derive(Debug, Serialize)]
struct PostEntry {
    meta: PostMeta,
    file_name: String,
    content: String,
}

#[derive(Serialize)]
struct PostContext<'a> {
    title: &'a str,
    description: &'a str,
    published_at: &'a str,
    tags: &'a str,
    html_content: String, // 変換後のHTML
    rel_path: &'a str,
}

impl<'a> PostContext<'a> {
    fn new(post: &'a PostEntry, html_content: String, rel_path: &'a str) -> Self {
        Self {
            title: &post.meta.title,
            description: &post.meta.description,
            published_at: &post.meta.published_at,
            tags: &post.meta.tags,
            html_content,
            rel_path,
        }
    }
}

#[derive(Serialize)]
struct IndexContext<'a> {
    title: &'a str,
    posts: &'a [PostEntry],
    rel_path: &'a str,
}

impl<'a> IndexContext<'a> {
    fn new(posts: &'a [PostEntry]) -> Self {
        Self {
            title: "rhoboro's microblog",
            posts,
            rel_path: "./",
        }
    }
}

#[derive(Serialize)]
struct TagContext<'a> {
    title: &'a str,
    tag_name: &'a str,
    posts: Vec<&'a PostEntry>,
    rel_path: &'a str,
}

impl<'a> TagContext<'a> {
    fn new(name: &'a str, posts: Vec<&'a PostEntry>) -> Self {
        Self {
            title: name,
            tag_name: name,
            posts,
            rel_path: "../",
        }
    }
}

fn main() -> Result<()> {
    let tera = Tera::new("contents/templates/**/*.html")?;
    prepare_dist()?;

    let mut page_files = Vec::new();
    collect_md_files(Path::new("contents/pages"), &mut page_files)?;
    for page in page_files {
        let post = load_post(page.as_path())?;
        render_single_file(&tera, &post, "dist", "./")?;
    }

    let mut posts = Vec::new();
    let mut post_files = Vec::new();
    collect_md_files(Path::new("contents/posts"), &mut post_files)?;
    for post in post_files {
        posts.push(load_post(post.as_path())?);
    }
    posts.sort_by(|a, b| b.meta.published_at.cmp(&a.meta.published_at));
    render_blog_collection(&tera, &posts)?;

    Ok(())
}

fn load_post(path: &Path) -> Result<PostEntry> {
    let raw = fs::read_to_string(path)?;

    // toypeg でパースを実行
    let ctx = parse_markdown_with_toypeg(&raw)?;

    let mut meta_map = HashMap::new();
    let mut content = String::new();

    // ASTから情報を抽出
    extract_content(&ctx.tree.borrow(), &mut meta_map, &mut content);

    let meta = PostMeta::from_map(meta_map);
    let file_name = if meta.published_at.is_empty() {
        meta.slug.clone()
    } else {
        format!("{}-{}", meta.published_at, meta.slug)
    };

    Ok(PostEntry {
        file_name,
        content,
        meta,
    })
}

fn render_single_file(tera: &Tera, post: &PostEntry, out_dir: &str, rel_path: &str) -> Result<()> {
    let html_content = render_markdown(&post.content);
    let ctx = Context::from_serialize(PostContext::new(post, html_content, rel_path))?;
    let rendered = tera.render("post.html", &ctx)?;
    fs::write(format!("{}/{}.html", out_dir, post.file_name), rendered)?;
    Ok(())
}

fn render_blog_collection(tera: &Tera, posts: &[PostEntry]) -> Result<()> {
    let mut tags_map: HashMap<&str, Vec<&PostEntry>> = HashMap::new();

    for post in posts {
        render_single_file(tera, post, "dist/posts", "../")?;

        for tag in post.meta.tags.split(',') {
            let t = tag.trim();
            if !t.is_empty() {
                tags_map.entry(t).or_default().push(post);
            }
        }
    }

    // Index
    let idx_ctx = Context::from_serialize(IndexContext::new(posts))?;
    fs::write("dist/index.html", tera.render("index.html", &idx_ctx)?)?;

    // Tags
    for (tag, tag_posts) in tags_map {
        let t_ctx = Context::from_serialize(TagContext::new(tag, tag_posts))?;
        fs::write(
            format!("dist/tags/{}.html", tag),
            tera.render("tag.html", &t_ctx)?,
        )?;
    }

    if Path::new("contents/static").exists() {
        copy_dir_all("contents/static", "dist")?;
    }
    Ok(())
}

fn render_markdown(input: &str) -> String {
    let mut options = Options::empty();
    options.insert(
        Options::ENABLE_TABLES
            | Options::ENABLE_STRIKETHROUGH
            | Options::ENABLE_TASKLISTS
            | Options::ENABLE_FOOTNOTES,
    );
    let parser = Parser::new_ext(input, options);
    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);
    html_output
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
        let ty = entry.file_type()?;
        if ty.is_dir() {
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
                    TPMany::new(g.reference("ANY_CHAR")),
                    TPTag::Tag("#Body"),
                ))
                .build(),
            TPTag::Tag("#Doc"),
        )
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
        // 識別子：英数字とアンダースコア
        .insert(
            "IDENTIFIER",
            TPOneMany::new(TPOr::new(
                TPRange::new("abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ_"),
                TPRange::new("0123456789"),
            )),
        )
        // 値：改行以外のすべての文字
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

    let context = TPContext::new(input.to_owned());
    let result = grammar.borrow().get("MARKDOWN").unwrap().matches(context);

    match result? {
        MatchResult::Success(c) => Ok(c),
        MatchResult::Failure(f) => {
            // 失敗した場所を表示するためのデバッグ用
            panic!("Parse failed at offset: {:?}", f);
        }
    }
}

/// toypegのASTからメタデータと本文を抽出する
fn extract_content(node: &Node, meta: &mut HashMap<String, String>, body: &mut String) {
    match node.tag {
        // YAMLのエントリ (#Entry) を見つけたら、その子の Key と Value を探す
        TPTag::Tag("#Entry") => {
            let mut key = String::new();
            let mut value = String::new();

            for child_rc in &node.nodes {
                let child = child_rc.borrow();
                // token が存在する場合のみ文字列を抽出
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
        // 本文 (#Body) を見つけたら、その内容を String に格納
        TPTag::Tag("#Body") => {
            if let Some(ref token) = node.token {
                *body = format!("{token}");
            }
        }
        // それ以外のノード（#Root, #Doc, #FrontMatterなど）は子を再帰的に探索
        _ => {
            for child_rc in &node.nodes {
                extract_content(&child_rc.borrow(), meta, body);
            }
        }
    }
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
