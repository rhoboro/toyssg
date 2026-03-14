use std::collections::HashMap;
use std::fs;
use std::path::Path;

use pulldown_cmark::{Options, Parser, html};
use serde::{Deserialize, Serialize};
use tera::{Context, Tera};
use walkdir::WalkDir;

#[derive(Debug, Deserialize)]
struct PageMeta {
    title: String,
    slug: String,
}

#[derive(Debug, Deserialize)]
struct PostMeta {
    title: String,
    published_at: String,
    slug: String,
    tags: String,
}

#[derive(Debug, Serialize, Clone)]
struct PostSummary {
    title: String,
    published_at: String,
    file_name: String,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let tera = Tera::new("templates/**/*.html").expect("Template parsing error");
    let mut all_posts = Vec::new();
    let mut tags_map: HashMap<String, Vec<PostSummary>> = HashMap::new();

    fs::create_dir_all("dist/posts")?;
    fs::create_dir_all("dist/tags")?;

    // pages
    if Path::new("pages").exists() {
        for entry in WalkDir::new("pages").into_iter().filter_map(|e| e.ok()) {
            if entry.path().extension().is_some_and(|s| s == "md") {
                let raw = fs::read_to_string(entry.path())?;
                let parts: Vec<&str> = raw.splitn(3, "---").collect();
                if parts.len() < 3 {
                    continue;
                }

                let meta: PageMeta = serde_yaml::from_str(parts[1])?;
                let html_body = render_markdown(parts[2]);

                let mut ctx = Context::new();
                ctx.insert("rel_path", "./");
                ctx.insert("title", &meta.title);
                ctx.insert("content", &html_body);
                ctx.insert("published_at", "");

                let rendered = tera.render("post.html", &ctx)?;
                fs::write(format!("dist/{}.html", meta.slug), rendered)?;
            }
        }
    }

    // posts
    for entry in WalkDir::new("posts").into_iter().filter_map(|e| e.ok()) {
        if entry.path().extension().is_some_and(|s| s == "md") {
            let raw = fs::read_to_string(entry.path())?;
            let parts: Vec<&str> = raw.splitn(3, "---").collect();
            if parts.len() < 3 {
                continue;
            }

            let meta: PostMeta = serde_yaml::from_str(parts[1])?;
            let file_name = format!("{}-{}", meta.published_at, meta.slug);
            let html_body = render_markdown(parts[2]);

            let summary = PostSummary {
                title: meta.title.clone(),
                published_at: meta.published_at.clone(),
                file_name: file_name.clone(),
            };

            let mut post_ctx = Context::new();
            post_ctx.insert("rel_path", "../");
            post_ctx.insert("title", &meta.title);
            post_ctx.insert("published_at", &meta.published_at);
            post_ctx.insert("tags", &meta.tags);
            post_ctx.insert("content", &html_body);
            let rendered = tera.render("post.html", &post_ctx)?;
            fs::write(format!("dist/posts/{}.html", file_name), rendered)?;

            all_posts.push(summary.clone());
            for tag in meta.tags.split(",") {
                tags_map
                    .entry(tag.trim().to_string())
                    .or_default()
                    .push(summary.clone());
            }
        }
    }

    // index
    all_posts.sort_by(|a, b| b.published_at.cmp(&a.published_at));
    let mut idx_ctx = Context::new();
    idx_ctx.insert("rel_path", "./");
    idx_ctx.insert("posts", &all_posts);
    fs::write("dist/index.html", tera.render("index.html", &idx_ctx)?)?;

    // tag
    for (tag, mut tag_posts) in tags_map {
        tag_posts.sort_by(|a, b| b.published_at.cmp(&a.published_at));
        let mut t_ctx = Context::new();
        t_ctx.insert("rel_path", "../");
        t_ctx.insert("tag_name", &tag);
        t_ctx.insert("posts", &tag_posts);
        fs::write(
            format!("dist/tags/{}.html", tag),
            tera.render("tag.html", &t_ctx)?,
        )?;
    }

    if Path::new("static/style.css").exists() {
        fs::copy("static/style.css", "dist/style.css")?;
    }

    Ok(())
}

fn render_markdown(input: &str) -> String {
    let mut options = Options::empty();
    options
        .insert(Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS);
    let parser = Parser::new_ext(input, options);
    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);
    html_output
}
