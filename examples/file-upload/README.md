# file-upload

Multipart file upload: a native `<form>` posts to a Salvo `/upload` route that parses the body with serverfn `MultipartForm` / `MultipartPart`.

```bash
# server (parses uploads)
cargo run --no-default-features --features web-ssr
# client bundle check
cargo check --no-default-features --features web-csr
```
