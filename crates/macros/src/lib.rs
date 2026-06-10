//! Proc macros for Glory.

use proc_macro::TokenStream;
use quote::quote;
use syn::spanned::Spanned;
use syn::{FnArg, ItemFn, LitStr, Pat, parse_macro_input};

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
/// - **wasm builds** replace the body with a `fetch` POST to that endpoint;
///   arguments are serialized as a JSON tuple, the result deserialized from
///   the response body.
///
/// Requirements: the function must be `async`, take owned serializable
/// arguments with plain identifier patterns, and return
/// `Result<T, ServerFnError>`.
///
/// `#[server(endpoint = "custom-name")]` overrides the URL segment when two
/// functions in different modules would otherwise collide on the name.
#[proc_macro_attribute]
pub fn server(attr: TokenStream, item: TokenStream) -> TokenStream {
    let item_fn = parse_macro_input!(item as ItemFn);

    let mut endpoint: Option<String> = None;
    if !attr.is_empty() {
        let parser = syn::meta::parser(|meta| {
            if meta.path.is_ident("endpoint") {
                let value: LitStr = meta.value()?.parse()?;
                endpoint = Some(value.value());
                Ok(())
            } else {
                Err(meta.error("unsupported #[server] option; expected `endpoint = \"...\"`"))
            }
        });
        parse_macro_input!(attr with parser);
    }

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

    let expanded = quote! {
        #[cfg(not(target_arch = "wasm32"))]
        #item_fn

        #[cfg(not(target_arch = "wasm32"))]
        glory_serverfn::inventory::submit! {
            glory_serverfn::ServerFnEntry {
                path: #url,
                handler: |__body: ::std::vec::Vec<u8>| ::std::boxed::Box::pin(async move {
                    let ( #(#arg_idents,)* ): ( #(#arg_types,)* ) = glory_serverfn::decode_args(&__body)?;
                    let __output = #name( #(#arg_idents),* ).await?;
                    glory_serverfn::encode_ok(&__output)
                }),
            }
        }

        #[cfg(target_arch = "wasm32")]
        #vis #sig {
            glory_serverfn::call_remote(#url, &( #(#arg_idents,)* )).await
        }
    };
    expanded.into()
}
