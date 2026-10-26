//! Proc macros for Glory.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use std::path::{Path, PathBuf};
use syn::parse::{Parse, ParseStream};
use syn::spanned::Spanned;
use syn::{Expr, FnArg, GenericArgument, ItemFn, LitStr, Pat, Token, Type, parse_macro_input};

struct AssetFolderInput {
    crate_path: syn::Path,
    _comma: Token![,],
    root: LitStr,
}

struct CssModuleInput {
    crate_path: syn::Path,
    _comma: Token![,],
    path: LitStr,
}

impl Parse for CssModuleInput {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        Ok(Self {
            crate_path: input.parse()?,
            _comma: input.parse()?,
            path: input.parse()?,
        })
    }
}

impl Parse for AssetFolderInput {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        Ok(Self {
            crate_path: input.parse()?,
            _comma: input.parse()?,
            root: input.parse()?,
        })
    }
}

#[proc_macro]
#[doc(hidden)]
pub fn __asset_folder(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as AssetFolderInput);
    match expand_asset_folder(input) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

#[proc_macro]
#[doc(hidden)]
pub fn __css_module(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as CssModuleInput);
    match expand_css_module(input) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

fn expand_asset_folder(input: AssetFolderInput) -> syn::Result<TokenStream2> {
    let crate_path = input.crate_path;
    let root = input.root;
    let root_value = root.value();
    let manifest_dir =
        std::env::var("CARGO_MANIFEST_DIR").map_err(|err| syn::Error::new(root.span(), format!("CARGO_MANIFEST_DIR is unavailable: {err}")))?;
    let source_root = PathBuf::from(manifest_dir).join(root_value.trim_start_matches('/'));
    if !source_root.is_dir() {
        return Err(syn::Error::new(
            root.span(),
            format!("asset folder does not exist: {}", source_root.display()),
        ));
    }

    let files = collect_folder_files(&source_root)
        .map_err(|err| syn::Error::new(root.span(), format!("failed to read asset folder {}: {err}", source_root.display())))?;
    let mut asset_literals = Vec::with_capacity(files.len());
    for file in files {
        let relative = file.strip_prefix(&source_root).map_err(|err| {
            syn::Error::new(
                root.span(),
                format!("failed to make asset path relative to {}: {err}", source_root.display()),
            )
        })?;
        let relative = path_to_slash(relative).ok_or_else(|| {
            syn::Error::new(
                root.span(),
                format!("asset path is not valid UTF-8 relative to {}: {}", source_root.display(), file.display()),
            )
        })?;
        let logical = join_logical_asset_path(&root_value, &relative);
        asset_literals.push(LitStr::new(&logical, root.span()));
    }

    Ok(quote! {{
        const _GLORY_ASSET_FOLDER_BYTES: &[&[u8]] = &[
            #(include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/", #asset_literals))),*
        ];
        const _GLORY_ASSET_FOLDER_ASSETS: &[#crate_path::assets::Asset] = &[
            #(#crate_path::assets::Asset::from_static(
                #asset_literals,
                concat!(env!("CARGO_MANIFEST_DIR"), "/", #asset_literals),
            )),*
        ];
        #crate_path::assets::AssetFolder::from_static(#root, _GLORY_ASSET_FOLDER_ASSETS)
    }})
}

fn collect_folder_files(root: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let mut dirs = vec![root.to_path_buf()];
    while let Some(dir) = dirs.pop() {
        let mut entries = std::fs::read_dir(&dir)?.collect::<Result<Vec<_>, _>>()?;
        entries.sort_by_key(|entry| entry.path());
        for entry in entries.into_iter().rev() {
            let path = entry.path();
            let file_type = entry.file_type()?;
            if file_type.is_dir() {
                dirs.push(path);
            } else if file_type.is_file() {
                files.push(path);
            }
        }
    }
    files.sort();
    Ok(files)
}

fn path_to_slash(path: &Path) -> Option<String> {
    Some(path.to_str()?.replace('\\', "/"))
}

fn join_logical_asset_path(root: &str, relative: &str) -> String {
    let root = root.replace('\\', "/");
    let root = root.trim_end_matches('/');
    if root.is_empty() {
        relative.to_owned()
    } else {
        format!("{root}/{relative}")
    }
}

fn expand_css_module(input: CssModuleInput) -> syn::Result<TokenStream2> {
    let _crate_path = input.crate_path;
    let path = input.path;
    let path_value = path.value().replace('\\', "/");
    if !path_value.ends_with(".module.css") {
        return Err(syn::Error::new(path.span(), "css_module! expects a `.module.css` file"));
    }

    let manifest_dir =
        std::env::var("CARGO_MANIFEST_DIR").map_err(|err| syn::Error::new(path.span(), format!("CARGO_MANIFEST_DIR is unavailable: {err}")))?;
    let source_path = PathBuf::from(manifest_dir).join(path_value.trim_start_matches('/'));
    let css = std::fs::read_to_string(&source_path)
        .map_err(|err| syn::Error::new(path.span(), format!("failed to read CSS module {}: {err}", source_path.display())))?;

    let class_names = extract_css_classes(&css);
    if class_names.is_empty() {
        return Err(syn::Error::new(path.span(), "css_module! found no CSS classes"));
    }

    let mut method_names = std::collections::BTreeMap::<String, String>::new();
    let mut class_map = std::collections::BTreeMap::<String, String>::new();
    for class_name in class_names {
        let method = class_method_name(&class_name);
        if let Some(previous) = method_names.insert(method.clone(), class_name.clone()) {
            return Err(syn::Error::new(
                path.span(),
                format!("CSS classes `{previous}` and `{class_name}` both map to method `{method}`"),
            ));
        }
        let hash_input = format!("{path_value}\0{class_name}\0{css}");
        let hash = stable_hash(hash_input.as_bytes());
        class_map.insert(class_name.clone(), format!("{class_name}__gly_{hash:016x}"));
    }

    let rewritten_css = rewrite_css_module(&css, &class_map);
    let rewritten_css = LitStr::new(&rewritten_css, path.span());
    let include_path = LitStr::new(&path_value, path.span());
    let mut methods = Vec::new();
    for (method, original) in method_names {
        let ident = format_ident!("{}", method);
        let class_name = LitStr::new(class_map.get(&original).expect("class map contains method class"), path.span());
        methods.push(quote! {
            pub const fn #ident(&self) -> &'static str {
                #class_name
            }
        });
    }

    Ok(quote! {{
        const _GLORY_CSS_MODULE_SOURCE: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/", #include_path));
        let _ = _GLORY_CSS_MODULE_SOURCE;
        #[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
        struct GloryCssModule;
        impl GloryCssModule {
            pub const fn css(&self) -> &'static str {
                #rewritten_css
            }
            #(#methods)*
        }
        GloryCssModule
    }})
}

fn extract_css_classes(css: &str) -> Vec<String> {
    let mut classes = Vec::new();
    let chars = css.char_indices().collect::<Vec<_>>();
    let mut index = 0usize;
    while index < chars.len() {
        let (_, ch) = chars[index];
        if ch != '.' {
            index += 1;
            continue;
        }
        let Some((_, next)) = chars.get(index + 1).copied() else {
            index += 1;
            continue;
        };
        if !is_css_ident_start(next) {
            index += 1;
            continue;
        }
        let start = index + 1;
        let mut end = start + 1;
        while end < chars.len() && is_css_ident_continue(chars[end].1) {
            end += 1;
        }
        let start_byte = chars[start].0;
        let end_byte = chars.get(end).map(|(byte, _)| *byte).unwrap_or(css.len());
        let class_name = css[start_byte..end_byte].to_owned();
        if !classes.contains(&class_name) {
            classes.push(class_name);
        }
        index = end;
    }
    classes.sort();
    classes
}

fn rewrite_css_module(css: &str, class_map: &std::collections::BTreeMap<String, String>) -> String {
    let chars = css.char_indices().collect::<Vec<_>>();
    let mut out = String::with_capacity(css.len());
    let mut index = 0usize;
    while index < chars.len() {
        let (byte, ch) = chars[index];
        if ch != '.' {
            out.push(ch);
            index += 1;
            continue;
        }
        let Some((_, next)) = chars.get(index + 1).copied() else {
            out.push(ch);
            index += 1;
            continue;
        };
        if !is_css_ident_start(next) {
            out.push(ch);
            index += 1;
            continue;
        }
        let start = index + 1;
        let mut end = start + 1;
        while end < chars.len() && is_css_ident_continue(chars[end].1) {
            end += 1;
        }
        let start_byte = chars[start].0;
        let end_byte = chars.get(end).map(|(byte, _)| *byte).unwrap_or(css.len());
        let class_name = &css[start_byte..end_byte];
        if let Some(rewritten) = class_map.get(class_name) {
            out.push('.');
            out.push_str(rewritten);
        } else {
            out.push_str(&css[byte..end_byte]);
        }
        index = end;
    }
    out
}

fn is_css_ident_start(ch: char) -> bool {
    ch == '_' || ch == '-' || ch.is_ascii_alphabetic()
}

fn is_css_ident_continue(ch: char) -> bool {
    is_css_ident_start(ch) || ch.is_ascii_digit()
}

fn class_method_name(class_name: &str) -> String {
    let mut out = String::with_capacity(class_name.len());
    for ch in class_name.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push('_');
        }
    }
    while out.contains("__") {
        out = out.replace("__", "_");
    }
    let out = out.trim_matches('_');
    let mut out = if out.is_empty() { "class".to_owned() } else { out.to_owned() };
    if out.chars().next().is_some_and(|ch| ch.is_ascii_digit()) {
        out.insert(0, '_');
    }
    if is_rust_keyword(&out) {
        out.push('_');
    }
    out
}

fn is_rust_keyword(value: &str) -> bool {
    matches!(
        value,
        "as" | "break"
            | "const"
            | "continue"
            | "crate"
            | "else"
            | "enum"
            | "extern"
            | "false"
            | "fn"
            | "for"
            | "if"
            | "impl"
            | "in"
            | "let"
            | "loop"
            | "match"
            | "mod"
            | "move"
            | "mut"
            | "pub"
            | "ref"
            | "return"
            | "self"
            | "Self"
            | "static"
            | "struct"
            | "super"
            | "trait"
            | "true"
            | "type"
            | "unsafe"
            | "use"
            | "where"
            | "while"
            | "async"
            | "await"
            | "dyn"
    )
}

fn stable_hash(bytes: &[u8]) -> u64 {
    const OFFSET: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x100000001b3;
    bytes.iter().fold(OFFSET, |hash, byte| (hash ^ u64::from(*byte)).wrapping_mul(PRIME))
}

/// Turns an `async fn` into a *server function*: the body runs on the
/// server, and clients call it transparently over HTTP.
///
/// ```ignore
/// #[server]
/// async fn list_todos(filter: Filter) -> Result<Vec<Todo>, ServerFnError> {
///     // database access — compiled into server builds only
/// }
/// ```
///
/// Split rule:
/// - **non-wasm builds** keep the original function and register it under
///   `/__glory/fn/<name>` in the [`inventory`]-backed registry consumed by
///   the adapter mounts (`glory_serverfn::salvo_mount` etc.).
/// - **wasm builds** replace the body with a `fetch` call to that endpoint;
///   arguments are serialized as a JSON tuple, the result deserialized from
///   the response body.
///
/// Requirements: the function must be `async`, take owned serializable
/// arguments with plain identifier patterns, and return
/// `Result<T, ServerFnError>`.
///
/// `#[server(endpoint = "custom-name")]` overrides the URL segment when two
/// functions in different modules would otherwise collide on the name.
/// `#[server(method = "GET")]` sends the JSON tuple through a query string
/// parameter; the default method is `POST`.
/// `#[server(encoding = "cbor")]` or `#[server(encoding = "postcard")]`
/// requests that encoding for the generated client stub when the matching
/// `glory-serverfn` feature is enabled. GET server functions currently use
/// JSON query arguments and therefore only support the default JSON encoding.
/// `#[server(middleware = require_auth)]` or a sibling
/// `#[middleware(require_auth)]` attribute registers an adapter-neutral
/// server-side middleware function for this endpoint.
/// `#[server(stream)]` marks a streaming server function. Its return type must
/// be `Result<StreamResponse<T>, ServerFnError>` (NDJSON items),
/// `Result<JsonStream<T>, ServerFnError>` (alias), or
/// `Result<StreamingBytes, ServerFnError>` / `Result<ByteStream, ...>` (raw
/// binary download chunks). The server build keeps the original function so an
/// adapter/resource route can pipe it through `into_streaming_response()`; the
/// wasm client stub fetches the body and decodes it back into a stream.
#[proc_macro_attribute]
pub fn server(attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut item_fn = parse_macro_input!(item as ItemFn);

    let mut endpoint: Option<String> = None;
    let mut method = "POST".to_owned();
    let mut encoding = "json".to_owned();
    let mut middlewares = Vec::<Expr>::new();
    let mut stream = false;
    if !attr.is_empty() {
        let parser = syn::meta::parser(|meta| {
            if meta.path.is_ident("stream") {
                stream = true;
                Ok(())
            } else if meta.path.is_ident("endpoint") {
                let value: LitStr = meta.value()?.parse()?;
                endpoint = Some(value.value());
                Ok(())
            } else if meta.path.is_ident("method") {
                let value: LitStr = meta.value()?.parse()?;
                let value = value.value().to_ascii_uppercase();
                match value.as_str() {
                    "GET" | "POST" => {
                        method = value;
                        Ok(())
                    }
                    _ => Err(meta.error("unsupported #[server] method; expected `GET` or `POST`")),
                }
            } else if meta.path.is_ident("encoding") {
                let value: LitStr = meta.value()?.parse()?;
                let value = value.value().to_ascii_lowercase();
                match value.as_str() {
                    "json" | "cbor" | "postcard" => {
                        encoding = value;
                        Ok(())
                    }
                    _ => Err(meta.error("unsupported #[server] encoding; expected `json`, `cbor`, or `postcard`")),
                }
            } else if meta.path.is_ident("middleware") {
                let value: Expr = meta.value()?.parse()?;
                middlewares.push(value);
                Ok(())
            } else {
                Err(meta.error(
                    "unsupported #[server] option; expected `stream`, `endpoint = \"...\"`, `method = \"GET\"`, `encoding = \"cbor\"`, `encoding = \"postcard\"`, or `middleware = path`",
                ))
            }
        });
        parse_macro_input!(attr with parser);
    }
    if method == "GET" && encoding != "json" {
        return syn::Error::new(
            item_fn.sig.span(),
            "#[server(method = \"GET\")] currently supports only JSON query argument encoding",
        )
        .to_compile_error()
        .into();
    }

    let mut retained_attrs = Vec::new();
    for attr in std::mem::take(&mut item_fn.attrs) {
        if attr.path().is_ident("middleware") {
            match attr.parse_args::<Expr>() {
                Ok(middleware) => middlewares.push(middleware),
                Err(err) => return err.to_compile_error().into(),
            }
        } else {
            retained_attrs.push(attr);
        }
    }
    item_fn.attrs = retained_attrs;

    if item_fn.sig.asyncness.is_none() {
        return syn::Error::new(item_fn.sig.span(), "#[server] functions must be `async fn`")
            .to_compile_error()
            .into();
    }

    let mut arg_idents = Vec::new();
    let mut arg_types = Vec::new();
    for input in &item_fn.sig.inputs {
        match input {
            FnArg::Receiver(receiver) => {
                return syn::Error::new(receiver.span(), "#[server] functions cannot take `self`")
                    .to_compile_error()
                    .into();
            }
            FnArg::Typed(arg) => match &*arg.pat {
                Pat::Ident(ident) => {
                    arg_idents.push(ident.ident.clone());
                    arg_types.push((*arg.ty).clone());
                }
                other => {
                    return syn::Error::new(other.span(), "#[server] arguments must be plain identifiers")
                        .to_compile_error()
                        .into();
                }
            },
        }
    }

    let vis = &item_fn.vis;
    let sig = &item_fn.sig;
    let name = &sig.ident;
    let endpoint = endpoint.unwrap_or_else(|| name.to_string());
    let url = format!("/__glory/fn/{endpoint}");
    let encoding = match encoding.as_str() {
        "json" => quote! { glory_serverfn::ServerFnEncoding::Json },
        "cbor" => quote! { glory_serverfn::ServerFnEncoding::Cbor },
        "postcard" => quote! { glory_serverfn::ServerFnEncoding::Postcard },
        _ => unreachable!("validated server fn encoding"),
    };
    if stream {
        if method == "GET" {
            return syn::Error::new(sig.span(), "#[server(stream)] currently supports only POST requests")
                .to_compile_error()
                .into();
        }
        // Server build keeps the original function so adapter/resource routes can
        // call it and pipe the result through `into_streaming_response()`. The
        // wasm client stub fetches the response body and decodes it back into a
        // stream. Byte-stream functions (returning `StreamingBytes`/`ByteStream`)
        // read raw chunks; all others decode NDJSON items.
        let byte_stream = server_fn_returns_byte_stream(&sig.output);
        let client_call = if byte_stream {
            quote! { glory_serverfn::call_remote_byte_stream(#method, #url, &( #(#arg_idents,)* )).await }
        } else {
            quote! { glory_serverfn::call_remote_stream(#method, #url, &( #(#arg_idents,)* )).await }
        };
        let expanded = quote! {
            #[cfg(not(target_arch = "wasm32"))]
            #item_fn

            #[cfg(target_arch = "wasm32")]
            #vis #sig {
                #client_call
            }
        };
        return expanded.into();
    }

    let decode_args = if arg_idents.len() == 1 {
        let arg_ident = &arg_idents[0];
        let arg_type = &arg_types[0];
        quote! {
            if glory_serverfn::is_form_request() {
                let #arg_ident: #arg_type = glory_serverfn::decode_form(&__body)?;
                ( #arg_ident, )
            } else {
                glory_serverfn::decode_args_with(__input_encoding, &__body)?
            }
        }
    } else {
        quote! {
            glory_serverfn::decode_args_with(__input_encoding, &__body)?
        }
    };

    let expanded = quote! {
        #[cfg(not(target_arch = "wasm32"))]
        #item_fn

        #[cfg(not(target_arch = "wasm32"))]
        glory_serverfn::inventory::submit! {
            glory_serverfn::ServerFnEntry {
                path: #url,
                method: #method,
                middlewares: &[ #(#middlewares),* ],
                handler: |__body: ::std::vec::Vec<u8>, __input_encoding: glory_serverfn::ServerFnEncoding, __output_encoding: glory_serverfn::ServerFnEncoding| ::std::boxed::Box::pin(async move {
                    let ( #(#arg_idents,)* ): ( #(#arg_types,)* ) = #decode_args;
                    let __output = #name( #(#arg_idents),* ).await?;
                    glory_serverfn::encode_ok_with(__output_encoding, &__output)
                }),
            }
        }

        #[cfg(target_arch = "wasm32")]
        #vis #sig {
            glory_serverfn::call_remote_with_method_and_encoding(#method, #url, &( #(#arg_idents,)* ), #encoding).await
        }
    };
    expanded.into()
}

/// True when a `#[server(stream)]` function's `Ok` type is a binary chunk
/// stream (`StreamingBytes` or its `ByteStream` alias), as opposed to an NDJSON
/// item stream. Recognizes the bare type name regardless of module path.
fn server_fn_returns_byte_stream(output: &syn::ReturnType) -> bool {
    let syn::ReturnType::Type(_, ty) = output else {
        return false;
    };
    // Unwrap `Result<Ok, _>` to inspect the `Ok` type when present; otherwise
    // inspect the return type directly.
    let ok_ty = generic_inner_type(ty, "Result").unwrap_or(ty);
    let Type::Path(path) = ok_ty else {
        return false;
    };
    path.path
        .segments
        .last()
        .is_some_and(|segment| segment.ident == "StreamingBytes" || segment.ident == "ByteStream")
}

fn generic_inner_type<'a>(ty: &'a Type, wrapper: &str) -> Option<&'a Type> {
    let Type::Path(path) = ty else {
        return None;
    };
    let segment = path.path.segments.last()?;
    if segment.ident != wrapper {
        return None;
    }
    let syn::PathArguments::AngleBracketed(args) = &segment.arguments else {
        return None;
    };
    args.args.iter().find_map(|arg| match arg {
        GenericArgument::Type(ty) => Some(ty),
        _ => None,
    })
}
