---
updated_at: 2026-03-15
published_at: 2026-03-15
title: 2つめのブログ
tags: rust,blog
slug: second
---

マイクロブログを作成中...

# マイクロブログとは

日常の小さな一言を残していきたい

## コードブロック

こんなコードブロックが使えると嬉しい。
**強調**や ~イタリック~ も使える？[^1]

```rust
#[derive(Serialize)]
struct PostContext<'a> {
    title: &'a str,
    published_at: &'a str,
    tags: &'a str,
    content: String, // 変換後のHTML
    rel_path: &'a str,
}

impl PostEntry {
    // 自身の参照と、外部から与えられるHTML・相対パスを組み合わせて Context 用構造体を作る
    fn to_context<'a>(&'a self, html_content: String, rel_path: &'a str) -> PostContext<'a> {
        PostContext {
            title: &self.meta.title,
            published_at: &self.meta.published_at,
            tags: &self.meta.tags,
            content: html_content,
            rel_path,
        }
    }
}
```

- [ ] task1
- [x] task2

普通の箇条書き

- foo
- bar
- baz

## 画像

画像も貼りたい。

![](https://www.rhoboro.com/images/profile/me/01.jpg)

[^1]: foobar
