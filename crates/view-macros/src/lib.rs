//! XML syntax which generates the same shared data/configuration as handwritten code.
use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as Tokens};
use quote::{ToTokens, quote, quote_spanned};
use syn::{
    Expr, Ident, LitStr, Token,
    ext::IdentExt,
    parenthesized,
    parse::{Parse, ParseStream},
    spanned::Spanned,
};

struct Input {
    api: Tokens,
    nodes: Vec<Node>,
}
#[derive(Clone)]
enum Value {
    Literal(LitStr),
    Expr(Expr),
    Flag,
}
#[derive(Clone)]
struct Attribute {
    name: String,
    span: Span,
    value: Value,
}
#[derive(Clone)]
enum Node {
    Text(LitStr),
    Expr(Expr),
    Element {
        path: syn::Path,
        attrs: Vec<Attribute>,
        children: Vec<Node>,
        component: bool,
        self_closed: bool,
    },
    Fragment(Vec<Node>),
}
fn path(input: ParseStream<'_>) -> syn::Result<syn::Path> {
    let first: Ident = input.call(Ident::parse_any)?;
    let mut segments = syn::punctuated::Punctuated::new();
    segments.push(first.into());
    while input.peek(Token![::]) {
        let separator = input.parse()?;
        segments.push_punct(separator);
        let next: Ident = input.call(Ident::parse_any)?;
        segments.push_value(next.into());
    }
    Ok(syn::Path {
        leading_colon: None,
        segments,
    })
}
fn nodes(input: ParseStream<'_>, depth: usize) -> syn::Result<Vec<Node>> {
    if depth > 64 {
        return Err(input.error("XML template exceeds depth 64"));
    }
    let mut values = Vec::new();
    while !input.is_empty() && !(input.peek(Token![<]) && input.peek2(Token![/])) {
        values.push(node(input, depth)?);
    }
    Ok(values)
}
fn expression(input: ParseStream<'_>) -> syn::Result<Expr> {
    let block: syn::Block = input.parse()?;
    if block.stmts.len() == 1
        && let syn::Stmt::Expr(value, None) = &block.stmts[0]
    {
        return Ok(value.clone());
    }
    Ok(Expr::Block(syn::ExprBlock {
        attrs: Vec::new(),
        label: None,
        block,
    }))
}
fn node(input: ParseStream<'_>, depth: usize) -> syn::Result<Node> {
    if input.peek(LitStr) {
        return Ok(Node::Text(input.parse()?));
    }
    if input.peek(syn::token::Brace) {
        return Ok(Node::Expr(expression(input)?));
    }
    if !input.peek(Token![<]) {
        return Err(input.error("expected <tag>, quoted text, or {Rust expression}"));
    }
    input.parse::<Token![<]>()?;
    if input.peek(Token![>]) {
        input.parse::<Token![>]>()?;
        let children = nodes(input, depth + 1)?;
        input.parse::<Token![<]>()?;
        input.parse::<Token![/]>()?;
        input.parse::<Token![>]>()?;
        return Ok(Node::Fragment(children));
    }
    let tag = path(input)?;
    let component = tag.segments.len() > 1
        || tag
            .segments
            .last()
            .unwrap()
            .ident
            .to_string()
            .starts_with(char::is_uppercase);
    let mut attrs = Vec::new();
    while !input.peek(Token![>]) && !input.peek(Token![/]) {
        let ident: Ident = input.call(Ident::parse_any)?;
        let span = ident.span();
        let mut name = ident.to_string();
        while input.peek(Token![-]) || input.peek(Token![:]) {
            let separator: proc_macro2::Punct = input.parse()?;
            let part: Ident = input.call(Ident::parse_any)?;
            name.push(separator.as_char());
            name.push_str(&part.to_string());
        }
        let value = if input.peek(Token![=]) {
            input.parse::<Token![=]>()?;
            if input.peek(LitStr) {
                Value::Literal(input.parse()?)
            } else if input.peek(syn::token::Brace) {
                Value::Expr(expression(input)?)
            } else {
                return Err(input.error("attribute values must be quoted or enclosed in braces"));
            }
        } else {
            Value::Flag
        };
        if attrs.iter().any(|a: &Attribute| a.name == name) {
            return Err(syn::Error::new(span, "duplicate XML attribute"));
        }
        attrs.push(Attribute { name, span, value });
    }
    let self_closed = input.peek(Token![/]);
    let children = if self_closed {
        input.parse::<Token![/]>()?;
        input.parse::<Token![>]>()?;
        Vec::new()
    } else {
        input.parse::<Token![>]>()?;
        let children = nodes(input, depth + 1)?;
        input.parse::<Token![<]>()?;
        input.parse::<Token![/]>()?;
        let closing = path(input)?;
        if closing.to_token_stream().to_string() != tag.to_token_stream().to_string() {
            return Err(syn::Error::new(
                closing.span(),
                "closing tag does not match opening tag",
            ));
        }
        input.parse::<Token![>]>()?;
        children
    };
    Ok(Node::Element {
        path: tag,
        attrs,
        children,
        component,
        self_closed,
    })
}
impl Parse for Input {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let content;
        parenthesized!(content in input);
        let api = content.parse()?;
        input.parse::<Token![;]>()?;
        let nodes = nodes(input, 0)?;
        if !input.is_empty() {
            return Err(input.error("unexpected closing tag"));
        }
        if nodes.is_empty() {
            return Err(input.error("empty XML view"));
        }
        Ok(Self { api, nodes })
    }
}
struct Emitter {
    api: Tokens,
    live: bool,
    actions: Vec<Tokens>,
    slots: usize,
    count: usize,
    bytes: usize,
}
impl Emitter {
    fn slot(&mut self, kind: &str) -> syn::Result<Tokens> {
        if self.slots >= 256 {
            return Err(syn::Error::new(
                Span::call_site(),
                "XML template exceeds 256 dynamic targets",
            ));
        }
        let name = LitStr::new(&format!("__glory_slot_{}", self.slots), Span::call_site());
        self.slots += 1;
        let ty = Ident::new(kind, Span::call_site());
        let api = &self.api;
        Ok(quote!(#api::#ty::new(#name)))
    }
    fn value(value: &Value) -> syn::Result<Tokens> {
        match value {
            Value::Literal(v) => Ok(quote!(#v)),
            Value::Expr(e) => Ok(quote!(#e)),
            Value::Flag => Err(syn::Error::new(Span::call_site(), "expected a value")),
        }
    }
    fn closure(expr: &Expr) -> bool {
        match expr {
            Expr::Closure(_) => true,
            Expr::Paren(value) => Self::closure(&value.expr),
            Expr::Group(value) => Self::closure(&value.expr),
            Expr::Block(value) => value
                .block
                .stmts
                .last()
                .is_some_and(|s| matches!(s,syn::Stmt::Expr(expr,None) if Self::closure(expr))),
            _ => false,
        }
    }
    fn bind(&mut self, slot: &Tokens, expr: &Expr) {
        let is_closure = Self::closure(expr);
        if is_closure {
            self.actions
                .push(quote_spanned!(expr.span()=> .watch(#slot,#expr)));
        } else {
            self.actions
                .push(quote_spanned!(expr.span()=> .bind(#slot,#expr)));
        }
    }
    fn component(
        &self,
        path: &syn::Path,
        attrs: &[Attribute],
        children: &[Node],
    ) -> syn::Result<Tokens> {
        let api = &self.api;
        let mut fields = Vec::new();
        for attr in attrs {
            let ident = syn::parse_str::<Ident>(&attr.name).map_err(|_| {
                syn::Error::new(attr.span, "component props must be Rust field names")
            })?;
            let value = Self::value(&attr.value)?;
            fields.push(quote_spanned!(attr.span=>#ident:#value));
        }
        if !children.is_empty() {
            if attrs.iter().any(|a| a.name == "children") {
                return Err(syn::Error::new(path.span(), "children supplied twice"));
            }
            let nested = expand(
                Input {
                    api: api.clone(),
                    nodes: children.to_vec(),
                },
                true,
            )?;
            fields.push(quote!(children:#api::child_content(#api::lazy(move || #nested))));
        }
        Ok(quote!({type __GloryProps=<#path as #api::Component>::Props;__GloryProps{#(#fields),*}}))
    }
    fn emit_nodes(
        &mut self,
        nodes: &[Node],
        parent: Option<&str>,
        in_button: bool,
    ) -> syn::Result<Vec<Tokens>> {
        let mut output = Vec::new();
        for node in nodes {
            if let Node::Fragment(children) = node {
                output.extend(self.emit_nodes(children, parent, in_button)?);
                continue;
            }
            output.push(self.emit(node, parent, in_button)?);
        }
        Ok(output)
    }
    fn emit(&mut self, node: &Node, parent: Option<&str>, in_button: bool) -> syn::Result<Tokens> {
        self.count += 1;
        if self.count > 256 {
            return Err(syn::Error::new(
                Span::call_site(),
                "XML template exceeds 256 static nodes",
            ));
        }
        let api = self.api.clone();
        match node {
            Node::Text(value) => {
                if parent.is_some_and(|p| matches!(p, "table" | "tbody" | "tr"))
                    && !value.value().trim().is_empty()
                {
                    return Err(syn::Error::new(
                        value.span(),
                        "table structure cannot contain direct text",
                    ));
                }
                self.bytes += value.value().len();
                Ok(quote_spanned!(value.span()=> #api::text(#value)))
            }
            Node::Expr(expr) => {
                if parent.is_some_and(|p| matches!(p, "table" | "tbody" | "tr")) {
                    return Err(syn::Error::new(
                        expr.span(),
                        "table structure requires Each/Content or its valid child tags",
                    ));
                }
                if !self.live {
                    return Err(syn::Error::new(
                        expr.span(),
                        "template! declares data; use <field slot={FIELD}/> or view! for values",
                    ));
                }
                let slot = self.slot("Field")?;
                self.bind(&slot, expr);
                Ok(quote_spanned!(expr.span()=>#api::field(#slot)))
            }
            Node::Fragment(_) => unreachable!(),
            Node::Element {
                path,
                attrs,
                children,
                component,
                self_closed,
            } => {
                if *component {
                    if !self.live {
                        return Err(syn::Error::new(path.span(), "components require view!"));
                    }
                    if parent.is_some_and(|p| !matches!(p, "div" | "tbody" | "td")) {
                        return Err(syn::Error::new(
                            path.span(),
                            "nested components require a div parent in this profile",
                        ));
                    }
                    let tag = path.to_token_stream().to_string();
                    if matches!(tag.as_str(), "Each" | "Show" | "Content") {
                        let get = |name: &str| -> syn::Result<Tokens> {
                            attrs
                                .iter()
                                .find(|a| a.name == name)
                                .map(|a| Self::value(&a.value))
                                .unwrap_or_else(|| {
                                    Err(syn::Error::new(
                                        path.span(),
                                        format!("{tag} requires {name}"),
                                    ))
                                })
                        };
                        let allowed: &[&str] = match tag.as_str() {
                            "Each" => &["each", "children"],
                            "Show" => &["when", "children", "fallback"],
                            _ => &["value"],
                        };
                        if let Some(a) = attrs.iter().find(|a| !allowed.contains(&a.name.as_str()))
                        {
                            return Err(syn::Error::new(a.span, "unknown structural property"));
                        }
                        let value = match tag.as_str() {
                            "Each" => {
                                if !self_closed {
                                    return Err(syn::Error::new(
                                        path.span(),
                                        "Each uses children={move |row| view! {...}}",
                                    ));
                                }
                                let source = get("each")?;
                                let factory = get("children")?;
                                quote!(#api::each(#source,#factory))
                            }
                            "Show" => {
                                let when = get("when")?;
                                let yes = if children.is_empty() {
                                    get("children")?
                                } else {
                                    if attrs.iter().any(|a| a.name == "children") {
                                        return Err(syn::Error::new(
                                            path.span(),
                                            "children supplied twice",
                                        ));
                                    }
                                    let nested = expand(
                                        Input {
                                            api: api.clone(),
                                            nodes: children.clone(),
                                        },
                                        true,
                                    )?;
                                    quote!(move || #nested)
                                };
                                let no = attrs
                                    .iter()
                                    .find(|a| a.name == "fallback")
                                    .map(|a| Self::value(&a.value))
                                    .transpose()?
                                    .unwrap_or_else(|| quote!(||#api::empty()));
                                quote!(#api::show(#when,#yes,#no))
                            }
                            _ => {
                                if !self_closed {
                                    return Err(syn::Error::new(
                                        path.span(),
                                        "Content uses value={view}",
                                    ));
                                }
                                get("value")?
                            }
                        };
                        let outlet = self.slot("Outlet")?;
                        self.actions.push(quote!(.child(#outlet,#value)));
                        return Ok(quote!(#api::outlet(#outlet)));
                    }
                    let props = self.component(path, attrs, children)?;
                    let outlet = self.slot("Outlet")?;
                    self.actions
                        .push(quote!(.component::<#path>(#outlet,#props)));
                    return Ok(quote!(#api::outlet(#outlet)));
                }
                let tag = path.segments.last().unwrap().ident.to_string();
                if !self.live && matches!(tag.as_str(), "field" | "outlet") {
                    if !self_closed || attrs.len() != 1 || attrs[0].name != "slot" {
                        return Err(syn::Error::new(
                            path.span(),
                            "slot declaration must be <field slot={FIELD}/> or <outlet slot={OUTLET}/>",
                        ));
                    }
                    if tag == "outlet"
                        && parent.is_some_and(|p| !matches!(p, "div" | "tbody" | "td"))
                    {
                        return Err(syn::Error::new(path.span(), "outlets require a div parent"));
                    }
                    let value = Self::value(&attrs[0].value)?;
                    let function = Ident::new(&tag, path.span());
                    return Ok(quote!(#api::#function(#value)));
                }
                if !matches!(
                    tag.as_str(),
                    "div" | "span" | "button" | "a" | "h1" | "table" | "tbody" | "tr" | "td"
                ) {
                    return Err(syn::Error::new(
                        path.span(),
                        "unsupported HTML tag; current display profile supports div, span and button",
                    ));
                }
                if match parent {
                    Some("table") => tag != "tbody",
                    Some("tbody") => tag != "tr",
                    Some("tr") => tag != "td",
                    Some("span" | "button" | "a" | "h1") => tag != "span",
                    _ => parent.is_some() && matches!(tag.as_str(), "tbody" | "tr" | "td"),
                } || in_button && matches!(tag.as_str(), "button" | "a")
                {
                    return Err(syn::Error::new(path.span(), "invalid HTML content context"));
                }
                let mut attributes = Vec::new();
                let mut emitted_names = Vec::new();
                for attr in attrs {
                    let name = &attr.name;
                    let value = match &attr.value {
                        Value::Flag => quote!(""),
                        _ => Self::value(&attr.value)?,
                    };
                    if name == "on:click" {
                        if matches!(attr.value, Value::Flag | Value::Literal(_)) {
                            return Err(syn::Error::new(
                                attr.span,
                                "click requires {callback} or a Click declaration in template!",
                            ));
                        }
                        let slot = if self.live {
                            let slot = self.slot("Click")?;
                            self.actions
                                .push(quote_spanned!(attr.span=>.on_click(#slot,#value)));
                            slot
                        } else {
                            value
                        };
                        attributes.push(quote!(#api::on_click(#slot)));
                        continue;
                    }
                    if name == "children" {
                        if !matches!(tag.as_str(), "div" | "tbody" | "td") || !children.is_empty() {
                            return Err(syn::Error::new(
                                attr.span,
                                "children outlet requires an empty div",
                            ));
                        }
                        attributes.push(quote!(#api::children(#value)));
                        continue;
                    }
                    if let Some(token) = name.strip_prefix("class:") {
                        if token.is_empty()
                            || token.len() > 128
                            || !token
                                .bytes()
                                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
                        {
                            return Err(syn::Error::new(attr.span, "invalid class toggle token"));
                        }
                        if attrs
                            .iter()
                            .any(|a| a.name == "class" && matches!(a.value, Value::Expr(_)))
                        {
                            return Err(syn::Error::new(
                                attr.span,
                                "class toggles cannot share a dynamic whole class attribute",
                            ));
                        }
                        let Value::Expr(expr) = &attr.value else {
                            return Err(syn::Error::new(
                                attr.span,
                                "class toggle requires {bool source} or a Field in template!",
                            ));
                        };
                        let token = LitStr::new(token, attr.span);
                        let slot = if self.live {
                            let slot = self.slot("Field")?;
                            let value = if Self::closure(expr) {
                                quote!(#api::computed(#expr))
                            } else {
                                quote!(#expr)
                            };
                            self.actions.push(quote!(.class_when(#slot,#value)));
                            slot
                        } else {
                            value
                        };
                        attributes.push(quote!(#api::toggle_class(#token,#slot)));
                        continue;
                    }
                    let actual = name.strip_prefix("field:").unwrap_or(name);
                    if emitted_names.contains(&actual) {
                        return Err(syn::Error::new(
                            attr.span,
                            "duplicate static/dynamic attribute",
                        ));
                    }
                    emitted_names.push(actual);
                    let literal_name = LitStr::new(actual, attr.span);
                    if name.starts_with("field:") {
                        if self.live || !matches!(actual, "class" | "title" | "aria-label") {
                            return Err(syn::Error::new(
                                attr.span,
                                "field:class/title/aria-label is a template! declaration",
                            ));
                        }
                        attributes.push(quote!(#api::bind_attr(#literal_name,#value)));
                        continue;
                    }
                    if let Value::Expr(expr) = &attr.value {
                        if !self.live || !matches!(actual, "class" | "title" | "aria-label") {
                            return Err(syn::Error::new(
                                attr.span,
                                "dynamic attributes support class, title and aria-label in view!",
                            ));
                        }
                        let slot = self.slot("Field")?;
                        self.bind(&slot, expr);
                        attributes.push(quote!(#api::bind_attr(#literal_name,#slot)));
                        continue;
                    }
                    if (!matches!(
                        actual,
                        "id" | "class" | "role" | "title" | "type" | "disabled"
                    ) && !actual.starts_with("aria-"))
                        || !actual.bytes().all(|b| b.is_ascii_lowercase() || b == b'-')
                    {
                        return Err(syn::Error::new(
                            attr.span,
                            "unsupported static HTML attribute",
                        ));
                    }
                    if actual == "disabled" && tag != "button"
                        || matches!(attr.value, Value::Flag) && actual != "disabled"
                    {
                        return Err(syn::Error::new(
                            attr.span,
                            "only button disabled supports a boolean attribute",
                        ));
                    }
                    if actual == "type"
                        && (tag != "button"
                            || !matches!(&attr.value,Value::Literal(v) if v.value()=="button"))
                    {
                        return Err(syn::Error::new(
                            attr.span,
                            "only type=\"button\" is supported",
                        ));
                    }
                    self.bytes += actual.len();
                    if let Value::Literal(v) = &attr.value {
                        self.bytes += v.value().len();
                    }
                    attributes.push(quote!(#api::attr(#literal_name,#value)));
                }
                let nodes = self.emit_nodes(children, Some(&tag), in_button || tag == "button")?;
                let name = LitStr::new(&tag, path.span());
                Ok(
                    quote_spanned!(path.span()=>#api::element(#name,&[#(#attributes),*],&[#(#nodes),*])),
                )
            }
        }
    }
}
fn expand(input: Input, live: bool) -> syn::Result<Tokens> {
    let api = input.api.clone();
    let mut emitter = Emitter {
        api: input.api,
        live,
        actions: Vec::new(),
        slots: 0,
        count: 0,
        bytes: 0,
    };
    if live
        && input.nodes.len() == 1
        && let Node::Element {
            path,
            attrs,
            component: true,
            children,
            ..
        } = &input.nodes[0]
        && !matches!(
            path.to_token_stream().to_string().as_str(),
            "Each" | "Show" | "Content"
        )
    {
        let props = emitter.component(path, attrs, children)?;
        return Ok(quote!(#api::component::<#path>(#props)));
    }
    let nodes = emitter.emit_nodes(&input.nodes, None, false)?;
    if nodes.is_empty() {
        return Err(syn::Error::new(Span::call_site(), "empty XML view"));
    }
    if emitter.bytes > 65536 {
        return Err(syn::Error::new(
            Span::call_site(),
            "XML template exceeds 64 KiB static text",
        ));
    }
    let template = quote!(#api::Template::new(&[#(#nodes),*]));
    if !live {
        return Ok(template);
    }
    let actions = emitter.actions;
    Ok(
        quote!({static __GLORY_TEMPLATE:#api::Template=#template;#api::view(&__GLORY_TEMPLATE)#(#actions)*}),
    )
}
fn run(input: TokenStream, live: bool) -> TokenStream {
    syn::parse::<Input>(input)
        .and_then(|i| expand(i, live))
        .unwrap_or_else(|e| {
            let errors = e.into_compile_error();
            quote!({#errors})
        })
        .into()
}
#[proc_macro]
pub fn expand_view(input: TokenStream) -> TokenStream {
    run(input, true)
}
#[proc_macro]
pub fn expand_template(input: TokenStream) -> TokenStream {
    run(input, false)
}
#[proc_macro]
pub fn expand_field(input: TokenStream) -> TokenStream {
    struct FieldInput {
        expr: Expr,
    }
    impl Parse for FieldInput {
        fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
            let api;
            parenthesized!(api in input);
            let _: Tokens = api.parse()?;
            input.parse::<Token![;]>()?;
            Ok(Self {
                expr: input.parse()?,
            })
        }
    }
    let result=syn::parse::<FieldInput>(input).and_then(|input|{
        let Expr::Field(expr)=input.expr else{return Err(syn::Error::new(Span::call_site(),"field! requires handle.named_field"))};
        let syn::Member::Named(member)=expr.member else{return Err(syn::Error::new(expr.span(),"field! requires a named struct field"))};
        let name=LitStr::new(&member.to_string(),member.span());let base=expr.base;
        Ok(quote!({let __glory_field_base=&#base;__glory_field_base.field(#name,|__value|&__value.#member,|__value|&mut __value.#member)}))
    });
    result.unwrap_or_else(|e| e.into_compile_error()).into()
}
mod component;
#[proc_macro_attribute]
pub fn component(args: TokenStream, item: TokenStream) -> TokenStream {
    component::expand(args.into(), item.into())
        .unwrap_or_else(|e| e.into_compile_error())
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn parse(code: &str, live: bool) -> syn::Result<Tokens> {
        expand(syn::parse_str(code)?, live)
    }
    #[test]
    fn rejects_bad_xml_content_and_unsupported_features() {
        for xml in [
            r#"(api);<div></span>"#,
            r#"(api);<button><button/></button>"#,
            r#"(api);<span><div/></span>"#,
            r#"(api);<input/>"#,
            r#"(api);<div class="a" class={c}/>"#,
            r#"(api);<button on:mouseover={f}/>"#,
            r#"(api);<div children={O}><span/></div>"#,
            r#"(api);<span><Counter/></span>"#,
        ] {
            assert!(parse(xml, true).is_err(), "{xml}");
        }
    }
    #[test]
    fn static_templates_have_no_binding_actions_and_live_inputs_have_one_each() {
        let static_xml = parse(r#"(api);<div class="a"><span>"hi"</span></div>"#, true)
            .unwrap()
            .to_string();
        assert!(!static_xml.contains(". bind"));
        assert!(static_xml.contains("static __GLORY_TEMPLATE"));
        let live=parse(r#"(api);<div class={class}><span>{move || count.get()*2}</span><button on:click={click}>"+"</button></div>"#,true).unwrap().to_string();
        assert_eq!(live.matches(". bind").count(), 1);
        assert_eq!(live.matches(". watch").count(), 1);
        assert_eq!(live.matches(". on_click").count(), 1);
    }
}
