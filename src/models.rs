use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Debug, Clone)]
pub struct PostMeta {
    pub title: String,
    pub published_at: String,
    pub slug: String,
    pub tags: Vec<String>,
    pub description: String,
}

impl PostMeta {
    pub fn from_hashmap(mut map: HashMap<String, String>) -> Self {
        Self {
            title: map.remove("title").unwrap_or_default(),
            published_at: map.remove("published_at").unwrap_or_default(),
            slug: map.remove("slug").unwrap_or_default(),
            tags: map
                .remove("tags")
                .map(|t| {
                    t.split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect()
                })
                .unwrap_or_default(),
            description: map.remove("description").unwrap_or_default(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct PostEntry {
    pub meta: PostMeta,
    pub file_name: String,
    pub content: String,
}

impl PostEntry {
    /// 新規作成時のロジックを一箇所に集約する
    pub fn new(meta: PostMeta, content: String) -> Self {
        let file_name = if meta.published_at.is_empty() {
            meta.slug.clone()
        } else {
            format!("{}-{}", meta.published_at, meta.slug)
        };

        Self {
            meta,
            file_name,
            content,
        }
    }
}

pub fn reset_dist_dir() -> Result<()> {
    let dist = Path::new("dist");
    if dist.exists() {
        fs::remove_dir_all(dist)?;
    }
    fs::create_dir_all("dist/posts")?;
    fs::create_dir_all("dist/tags")?;
    Ok(())
}

pub fn collect_markdown_files<P: AsRef<Path>>(dir: P) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let dir = dir.as_ref();

    if dir.is_dir() {
        for entry in fs::read_dir(dir)? {
            let path = entry?.path();
            if path.is_dir() {
                files.extend(collect_markdown_files(&path)?);
            } else if path.extension().and_then(|s| s.to_str()) == Some("md") {
                files.push(path);
            }
        }
    }
    Ok(files)
}

pub fn copy_dir_recursive<P: AsRef<Path>>(src: P, dst: P) -> Result<()> {
    let src = src.as_ref();
    let dst = dst.as_ref();

    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let target_path = dst.join(entry.file_name());

        if file_type.is_dir() {
            copy_dir_recursive(entry.path(), target_path)?;
        } else {
            fs::copy(entry.path(), target_path)?;
        }
    }
    Ok(())
}
