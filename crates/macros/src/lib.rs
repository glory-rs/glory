//! Proc macros for Glory.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::spanned::Spanned;
use syn::{Attribute, Data, DeriveInput, Expr, Fields, FnArg, GenericArgument, Ident, ItemFn, LitStr, Pat, Type, parse_macro_input};

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

fn expand_routable(input: DeriveInput) -> syn::Result<TokenStream2> {
    let enum_data = match &input.data {
        Data::Enum(data) => data,
        _ => {
            return Err(syn::Error::new(input.ident.span(), "#[derive(Routable)] can only be used on enums"));
        }
    };

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
            to_url_arms.push(to_url_arm(&input.ident, &variant.ident, &variant.fields, &pattern)?);
            from_url_checks.push(from_url_check(&input.ident, &variant.ident, &variant.fields, &pattern, &route.value())?);
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
            redirect_checks.push(from_url_check(
                &input.ident,
                &variant.ident,
                &variant.fields,
                &pattern,
                &redirect.value(),
            )?);
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
        impl #impl_generics ::glory::routing::Routable for #name #type_generics #where_clause {
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
    parts: Vec<RoutePart>,
    params: Vec<RouteParam>,
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
    if pattern.contains('?') || pattern.contains('#') {
        return Err(syn::Error::new(
            lit.span(),
            "query strings and fragments are not supported in #[route]; use RouteQuery helpers in a manual Routable impl",
        ));
    }

    let mut parts = Vec::new();
    let mut params = Vec::new();
    let mut cursor = 0;
    while let Some(start_offset) = pattern[cursor..].find('<') {
        let start = cursor + start_offset;
        if start > cursor {
            parts.push(RoutePart::Const(pattern[cursor..start].to_owned()));
        }
        let end = pattern[start + 1..]
            .find('>')
            .map(|offset| start + 1 + offset)
            .ok_or_else(|| syn::Error::new(lit.span(), "route parameter is missing closing '>'"))?;
        let raw = &pattern[start + 1..end];
        let param = parse_route_param_name(raw, lit)?;
        if params.iter().any(|existing: &RouteParam| existing.field == param.field) {
            return Err(syn::Error::new(lit.span(), "route parameter field names must be unique"));
        }
        parts.push(RoutePart::Param(param.clone()));
        params.push(param);
        cursor = end + 1;
    }
    if cursor < pattern.len() {
        parts.push(RoutePart::Const(pattern[cursor..].to_owned()));
    }

    Ok(RoutePattern { parts, params })
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

fn to_url_arm(enum_ident: &Ident, variant_ident: &Ident, fields: &Fields, pattern: &RoutePattern) -> syn::Result<TokenStream2> {
    let bindings = route_bindings(fields, &pattern.params)?;
    let pat = bindings.pattern(enum_ident, variant_ident);
    let pushes = pattern.parts.iter().map(|part| match part {
        RoutePart::Const(value) => quote! {
            __glory_url.push_str(#value);
        },
        RoutePart::Param(param) => {
            let binding = bindings.binding_for(param);
            if param.catch_all {
                quote! {
                    __glory_url.push_str(&::glory::routing::encode_catch_all(#binding));
                }
            } else {
                quote! {
                    __glory_url.push_str(&::glory::routing::encode_route_param(#binding));
                }
            }
        }
    });

    Ok(quote! {
        #pat => {
            let mut __glory_url = ::std::string::String::new();
            #(#pushes)*
            __glory_url
        }
    })
}

fn from_url_check(
    enum_ident: &Ident,
    variant_ident: &Ident,
    fields: &Fields,
    pattern: &RoutePattern,
    route_literal: &str,
) -> syn::Result<TokenStream2> {
    let builders = route_builders(fields, &pattern.params)?;
    let build = builders.constructor(enum_ident, variant_ident);
    Ok(quote! {
        if let ::std::option::Option::Some(__glory_matched) = ::glory::routing::match_route_pattern(url, #route_literal) {
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

    fn binding_for(&self, param: &RouteParam) -> TokenStream2 {
        match &self.kind {
            BindingKind::Unit => unreachable!("unit variants cannot have route params"),
            BindingKind::Named(_) => {
                let ident = format_ident!("{}", param.field);
                quote! { #ident }
            }
            BindingKind::Unnamed(fields) => {
                let index = fields
                    .iter()
                    .position(|ident| ident == &format_ident!("__glory_field_{}", param.field))
                    .unwrap_or(0);
                let ident = &fields[index];
                quote! { #ident }
            }
        }
    }
}

fn route_bindings(fields: &Fields, params: &[RouteParam]) -> syn::Result<RouteBindings> {
    match fields {
        Fields::Unit => {
            if !params.is_empty() {
                return Err(syn::Error::new_spanned(fields, "unit route variants cannot contain path parameters"));
            }
            Ok(RouteBindings { kind: BindingKind::Unit })
        }
        Fields::Named(fields) => {
            let mut idents = Vec::new();
            for field in &fields.named {
                let ident = field.ident.clone().unwrap();
                if !params.iter().any(|param| ident == param.field) {
                    return Err(syn::Error::new(ident.span(), "field does not appear in the #[route] path pattern"));
                }
                idents.push(ident);
            }
            for param in params {
                if !idents.iter().any(|ident| ident == &format_ident!("{}", param.field)) {
                    return Err(syn::Error::new_spanned(fields, "route parameter has no matching field"));
                }
            }
            Ok(RouteBindings {
                kind: BindingKind::Named(idents),
            })
        }
        Fields::Unnamed(fields) => {
            if fields.unnamed.len() != params.len() {
                return Err(syn::Error::new_spanned(
                    fields,
                    "tuple route variants must have the same number of fields as path parameters",
                ));
            }
            Ok(RouteBindings {
                kind: BindingKind::Unnamed(params.iter().map(|param| format_ident!("__glory_field_{}", param.field)).collect()),
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

fn route_builders(fields: &Fields, params: &[RouteParam]) -> syn::Result<RouteBuilders> {
    match fields {
        Fields::Unit => Ok(RouteBuilders { kind: BuilderKind::Unit }),
        Fields::Named(fields) => {
            let mut builders = Vec::new();
            for field in &fields.named {
                let ident = field.ident.clone().unwrap();
                let param = params
                    .iter()
                    .find(|param| ident == param.field)
                    .ok_or_else(|| syn::Error::new(ident.span(), "field does not appear in the #[route] path pattern"))?;
                builders.push((ident, param_parse_expr(param, &field.ty)));
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
                .map(|(field, param)| param_parse_expr(param, &field.ty))
                .collect();
            Ok(RouteBuilders {
                kind: BuilderKind::Unnamed(builders),
            })
        }
    }
}

fn param_parse_expr(param: &RouteParam, ty: &Type) -> TokenStream2 {
    let key = &param.key;
    if param.catch_all
        && let Some(inner) = vec_inner_type(ty)
    {
        quote! {
            ::glory::routing::parse_catch_all::<#inner>(
                __glory_matched.params().get(#key).map(::std::string::String::as_str).unwrap_or_default()
            ).ok()?
        }
    } else {
        quote! {
            __glory_matched.param(#key).ok()?
        }
    }
}

fn vec_inner_type(ty: &Type) -> Option<&Type> {
    let Type::Path(path) = ty else {
        return None;
    };
    let segment = path.path.segments.last()?;
    if segment.ident != "Vec" {
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
