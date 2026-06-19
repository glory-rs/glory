//! Proc macros for Glory.

use proc_macro::TokenStream;
use quote::quote;
use syn::spanned::Spanned;
use syn::{Expr, FnArg, ItemFn, LitStr, Pat, parse_macro_input};

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
#[proc_macro_attribute]
pub fn server(attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut item_fn = parse_macro_input!(item as ItemFn);

    let mut endpoint: Option<String> = None;
    let mut method = "POST".to_owned();
    let mut encoding = "json".to_owned();
    let mut middlewares = Vec::<Expr>::new();
    if !attr.is_empty() {
        let parser = syn::meta::parser(|meta| {
            if meta.path.is_ident("endpoint") {
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
                    "unsupported #[server] option; expected `endpoint = \"...\"`, `method = \"GET\"`, `encoding = \"cbor\"`, `encoding = \"postcard\"`, or `middleware = path`",
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
