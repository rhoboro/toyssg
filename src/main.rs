use pulldown_cmark::{Options, Parser, html};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, path::Path};
use tera::{Context, Tera};
use walkdir::WalkDir;

#[derive(Debug, Serialize, Deserialize)]
struct PostMeta {
    title: String,
    #[serde(default)] // page の場合は空になるため
    published_at: String,
    slug: String,
    #[serde(default)]
    tags: String,
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
    published_at: &'a str,
    tags: &'a str,
    html_content: String, // 変換後のHTML
    rel_path: &'a str,
}

impl<'a> PostContext<'a> {
    fn new(post: &'a PostEntry, html_content: String, rel_path: &'a str) -> Self {
        Self {
            title: &post.meta.title,
            published_at: &post.meta.published_at,
            tags: &post.meta.tags,
            html_content,
            rel_path,
        }
    }
}

#[derive(Serialize)]
struct IndexContext<'a> {
    posts: &'a [PostEntry],
    rel_path: &'a str,
}

impl<'a> IndexContext<'a> {
    fn new(posts: &'a [PostEntry]) -> Self {
        Self {
            posts,
            rel_path: "./",
        }
    }
}

#[derive(Serialize)]
struct TagContext<'a> {
    tag_name: &'a str,
    posts: Vec<&'a PostEntry>,
    rel_path: &'a str,
}

impl<'a> TagContext<'a> {
    fn new(name: &'a str, posts: Vec<&'a PostEntry>) -> Self {
        Self {
            tag_name: name,
            posts,
            rel_path: "../",
        }
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let tera = Tera::new("contents/templates/**/*.html")?;
    prepare_dist()?;

    if Path::new("contents/pages").exists() {
        for entry in WalkDir::new("contents/pages")
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if entry.path().extension().is_some_and(|s| s == "md") {
                let post = load_post(entry.path())?;
                render_single_file(&tera, &post, "dist", "./")?;
            }
        }
    }

    let mut posts = Vec::new();
    for entry in WalkDir::new("contents/posts")
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.path().extension().is_some_and(|s| s == "md") {
            posts.push(load_post(entry.path())?);
        }
    }
    posts.sort_by(|a, b| b.meta.published_at.cmp(&a.meta.published_at));

    render_blog_collection(&tera, &posts)?;
    Ok(())
}

fn load_post(path: &Path) -> Result<PostEntry, Box<dyn std::error::Error>> {
    let raw = fs::read_to_string(path)?;
    let parts: Vec<&str> = raw.splitn(3, "---").collect();
    let meta: PostMeta = serde_yaml::from_str(parts[1])?;

    // slug があればそれを使う。日付がある場合は日付を接頭辞にする
    let file_name = if meta.published_at.is_empty() {
        meta.slug.clone()
    } else {
        format!("{}-{}", meta.published_at, meta.slug)
    };

    Ok(PostEntry {
        file_name,
        content: parts[2].to_string(),
        meta,
    })
}

fn render_single_file(
    tera: &Tera,
    post: &PostEntry,
    out_dir: &str,
    rel_path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let html_content = render_markdown(&post.content);
    let ctx = Context::from_serialize(PostContext::new(post, html_content, rel_path))?;
    let rendered = tera.render("post.html", &ctx)?;
    fs::write(format!("{}/{}.html", out_dir, post.file_name), rendered)?;
    Ok(())
}

fn render_blog_collection(
    tera: &Tera,
    posts: &[PostEntry],
) -> Result<(), Box<dyn std::error::Error>> {
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

    if Path::new("contents/static/style.css").exists() {
        fs::copy("contents/static/style.css", "dist/style.css")?;
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

fn prepare_dist() -> Result<(), std::io::Error> {
    let _ = fs::remove_dir_all("dist");
    fs::create_dir_all("dist/posts")?;
    fs::create_dir_all("dist/tags")?;
    Ok(())
}
