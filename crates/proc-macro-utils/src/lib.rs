extern crate proc_macro;

use std::iter;

use either::Either;
use proc_macro::TokenStream;
use quote::quote;
use syn::{
    Attribute, Ident, Token, Type, Visibility, braced, bracketed, parse::Parse, parse_macro_input,
    punctuated::Punctuated, token::Bracket,
};

struct HirContainer {
    attrs: Vec<Attribute>,
    vis: Visibility,
    container_name: Ident,
    fields: Punctuated<HirField, Token![,]>,
}

impl Parse for HirContainer {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let attrs = input.call(Attribute::parse_outer)?;
        let vis = input.parse()?;

        input.parse::<Token![struct]>()?;
        let container_name = input.parse()?;

        let content;
        braced!(content in input);
        let fields = content.parse_terminated(|input| input.parse::<HirField>(), Token![,])?;

        Ok(Self { attrs, vis, container_name, fields })
    }
}

struct HirField {
    name: Ident,
    ty: HirFieldType,
    access: Option<Punctuated<(Type, Type), Token![,]>>,
}

impl Parse for HirField {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let name = input.parse()?;
        input.parse::<Token![:]>()?;

        let ty = input.parse()?;

        let access = if input.peek(Token![=>]) {
            input.parse::<Token![=>]>()?;

            let content;
            braced!(content in input);

            let access = content.parse_terminated(
                |input| {
                    let content;
                    bracketed!(content in input);

                    let id = content.parse::<Type>()?;
                    content.parse::<Token![|]>()?;
                    let src = content.parse::<Type>()?;
                    Ok((id, src))
                },
                Token![,],
            )?;
            Some(access)
        } else {
            None
        };

        Ok(Self { name, ty, access })
    }
}

// its just a proc-macro, so it's fine to allow this for clarity
#[allow(clippy::large_enum_variant)]
enum HirFieldType {
    Type(Type),
    Arena(Type),
    SourceMap { hir: Type, src: Type },
}

impl Parse for HirFieldType {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let ty = if input.peek(Bracket) {
            let content;
            bracketed!(content in input);
            let ty = content.parse()?;

            if content.peek(Token![|]) {
                content.parse::<Token![|]>()?;
                let src = content.parse()?;
                HirFieldType::SourceMap { hir: ty, src }
            } else {
                HirFieldType::Arena(ty)
            }
        } else {
            let ty = input.parse()?;
            HirFieldType::Type(ty)
        };

        Ok(ty)
    }
}

#[proc_macro]
pub fn define_container(input: TokenStream) -> TokenStream {
    let HirContainer { attrs, vis, container_name, fields, .. } =
        &parse_macro_input!(input as HirContainer);

    let is_arena = fields.iter().any(|HirField { ty, .. }| matches!(ty, HirFieldType::Arena(_)));

    // Generate the fields for the data struct
    let container_fields = fields.iter().map(|HirField { name, ty, .. }| match ty {
        HirFieldType::Type(ty) => {
            quote! { #vis #name: #ty }
        }
        HirFieldType::Arena(ty) => quote! { #vis #name: Arena<#ty> },
        HirFieldType::SourceMap { hir, src } => {
            quote! { #vis #name: SourceMap<#src, #hir> }
        }
    });

    let field_names = fields.iter().flat_map(|HirField { name, ty, access }| match (ty, access) {
        (HirFieldType::Type(_), None) => Either::Left(iter::empty()),
        _ => Either::Right(iter::once(name.clone())),
    });

    let impl_get = fields.iter().flat_map(|HirField { name, ty, access }| {
        match (ty, access) {
            (HirFieldType::Type(_), _) | (_, Some(_)) => Either::Left(iter::empty()),
            (HirFieldType::SourceMap { hir, src }, None) => {
                Either::Right(Either::Left(iter::once(quote! {
                    impl utils::get::Get<#src> for #container_name {
                        type Output = Option<la_arena::Idx<#hir>>;

                        fn get(&self, src: #src) -> Self::Output {
                            self.#name.get(src)
                        }
                    }

                    impl utils::get::Get<la_arena::Idx<#hir>> for #container_name {
                        type Output = Option<#src>;

                        fn get(&self, idx: la_arena::Idx<#hir>) -> Self::Output {
                            self.#name.get(idx)
                        }
                    }
                })))
            }
            (HirFieldType::Arena(ty), None) => Either::Right(Either::Right(iter::once(quote! {
                impl utils::get::GetRef<la_arena::Idx<#ty>> for #container_name {
                    type Output = #ty;

                    fn get(&self, idx: la_arena::Idx<#ty>) -> &Self::Output {
                        self.#name.get(idx)
                    }
                }
            }))),
        }
        .chain(access.iter().flatten().map(move |(id, src)| {
            if is_arena {
                quote! {
                    impl utils::get::GetRef<#id> for #container_name {
                        type Output = #src;

                        fn get(&self, idx: #id) -> &Self::Output {
                            self.#name.get(idx)
                        }
                    }
                }
            } else {
                quote! {
                    impl utils::get::Get<#src> for #container_name {
                        type Output = Option<#id>;

                        fn get(&self, src: #src) -> Self::Output {
                            self.#name.get(src)
                        }
                    }

                    impl utils::get::Get<#id> for #container_name {
                        type Output = Option<#src>;

                        fn get(&self, idx: #id) -> Self::Output {
                            self.#name.get(idx)
                        }
                    }
                }
            }
        }))
    });

    let def = quote! {
        #(#attrs)*
        #vis struct #container_name {
            #(#container_fields,)*
        }

        impl #container_name {
            pub fn shrink_to_fit(&mut self) {
                #(self.#field_names.shrink_to_fit();)*
            }
        }

        #(#impl_get)*
    };

    TokenStream::from(def)
}

struct HirContainerImpl {
    attrs: Vec<Attribute>,
    vis: Visibility,
    containers: Punctuated<(Type, Type), Token![,]>,
    access: Punctuated<HirFieldAccess, Token![,]>,
}

struct HirFieldAccess {
    data_ty: Type,
    data_id_ty: Type,
    src_ty: Type,
}

impl Parse for HirFieldAccess {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let data_ty = input.parse()?;

        let buffer;
        bracketed!(buffer in input);
        let data_id_ty = buffer.parse()?;
        buffer.parse::<Token![|]>()?;
        let src_ty = buffer.parse()?;
        if !buffer.is_empty() {
            return Err(buffer.error("unexpected token"));
        }
        Ok(Self { data_ty, data_id_ty, src_ty })
    }
}

impl Parse for HirContainerImpl {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let attrs = input.call(Attribute::parse_outer)?;
        let vis = input.parse()?;
        input.parse::<Token![enum]>()?;
        let buffer;
        braced!(buffer in input);
        let containers = buffer.parse_terminated(
            |buf| {
                let container = buf.parse()?;
                buf.parse::<Token![|]>()?;
                let src_map = buf.parse()?;
                Ok((container, src_map))
            },
            Token![,],
        )?;
        input.parse::<Token![=>]>()?;
        let buffer;
        braced!(buffer in input);
        let access = buffer.parse_terminated(HirFieldAccess::parse, Token![,])?;
        Ok(Self { attrs, vis, containers, access })
    }
}

#[proc_macro]
pub fn impl_container(input: TokenStream) -> TokenStream {
    let HirContainerImpl { attrs, vis, containers, access } =
        parse_macro_input!(input as HirContainerImpl);

    let container_def = containers.iter().map(|(name, _)| quote! { #name(Arc<#name>), });
    let container_srcmap_def = containers.iter().map(|(_, name)| quote! { #name(Arc<#name>), });

    let impls = access.iter().flat_map(|HirFieldAccess { data_ty, data_id_ty, src_ty }| {
        let data_arms = containers.iter().map(|(name, _)| {
            quote! { Self::#name(it) => it.get(idx), }
        });

        let src_arms = containers.iter().map(|(_, name)| {
            quote! { Self::#name(it) => it.get(src), }
        });

        let src_arms_2 = containers.iter().map(|(_, name)| {
            quote! { Self::#name(it) => it.get(idx), }
        });

        quote! {
            impl utils::get::GetRef<#data_id_ty> for Container {
                type Output = #data_ty;

                fn get(&self, idx: #data_id_ty) -> &Self::Output {
                    match self {
                        #(#data_arms)*
                    }
                }
            }

            impl utils::get::Get<#src_ty> for ContainerSrcMap {
                type Output = Option<#data_id_ty>;

                fn get(&self, src: #src_ty) -> Self::Output {
                    match self {
                        #(#src_arms)*
                    }
                }
            }

            impl utils::get::Get<#data_id_ty> for ContainerSrcMap {
                type Output = Option<#src_ty>;

                fn get(&self, idx: #data_id_ty) -> Self::Output {
                    match self {
                        #(#src_arms_2)*
                    }
                }
            }
        }
    });

    let output = quote! {
        utils::define_enum_deriving_from! {
            #(#attrs)*
            #vis enum Container {
                #(#container_def)*
            }
        }

        utils::define_enum_deriving_from! {
            #(#attrs)*
            #vis enum ContainerSrcMap {
                #(#container_srcmap_def)*
            }
        }

        #(#impls)*
    };
    TokenStream::from(output)
}
