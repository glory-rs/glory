use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{FnArg, ItemFn, Pat, Type, spanned::Spanned};

pub fn expand(args: TokenStream, item: TokenStream) -> syn::Result<TokenStream> {
    let facade: syn::Path = if args.is_empty() {
        syn::parse_quote!(::glory)
    } else {
        syn::parse2(args)?
    };
    let api = quote!(#facade::ui);
    let function = syn::parse2::<ItemFn>(item)?;
    let signature = &function.sig;
    if !signature.generics.params.is_empty()
        || signature.generics.where_clause.is_some()
        || signature.asyncness.is_some()
        || !matches!(signature.safety, syn::Safety::Default)
        || signature.constness.is_some()
        || signature.abi.is_some()
        || signature.variadic.is_some()
    {
        return Err(syn::Error::new(
            signature.span(),
            "component requires a synchronous function with concrete property types",
        ));
    }
    let name = &signature.ident;
    let visibility = &function.vis;
    let attrs = &function.attrs;
    let body = &function.block;
    let mut fields = Vec::new();
    let mut types = Vec::new();
    let mut mutable = Vec::new();
    let mut scope = format_ident!("_scope");
    let mut scope_type = quote!(&#facade::core::Scope);
    for (index, arg) in signature.inputs.iter().enumerate() {
        let FnArg::Typed(arg) = arg else {
            return Err(syn::Error::new(arg.span(), "component cannot have self"));
        };
        let Pat::Ident(pattern) = arg.pat.as_ref() else {
            return Err(syn::Error::new(
                arg.pat.span(),
                "properties require named parameters",
            ));
        };
        if pattern.by_ref.is_some() || pattern.subpat.is_some() {
            return Err(syn::Error::new(
                pattern.span(),
                "properties require plain parameter names",
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
            scope_type = quote!(#ty);
            continue;
        }
        if matches!(ty.as_ref(), Type::ImplTrait(_)) {
            return Err(syn::Error::new(
                ty.span(),
                "properties require a concrete type; use ui::Children for children",
            ));
        }
        fields.push(ident.clone());
        types.push(ty.clone());
        mutable.push(pattern.mutability);
    }
    let generics: Vec<_> = (0..fields.len())
        .map(|n| format_ident!("__GloryP{n}"))
        .collect();
    let defaults = generics.iter().map(|g| quote!(#g=#api::Missing));
    let declaration = if generics.is_empty() {
        quote!()
    } else {
        quote!(<#(#defaults),*>)
    };
    let parameters = if generics.is_empty() {
        quote!()
    } else {
        quote!(<#(#generics),*>)
    };
    let ready = if types.is_empty() {
        quote!()
    } else {
        quote!(<#(#types),*>)
    };
    let stored = fields
        .iter()
        .zip(&generics)
        .map(|(f, g)| quote!(#f: ::core::option::Option<#g>));
    let empty = fields.iter().map(|f| quote!(#f:None));
    let take = fields
        .iter()
        .map(|f| quote!(let #f=self.#f.take().ok_or(#api::Error::Gone)?;));
    let output = &signature.output;
    let arguments = fields
        .iter()
        .zip(&types)
        .zip(&mutable)
        .map(|((field, ty), mutable)| quote!(#mutable #field:#ty));
    let setters = fields
        .iter()
        .zip(&types)
        .enumerate()
        .map(|(index, (field, ty))| {
            let method = if field == "children" {
                format_ident!("fill")
            } else {
                field.clone()
            };
            let mut result = generics.iter().map(|g| quote!(#g)).collect::<Vec<_>>();
            result[index] = quote!(#ty);
            let result = quote!(<#(#result),*>);
            let values = fields.iter().enumerate().map(|(n, f)| {
                if n == index {
                    quote!(#f:Some(value))
                } else {
                    quote!(#f:self.#f)
                }
            });
            let (argument, convert) = if field == "children" {
                (
                    quote!(value:impl #api::IntoChildren),
                    quote!(let value=#api::IntoChildren::into_children(value);),
                )
            } else {
                (quote!(value:#ty), quote!())
            };
            quote!(pub fn #method(self,#argument)->#name #result {#convert #name{#(#values),*}})
        });
    Ok(quote! {
        #(#attrs)* #visibility struct #name #declaration {#(#stored),*}
        impl ::core::default::Default for #name {fn default()->Self {Self{#(#empty),*}}}
        impl #parameters #name #parameters {#(#setters)*}
        impl #api::Widget for #name #ready {
            fn compose(&mut self,#scope:&#facade::core::Scope)->#api::Result<::core::option::Option<::std::boxed::Box<dyn #api::Widget>>> {
                #(#take)*
                Ok(Some(Box::new(Self::__glory_body(#scope,#(#fields),*))))
            }
        }
        impl #name #ready {
            fn __glory_body(#scope:#scope_type,#(#arguments),*) #output #body
        }
    })
}
