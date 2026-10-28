use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{FnArg, ItemFn, Pat, Type, spanned::Spanned};

pub fn expand(args: TokenStream, item: TokenStream) -> syn::Result<TokenStream> {
    let facade = if args.is_empty() {
        syn::parse_quote!(::glory)
    } else {
        syn::parse2::<syn::Path>(args)?
    };
    let api = quote!(#facade::display);
    let function = syn::parse2::<ItemFn>(item)?;
    let signature = &function.sig;
    if !signature.generics.params.is_empty()
        || signature.generics.where_clause.is_some()
        || signature.asyncness.is_some()
        || signature.unsafety.is_some()
        || signature.constness.is_some()
        || signature.abi.is_some()
        || signature.variadic.is_some()
    {
        return Err(syn::Error::new(
            signature.span(),
            "component requires a synchronous function with concrete Props types",
        ));
    }
    let name = &signature.ident;
    let props = format_ident!("{name}Props");
    let visibility = &function.vis;
    let attrs = &function.attrs;
    let body = &function.block;
    let mut fields = Vec::new();
    let mut names = Vec::new();
    let mut scope = format_ident!("_scope");
    let mut scope_type = None;
    for (index, arg) in signature.inputs.iter().enumerate() {
        let FnArg::Typed(arg) = arg else {
            return Err(syn::Error::new(arg.span(), "component cannot have self"));
        };
        let Pat::Ident(pattern) = arg.pat.as_ref() else {
            return Err(syn::Error::new(
                arg.pat.span(),
                "component props require named parameters",
            ));
        };
        if pattern.by_ref.is_some() || pattern.subpat.is_some() {
            return Err(syn::Error::new(
                pattern.span(),
                "component props require plain parameter names",
            ));
        }
        let ident = &pattern.ident;
        let ty = &arg.ty;
        if index == 0 && ident == "scope" {
            if !matches!(ty.as_ref(),Type::Reference(r) if matches!(r.elem.as_ref(),Type::Path(p) if p.path.segments.last().is_some_and(|s|s.ident=="Scope")))
            {
                return Err(syn::Error::new(ty.span(), "injected scope must be &Scope"));
            }
            scope = ident.clone();
            scope_type = Some(ty);
            continue;
        }
        if matches!(ty.as_ref(), Type::ImplTrait(_)) {
            return Err(syn::Error::new(
                ty.span(),
                "component Props require a concrete type; use ChildContent for children",
            ));
        }
        fields.push(quote!(pub #ident:#ty));
        names.push((ident, pattern.mutability));
    }
    let take = names
        .iter()
        .map(|(name, mutable)| quote!(let #mutable #name=props.#name;));
    let check_scope = scope_type.map(|ty| quote!(let #scope:#ty=#scope;));
    Ok(quote! {
        #(#attrs)* #visibility struct #name;
        #visibility struct #props {#(#fields),*}
        impl #api::Component for #name {
            type Props=#props;
            fn view(#scope:&#api::Scope,props:Self::Props)->impl #api::IntoView {#check_scope #(#take)* #body}
        }
    })
}
