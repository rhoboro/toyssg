# microblog-rs

Rust製の軽量自作SSG

```bash
# HTML がdist/ に出力される
cargo run
```

- `contents/pages/`には単一ページを格納し、投稿は`contents/posts/`に格納する。
- `contents/static/images/foo.png`に配置した画像は`![](../images/foo.png)`のように参照できる。
