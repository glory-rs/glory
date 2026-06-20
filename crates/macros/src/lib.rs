//! Proc macros for Glory.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use std::path::{Path, PathBuf};
use syn::parse::{Parse, ParseStream};
use syn::spanned::Spanned;
use syn::{Attribute, Data, DeriveInput, Expr, Fields, FnArg, GenericArgument, Ident, ItemFn, LitStr, Pat, Token, Type, parse_macro_input};

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

/// Derives a typed field-accessor *store* for a named-field struct, so a
/// root [`Cage`] can be projected into a per-field
/// [`CageLens`] without writing the getter/setter closures by hand.
///
/// For `struct Foo { a: A, b: B }` the derive generates:
///
/// - `FooStore<Root>` — a cheap, `Clone` accessor wrapping a
///   `CageLens<Root, Foo>`. It exposes `.a() -> CageLens<Root, A>` and
///   `.b() -> CageLens<Root, B>`, each built with
///   `root_lens.lens(|f| &f.a, |f| &mut f.a)`. It also re-exposes the
///   underlying root lens via `.as_lens()` / `.cage()` accessors.
/// - `FooStoreExt` — an extension trait with `.store()`, implemented for
///   both `Cage<Foo>` and `CageLens<R, Foo>`, so any handle to a `Foo`
///   can produce a `FooStore`.
///
/// ```ignore
/// #[derive(Debug, glory::Store)]
/// struct Counter { count: i32, label: String }
///
/// let cage = glory::reflow::Cage::new(Counter { count: 0, label: "n".into() });
/// let store = cage.store();          // FooStoreExt::store
/// store.count().set(5);              // revises only the `count` projection
/// assert_eq!(*store.label().get_untracked(), "n");
/// ```
///
/// Subscription granularity: each field accessor returns a `CageLens`
/// whose reads project through the **root** `Cage`. Reading
/// `store.count().get()` subscribes the caller to the root cell, and
/// writing through any field lens bumps the root `Cage` version. The
/// ergonomic win is typed, boilerplate-free field projection; this derive
/// does *not* add finer-than-root invalidation — that is bounded by what
/// `CageLens::get()` subscribes to today (the root version).
///
/// Only named-field structs are supported. Tuple structs, unit structs,
/// and enums produce a compile error.
#[proc_macro_derive(Store)]
pub fn derive_store(input: TokenStream) -> TokenStream {
    match expand_store(parse_macro_input!(input as DeriveInput)) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

/// Resolve the path to the reflow module hosting [`CageLens`]/[`Cage`] for
/// the crate currently invoking the [`Store`](derive_store) derive.
///
/// Resolves to `::glory::reflow` for downstream crates depending on the
/// umbrella `glory` crate, `::glory_core::reflow` for crates depending on
/// `glory-core` directly, and `crate::reflow` when used from within
/// `glory-core` itself.
fn reflow_path() -> TokenStream2 {
    use proc_macro_crate::{FoundCrate, crate_name};

    if let Ok(found) = crate_name("glory-core") {
        return match found {
            FoundCrate::Itself => quote! { crate::reflow },
            FoundCrate::Name(name) => {
                let ident = format_ident!("{}", name);
                quote! { ::#ident::reflow }
            }
        };
    }

    match crate_name("glory") {
        Ok(FoundCrate::Itself) => quote! { crate::reflow },
        Ok(FoundCrate::Name(name)) => {
            let ident = format_ident!("{}", name);
            quote! { ::#ident::reflow }
        }
        Err(_) => quote! { ::glory::reflow },
    }
}

fn expand_store(input: DeriveInput) -> syn::Result<TokenStream2> {
    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => &fields.named,
            Fields::Unnamed(_) | Fields::Unit => {
                return Err(syn::Error::new(
                    input.ident.span(),
                    "#[derive(Store)] only supports structs with named fields",
                ));
            }
        },
        _ => {
            return Err(syn::Error::new(input.ident.span(), "#[derive(Store)] can only be used on structs"));
        }
    };

    if !input.generics.params.is_empty() {
        return Err(syn::Error::new(
            input.ident.span(),
            "#[derive(Store)] does not support generic structs yet",
        ));
    }

    let reflow = reflow_path();
    let name = &input.ident;
    let vis = &input.vis;
    let store_ident = format_ident!("{}Store", name);
    let ext_ident = format_ident!("{}StoreExt", name);

    // Pick a root type parameter name that cannot collide with a real type.
    let root_param = format_ident!("__GloryStoreRoot");

    let mut accessors = Vec::with_capacity(fields.len());
    for field in fields {
        let field_ident = field.ident.as_ref().expect("named field has an ident");
        let field_ty = &field.ty;
        let doc = format!("Project a field lens onto `{name}::{field_ident}`.");
        accessors.push(quote! {
            #[doc = #doc]
            #vis fn #field_ident(&self) -> #reflow::CageLens<#root_param, #field_ty> {
                self.__glory_lens.lens(
                    |__glory_root| &__glory_root.#field_ident,
                    |__glory_root| &mut __glory_root.#field_ident,
                )
            }
        });
    }

    let store_doc = format!("Typed field-accessor store for [`{name}`], generated by `#[derive(Store)]`.");
    let ext_doc = format!("Extension trait producing a [`{store_ident}`] from any handle to a [`{name}`].");

    Ok(quote! {
        #[doc = #store_doc]
        #vis struct #store_ident<#root_param>
        where
            #root_param: ::std::fmt::Debug + 'static,
        {
            __glory_lens: #reflow::CageLens<#root_param, #name>,
        }

        impl<#root_param> ::std::clone::Clone for #store_ident<#root_param>
        where
            #root_param: ::std::fmt::Debug + 'static,
        {
            fn clone(&self) -> Self {
                Self {
                    __glory_lens: ::std::clone::Clone::clone(&self.__glory_lens),
                }
            }
        }

        impl<#root_param> #store_ident<#root_param>
        where
            #root_param: ::std::fmt::Debug + 'static,
        {
            #[doc = "Wrap an existing root lens that points at this struct."]
            #vis fn new(lens: #reflow::CageLens<#root_param, #name>) -> Self {
                Self { __glory_lens: lens }
            }

            #[doc = "Borrow the underlying root lens for this struct."]
            #vis fn as_lens(&self) -> #reflow::CageLens<#root_param, #name> {
                ::std::clone::Clone::clone(&self.__glory_lens)
            }

            #[doc = "The root reactive cell backing this store."]
            #vis fn cage(&self) -> #reflow::Cage<#root_param> {
                self.__glory_lens.root()
            }

            #(#accessors)*
        }

        #[doc = #ext_doc]
        #vis trait #ext_ident {
            #[doc = "The root reactive type backing the produced store."]
            type Root: ::std::fmt::Debug + 'static;
            #[doc = "Produce the typed field-accessor store."]
            fn store(self) -> #store_ident<Self::Root>;
        }

        impl #ext_ident for #reflow::Cage<#name> {
            type Root = #name;
            fn store(self) -> #store_ident<Self::Root> {
                #store_ident::new(#reflow::StoreExt::lens(
                    &self,
                    |__glory_root| __glory_root,
                    |__glory_root| __glory_root,
                ))
            }
        }

        impl<#root_param> #ext_ident for #reflow::CageLens<#root_param, #name>
        where
            #root_param: ::std::fmt::Debug + 'static,
        {
            type Root = #root_param;
            fn store(self) -> #store_ident<Self::Root> {
                #store_ident::new(self)
            }
        }
    })
}

/// Derives `glory::routing::Routable` for an enum.
///
/// Supported route attributes intentionally mirror Glory's existing path
/// syntax:
///
/// ```ignore
/// #[derive(glory::Routable)]
/// enum Route {
///     #[route("/")]
///     Home,
///     #[route("/users/<id>")]
///     User { id: u64 },
///     #[route("/files/<**path>")]
///     Files { path: Vec<String> },
///     #[route("/users/<id>")]
///     #[redirect("/u/<id>")]
///     LegacyUser { id: u64 },
///     #[not_found]
///     NotFound { raw_url: String },
/// }
/// ```
#[proc_macro_derive(Routable, attributes(route, redirect, not_found))]
pub fn derive_routable(input: TokenStream) -> TokenStream {
    match expand_routable(parse_macro_input!(input as DeriveInput)) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

/// Resolve the path to the `routing` module for the crate currently invoking
/// the derive.
///
/// Resolves to `::glory::routing` for downstream crates that depend on the
/// umbrella `glory` crate, and to `crate`/`::glory_routing` when the derive is
/// used from within `glory-routing` itself (so the routing crate can exercise
/// the derive in its own tests).
fn routing_path() -> TokenStream2 {
    use proc_macro_crate::{FoundCrate, crate_name};

    if let Ok(found) = crate_name("glory-routing") {
        return match found {
            FoundCrate::Itself => quote! { crate },
            FoundCrate::Name(name) => {
                let ident = format_ident!("{}", name);
                quote! { ::#ident }
            }
        };
    }

    match crate_name("glory") {
        Ok(FoundCrate::Itself) => quote! { crate::routing },
        Ok(FoundCrate::Name(name)) => {
            let ident = format_ident!("{}", name);
            quote! { ::#ident::routing }
        }
        Err(_) => quote! { ::glory::routing },
    }
}

fn expand_routable(input: DeriveInput) -> syn::Result<TokenStream2> {
    let enum_data = match &input.data {
        Data::Enum(data) => data,
        _ => {
            return Err(syn::Error::new(input.ident.span(), "#[derive(Routable)] can only be used on enums"));
        }
    };

    let krate = routing_path();

    let mut to_url_arms = Vec::new();
    let mut from_url_checks = Vec::new();
    let mut redirect_checks = Vec::new();
    let mut not_found_impl = None;

    for variant in &enum_data.variants {
        let route = route_attr(&variant.attrs)?;
        let redirects = redirect_attrs(&variant.attrs)?;
        let is_not_found = has_marker_attr(&variant.attrs, "not_found");

        if let Some(route) = route {
            let pattern = parse_route_pattern(&route)?;
            to_url_arms.push(to_url_arm(&input.ident, &variant.ident, &variant.fields, &pattern, &krate)?);
            from_url_checks.push(from_url_check(&input.ident, &variant.ident, &variant.fields, &pattern, &krate)?);
        } else if is_not_found {
            to_url_arms.push(not_found_to_url_arm(&input.ident, &variant.ident, &variant.fields)?);
        } else {
            return Err(syn::Error::new(
                variant.ident.span(),
                "route variants need #[route(\"/path\")] or #[not_found]",
            ));
        }

        for redirect in redirects {
            let pattern = parse_route_pattern(&redirect)?;
            redirect_checks.push(from_url_check(&input.ident, &variant.ident, &variant.fields, &pattern, &krate)?);
        }

        if is_not_found {
            if not_found_impl.is_some() {
                return Err(syn::Error::new(variant.ident.span(), "only one #[not_found] variant is supported"));
            }
            not_found_impl = Some(not_found_check(&input.ident, &variant.ident, &variant.fields)?);
        }
    }

    let name = &input.ident;
    let (impl_generics, type_generics, where_clause) = input.generics.split_for_impl();
    let not_found_body = not_found_impl.unwrap_or_else(|| quote! { None });

    Ok(quote! {
        impl #impl_generics #krate::Routable for #name #type_generics #where_clause {
            fn to_url(&self) -> ::std::string::String {
                match self {
                    #(#to_url_arms)*
                }
            }

            fn from_url(url: &str) -> ::std::option::Option<Self> {
                #(#from_url_checks)*
                None
            }

            fn redirect(url: &str) -> ::std::option::Option<Self> {
                #(#redirect_checks)*
                None
            }

            fn not_found(url: &str) -> ::std::option::Option<Self> {
                #not_found_body
            }
        }
    })
}

#[derive(Clone)]
struct RoutePattern {
    /// The path-only portion of the pattern (no `?query`), as a literal usable
    /// with [`match_route_pattern`].
    path_literal: String,
    parts: Vec<RoutePart>,
    /// Path parameters, in pattern order.
    params: Vec<RouteParam>,
    /// Declared query parameters, in pattern order.
    queries: Vec<QueryParam>,
}

impl RoutePattern {
    /// All variant fields bound by this pattern (path params then query params).
    fn field_names(&self) -> impl Iterator<Item = &str> {
        self.params
            .iter()
            .map(|p| p.field.as_str())
            .chain(self.queries.iter().map(|q| q.field.as_str()))
    }
}

#[derive(Clone)]
enum RoutePart {
    Const(String),
    Param(RouteParam),
}

#[derive(Clone)]
struct RouteParam {
    key: String,
    field: String,
    catch_all: bool,
}

#[derive(Clone)]
struct QueryParam {
    /// Query string key (defaults to the field name).
    key: String,
    /// Variant field this query parameter binds to.
    field: String,
}

fn route_attr(attrs: &[Attribute]) -> syn::Result<Option<LitStr>> {
    let mut found = None;
    for attr in attrs.iter().filter(|attr| attr.path().is_ident("route")) {
        if found.is_some() {
            return Err(syn::Error::new(attr.span(), "only one #[route(\"...\")] attribute is supported"));
        }
        found = Some(attr.parse_args::<LitStr>()?);
    }
    Ok(found)
}

fn redirect_attrs(attrs: &[Attribute]) -> syn::Result<Vec<LitStr>> {
    attrs
        .iter()
        .filter(|attr| attr.path().is_ident("redirect"))
        .map(|attr| attr.parse_args::<LitStr>())
        .collect()
}

fn has_marker_attr(attrs: &[Attribute], name: &str) -> bool {
    attrs.iter().any(|attr| attr.path().is_ident(name))
}

fn parse_route_pattern(lit: &LitStr) -> syn::Result<RoutePattern> {
    let pattern = lit.value();
    if !pattern.starts_with('/') {
        return Err(syn::Error::new(lit.span(), "route patterns must start with '/'"));
    }
    if pattern.contains('#') {
        return Err(syn::Error::new(
            lit.span(),
            "URL fragments are not supported in #[route]; use a manual Routable impl",
        ));
    }

    // Split off an optional `?query` declaration, e.g. `/search?q&page&tag`.
    let (path_pattern, query_pattern) = match pattern.split_once('?') {
        Some((path, query)) => (path, Some(query)),
        None => (pattern.as_str(), None),
    };

    let mut parts = Vec::new();
    let mut params = Vec::new();
    let mut cursor = 0;
    while let Some(start_offset) = path_pattern[cursor..].find('<') {
        let start = cursor + start_offset;
        if start > cursor {
            parts.push(RoutePart::Const(path_pattern[cursor..start].to_owned()));
        }
        let end = path_pattern[start + 1..]
            .find('>')
            .map(|offset| start + 1 + offset)
            .ok_or_else(|| syn::Error::new(lit.span(), "route parameter is missing closing '>'"))?;
        let raw = &path_pattern[start + 1..end];
        let param = parse_route_param_name(raw, lit)?;
        if params.iter().any(|existing: &RouteParam| existing.field == param.field) {
            return Err(syn::Error::new(lit.span(), "route parameter field names must be unique"));
        }
        parts.push(RoutePart::Param(param.clone()));
        params.push(param);
        cursor = end + 1;
    }
    if cursor < path_pattern.len() {
        parts.push(RoutePart::Const(path_pattern[cursor..].to_owned()));
    }

    let queries = parse_query_pattern(query_pattern, lit, &params)?;

    Ok(RoutePattern {
        path_literal: path_pattern.to_owned(),
        parts,
        params,
        queries,
    })
}

/// Parse the `?a&b=field&c` portion of a `#[route]` pattern into query params.
///
/// Each entry is either a bare name (`q`) where the query key and the variant
/// field share the same identifier, or `key=field` to bind a differently named
/// query key onto a field.
fn parse_query_pattern(query: Option<&str>, lit: &LitStr, params: &[RouteParam]) -> syn::Result<Vec<QueryParam>> {
    let Some(query) = query else {
        return Ok(Vec::new());
    };

    let mut queries: Vec<QueryParam> = Vec::new();
    for entry in query.split('&') {
        let entry = entry.trim();
        if entry.is_empty() {
            continue;
        }
        let (key, field) = match entry.split_once('=') {
            Some((key, field)) => (key.trim(), field.trim()),
            None => (entry, entry),
        };
        if key.is_empty() || field.is_empty() {
            return Err(syn::Error::new(lit.span(), "query parameter declarations cannot be empty"));
        }
        syn::parse_str::<Ident>(field).map_err(|_| {
            syn::Error::new(
                lit.span(),
                "query parameter field names used by derive must be valid Rust field identifiers",
            )
        })?;
        if params.iter().any(|p| p.field == field) {
            return Err(syn::Error::new(lit.span(), "query parameter field collides with a path parameter field"));
        }
        if queries.iter().any(|q| q.field == field) {
            return Err(syn::Error::new(lit.span(), "query parameter field names must be unique"));
        }
        queries.push(QueryParam {
            key: key.to_owned(),
            field: field.to_owned(),
        });
    }
    Ok(queries)
}

fn parse_route_param_name(raw: &str, lit: &LitStr) -> syn::Result<RouteParam> {
    let key = raw.split_once(':').map(|(name, _)| name).unwrap_or(raw).trim();
    if key.is_empty() {
        return Err(syn::Error::new(lit.span(), "route parameter name cannot be empty"));
    }
    let catch_all = key.starts_with('*');
    let field = key.trim_start_matches('*').trim_start_matches(['+', '?']).to_owned();
    if field.is_empty() {
        return Err(syn::Error::new(
            lit.span(),
            "wildcard route parameters used by derive need a field name, such as <**path>",
        ));
    }
    syn::parse_str::<Ident>(&field)
        .map_err(|_| syn::Error::new(lit.span(), "route parameter names used by derive must be valid Rust field identifiers"))?;
    Ok(RouteParam {
        key: key.to_owned(),
        field,
        catch_all,
    })
}

fn to_url_arm(enum_ident: &Ident, variant_ident: &Ident, fields: &Fields, pattern: &RoutePattern, krate: &TokenStream2) -> syn::Result<TokenStream2> {
    let bindings = route_bindings(fields, pattern)?;
    let pat = bindings.pattern(enum_ident, variant_ident);
    let pushes = pattern.parts.iter().map(|part| match part {
        RoutePart::Const(value) => quote! {
            __glory_url.push_str(#value);
        },
        RoutePart::Param(param) => {
            let binding = bindings.binding_for(&param.field);
            if param.catch_all {
                quote! {
                    __glory_url.push_str(&#krate::encode_catch_all(#binding));
                }
            } else {
                quote! {
                    __glory_url.push_str(&#krate::encode_route_param(#binding));
                }
            }
        }
    });

    let query_pushes = pattern.queries.iter().map(|query| {
        let key = &query.key;
        let binding = bindings.binding_for(&query.field);
        match bindings.field_type(&query.field) {
            Some(ty) if is_vec_type(ty) => quote! {
                for __glory_value in #binding {
                    #krate::append_route_query_param(&mut __glory_url, #key, __glory_value);
                }
            },
            Some(ty) if is_option_type(ty) => quote! {
                if let ::std::option::Option::Some(__glory_value) = #binding {
                    #krate::append_route_query_param(&mut __glory_url, #key, __glory_value);
                }
            },
            _ => quote! {
                #krate::append_route_query_param(&mut __glory_url, #key, #binding);
            },
        }
    });

    Ok(quote! {
        #pat => {
            let mut __glory_url = ::std::string::String::new();
            #(#pushes)*
            #(#query_pushes)*
            __glory_url
        }
    })
}

fn from_url_check(
    enum_ident: &Ident,
    variant_ident: &Ident,
    fields: &Fields,
    pattern: &RoutePattern,
    krate: &TokenStream2,
) -> syn::Result<TokenStream2> {
    let builders = route_builders(fields, pattern, krate)?;
    let build = builders.constructor(enum_ident, variant_ident);
    let path_literal = &pattern.path_literal;
    Ok(quote! {
        if let ::std::option::Option::Some(__glory_matched) = #krate::match_route_pattern(url, #path_literal) {
            return ::std::option::Option::Some(#build);
        }
    })
}

fn not_found_to_url_arm(enum_ident: &Ident, variant_ident: &Ident, fields: &Fields) -> syn::Result<TokenStream2> {
    match fields {
        Fields::Unit => Ok(quote! {
            #enum_ident::#variant_ident => "/".to_owned(),
        }),
        Fields::Named(fields) if fields.named.len() == 1 => {
            let ident = fields.named.first().and_then(|field| field.ident.as_ref()).unwrap();
            Ok(quote! {
                #enum_ident::#variant_ident { #ident } => #ident.to_string(),
            })
        }
        Fields::Unnamed(fields) if fields.unnamed.len() == 1 => Ok(quote! {
            #enum_ident::#variant_ident(__glory_raw_url) => __glory_raw_url.to_string(),
        }),
        _ => Err(syn::Error::new(
            variant_ident.span(),
            "#[not_found] without #[route] supports unit variants or a single raw-url field",
        )),
    }
}

fn not_found_check(enum_ident: &Ident, variant_ident: &Ident, fields: &Fields) -> syn::Result<TokenStream2> {
    match fields {
        Fields::Unit => Ok(quote! {
            ::std::option::Option::Some(#enum_ident::#variant_ident)
        }),
        Fields::Named(fields) if fields.named.len() == 1 => {
            let ident = fields.named.first().and_then(|field| field.ident.as_ref()).unwrap();
            Ok(quote! {
                ::std::option::Option::Some(#enum_ident::#variant_ident {
                    #ident: ::std::convert::From::from(url.to_owned()),
                })
            })
        }
        Fields::Unnamed(fields) if fields.unnamed.len() == 1 => Ok(quote! {
            ::std::option::Option::Some(#enum_ident::#variant_ident(::std::convert::From::from(url.to_owned())))
        }),
        _ => Err(syn::Error::new(
            variant_ident.span(),
            "#[not_found] supports unit variants or a single raw-url field",
        )),
    }
}

struct RouteBindings {
    kind: BindingKind,
    /// Field name -> declared type, for named variants (used by query encoding).
    field_types: Vec<(String, Type)>,
}

enum BindingKind {
    Unit,
    Named(Vec<Ident>),
    Unnamed(Vec<Ident>),
}

impl RouteBindings {
    fn pattern(&self, enum_ident: &Ident, variant_ident: &Ident) -> TokenStream2 {
        match &self.kind {
            BindingKind::Unit => quote! { #enum_ident::#variant_ident },
            BindingKind::Named(fields) => quote! { #enum_ident::#variant_ident { #(#fields),* } },
            BindingKind::Unnamed(fields) => quote! { #enum_ident::#variant_ident( #(#fields),* ) },
        }
    }

    /// Token for reading the value bound to `field` inside a match arm.
    fn binding_for(&self, field: &str) -> TokenStream2 {
        match &self.kind {
            BindingKind::Unit => unreachable!("unit variants cannot have route params"),
            BindingKind::Named(_) => {
                let ident = format_ident!("{}", field);
                quote! { #ident }
            }
            BindingKind::Unnamed(fields) => {
                let target = format_ident!("__glory_field_{}", field);
                let index = fields.iter().position(|ident| ident == &target).unwrap_or(0);
                let ident = &fields[index];
                quote! { #ident }
            }
        }
    }

    fn field_type(&self, field: &str) -> Option<&Type> {
        self.field_types.iter().find(|(name, _)| name == field).map(|(_, ty)| ty)
    }
}

fn route_bindings(fields: &Fields, pattern: &RoutePattern) -> syn::Result<RouteBindings> {
    let params = &pattern.params;
    match fields {
        Fields::Unit => {
            if !params.is_empty() || !pattern.queries.is_empty() {
                return Err(syn::Error::new_spanned(fields, "unit route variants cannot contain route parameters"));
            }
            Ok(RouteBindings {
                kind: BindingKind::Unit,
                field_types: Vec::new(),
            })
        }
        Fields::Named(fields) => {
            let mut idents = Vec::new();
            let mut field_types = Vec::new();
            for field in &fields.named {
                let ident = field.ident.clone().unwrap();
                if !pattern.field_names().any(|name| ident == name) {
                    return Err(syn::Error::new(
                        ident.span(),
                        "field does not appear in the #[route] path or query pattern",
                    ));
                }
                field_types.push((ident.to_string(), field.ty.clone()));
                idents.push(ident);
            }
            for name in pattern.field_names() {
                if !idents.iter().any(|ident| ident == name) {
                    return Err(syn::Error::new_spanned(fields, "route parameter has no matching field"));
                }
            }
            Ok(RouteBindings {
                kind: BindingKind::Named(idents),
                field_types,
            })
        }
        Fields::Unnamed(fields) => {
            if !pattern.queries.is_empty() {
                return Err(syn::Error::new_spanned(
                    fields,
                    "tuple route variants do not support query parameters; use a named-field variant",
                ));
            }
            if fields.unnamed.len() != params.len() {
                return Err(syn::Error::new_spanned(
                    fields,
                    "tuple route variants must have the same number of fields as path parameters",
                ));
            }
            Ok(RouteBindings {
                kind: BindingKind::Unnamed(params.iter().map(|param| format_ident!("__glory_field_{}", param.field)).collect()),
                field_types: Vec::new(),
            })
        }
    }
}

struct RouteBuilders {
    kind: BuilderKind,
}

enum BuilderKind {
    Unit,
    Named(Vec<(Ident, TokenStream2)>),
    Unnamed(Vec<TokenStream2>),
}

impl RouteBuilders {
    fn constructor(&self, enum_ident: &Ident, variant_ident: &Ident) -> TokenStream2 {
        match &self.kind {
            BuilderKind::Unit => quote! { #enum_ident::#variant_ident },
            BuilderKind::Named(fields) => {
                let names = fields.iter().map(|(name, _)| name);
                let values = fields.iter().map(|(_, value)| value);
                quote! { #enum_ident::#variant_ident { #(#names: #values),* } }
            }
            BuilderKind::Unnamed(values) => quote! { #enum_ident::#variant_ident( #(#values),* ) },
        }
    }
}

fn route_builders(fields: &Fields, pattern: &RoutePattern, krate: &TokenStream2) -> syn::Result<RouteBuilders> {
    let params = &pattern.params;
    match fields {
        Fields::Unit => Ok(RouteBuilders { kind: BuilderKind::Unit }),
        Fields::Named(fields) => {
            let mut builders = Vec::new();
            for field in &fields.named {
                let ident = field.ident.clone().unwrap();
                if let Some(param) = params.iter().find(|param| ident == param.field) {
                    builders.push((ident, param_parse_expr(param, &field.ty, krate)));
                } else if let Some(query) = pattern.queries.iter().find(|query| ident == query.field) {
                    builders.push((ident, query_parse_expr(query, &field.ty, krate)));
                } else {
                    return Err(syn::Error::new(
                        ident.span(),
                        "field does not appear in the #[route] path or query pattern",
                    ));
                }
            }
            Ok(RouteBuilders {
                kind: BuilderKind::Named(builders),
            })
        }
        Fields::Unnamed(fields) => {
            if fields.unnamed.len() != params.len() {
                return Err(syn::Error::new_spanned(
                    fields,
                    "tuple route variants must have the same number of fields as path parameters",
                ));
            }
            let builders = fields
                .unnamed
                .iter()
                .zip(params)
                .map(|(field, param)| param_parse_expr(param, &field.ty, krate))
                .collect();
            Ok(RouteBuilders {
                kind: BuilderKind::Unnamed(builders),
            })
        }
    }
}

fn param_parse_expr(param: &RouteParam, ty: &Type, krate: &TokenStream2) -> TokenStream2 {
    let key = &param.key;
    if param.catch_all
        && let Some(inner) = vec_inner_type(ty)
    {
        quote! {
            #krate::parse_catch_all::<#inner>(
                __glory_matched.params().get(#key).map(::std::string::String::as_str).unwrap_or_default()
            ).ok()?
        }
    } else {
        quote! {
            __glory_matched.param(#key).ok()?
        }
    }
}

/// Build the expression that reads a derived query parameter from the matched
/// route. `Vec<T>` fields collect repeated values, `Option<T>` fields parse an
/// optional value, and every other type requires a single value.
fn query_parse_expr(query: &QueryParam, ty: &Type, krate: &TokenStream2) -> TokenStream2 {
    let key = &query.key;
    if let Some(inner) = vec_inner_type(ty) {
        quote! {
            #krate::repeated_query_param::<#inner>(__glory_matched.query(), #key).ok()?
        }
    } else if let Some(inner) = option_inner_type(ty) {
        quote! {
            #krate::optional_query_param::<#inner>(__glory_matched.query(), #key).ok()?
        }
    } else {
        quote! {
            #krate::required_query_param(__glory_matched.query(), #key).ok()?
        }
    }
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

fn is_vec_type(ty: &Type) -> bool {
    vec_inner_type(ty).is_some()
}

fn is_option_type(ty: &Type) -> bool {
    option_inner_type(ty).is_some()
}

fn vec_inner_type(ty: &Type) -> Option<&Type> {
    generic_inner_type(ty, "Vec")
}

fn option_inner_type(ty: &Type) -> Option<&Type> {
    generic_inner_type(ty, "Option")
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
