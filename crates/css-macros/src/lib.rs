//! This crate is strictly for internal usage of wbp and use to generate properties structs based on the json description. See `props.json` and `props.rs` for example usage.

use std::{env::current_dir, fs::File, io::read_to_string};

use chumsky::Parser as _;
use convert_case::{Case, Casing};
use css_vds_parser::*;
use ir::{build_ir, IrItem, IrType, IrVariant};
use litrs::Literal;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use serde::Deserialize;

/// Yes, we need intermediate representation to properly build structures, enums, and parsers for them.
mod ir;

#[derive(Deserialize, Debug, PartialEq)]
pub(crate) struct Property {
    name: String,
    values: String,
    initial_value: String,
    inherited: bool,
}

pub(crate) type Properties = Vec<Property>;

pub(crate) fn gen_parser_body(_item: &IrItem) -> TokenStream {
    quote! {
        Err(())
    }
}

pub(crate) fn gen_parser(name: &str, item: &IrItem) -> TokenStream {
    let parser_body = gen_parser_body(item);

    quote! {
        impl<'i> ParseableProperty<'i> for #name {
            fn parse<'t>(input: &mut Parser<'i, 't>) -> Result<Self, ()> {
                #parser_body
                // input
                //     .try_parse(Length::parse)
                //     .map(MarginWidth::Length)
                //     .or_else(|_| {
                //         input
                //             .try_parse(Percentage::parse)
                //             .map(MarginWidth::Percentage)
                //     })
                //     .or_else(|_| {
                //         input
                //             .try_parse(|input| input.expect_ident_matching("auto"))
                //             .map(|_| MarginWidth::Auto)
                //             .map_err(|_| ())
                //     })
            }
        }
    }
}

pub(crate) fn gen_type(ty: &IrType) -> TokenStream {
    match ty {
        IrType::Leaf(s) | IrType::Named(s) => {
            let s = format_ident!("{}", s.to_case(Case::Pascal));
            quote! {
                #s
            }
        }
        IrType::Repetition { inner, min, max } => match (min, max) {
            (0, None) => {
                // means `*` (ZeroOrMore)
                let name = gen_type(inner);
                quote! {Vec<#name>}
            }
            (1, None) => {
                // means `+` (OneOrMore)
                let name = gen_type(inner);
                // In this case we have nonzero vector.
                quote! {Vec<#name>}
            }
            (0, Some(1)) => {
                // means `?` (Optional)
                let name = gen_type(inner);
                // In this case we have nonzero vector.
                quote! {Option<#name>}
            }
            (_m, Some(_n)) => {
                let name = gen_type(inner);
                // In this case we have vector `vec` with `m <= vec.len() <= n`.
                quote! {Vec<#name>}
            }
            _ => unreachable!(),
        },
    }
}

pub(crate) fn gen_variant(v: &IrVariant) -> TokenStream {
    let IrVariant { name, payload } = dbg!(v);
    let name = format_ident!("{}", name.to_case(Case::Pascal));
    let mut def = quote! {};
    if let Some(ty) = payload {
        def = gen_type(ty);
        def = quote! {(#def)};
    }
    quote! {
        #name #def
    }
}

pub(crate) fn gen_declaration(item: &IrItem) -> TokenStream {
    match item {
        IrItem::Struct(ir::IrStruct {
            name,
            order: _, // We don't need order to emit struct.
            fields,
        }) => {
            let fields = fields.iter().map(|ir::IrField { name, ty }| {
                let ty = gen_type(ty);
                let name = format_ident!("{}", name.to_case(Case::Snake));
                quote! {
                    pub #name: #ty
                }
            });
            quote! {
                // #[derive(Debug, Clone, Copy, PartialEq, Eq)]
                pub struct #name {
                    #(#fields),*
                }
            }
        }
        IrItem::Enum(ir::IrEnum { name, variants }) => {
            let variants = variants.iter().map(gen_variant);
            let name = format_ident!("{}", name.to_case(Case::Pascal));

            quote! {
                // #[derive(Debug, Clone, Copy, PartialEq, Eq)]
                pub enum #name {
                    #(#variants),*
                }
            }
        }
    }
}

pub(crate) fn gen_property(i: u8, prop: &Property) -> TokenStream {
    let name = prop.name.to_case(Case::Pascal);
    let ident = format_ident!("{}", name);

    // // We just unwrap, since it just a macro and not an actual code that will be executed in the browser.
    let value_definition_syntax = css_value_parser().parse(&prop.values).unwrap();

    let mut items = Vec::new();
    let _ = build_ir(&prop.name, &value_definition_syntax, &mut items);

    let _parsers = items.iter().map(|i| gen_parser(&name, i));

    let decls = items.iter().map(gen_declaration);

    quote! {
        #(#decls)*

        impl Indexable for #ident {
            const ID: PropIndex = #i;
        }

        // #(#parsers)*
    }
}

#[proc_macro]
pub fn css_properties(tokens: proc_macro::TokenStream) -> proc_macro::TokenStream {
    println!(
        "The current directory is {}",
        current_dir().unwrap().display()
    );

    let mut tokens_iter = tokens.into_iter();
    // TODO: figure out how to handle this properly.
    let first_token = tokens_iter.next().expect("Expected one string literal.");
    if tokens_iter.count() != 0 {
        panic!("Expected one string literal.");
    }

    let Ok(Literal::String(string_lit)) = Literal::try_from(first_token) else {
        panic!("Expected one string literal.")
    };

    let path_str = string_lit.value();
    let file = File::open(path_str).unwrap();
    let file_content = read_to_string(file).unwrap();

    let props_json = json5::from_str::<Properties>(&file_content).unwrap();

    let props = props_json
        .iter()
        .enumerate()
        .map(|(i, p)| gen_property(i as u8, p))
        .collect::<Vec<_>>();

    let props_names = props_json
        .iter()
        .map(|p| p.name.as_str())
        .collect::<Vec<_>>();

    let prop_union_variants = props_names
        .iter()
        .map(|name| {
            let variant = format_ident!("{}", name.to_case(Case::Snake));
            let r#type = format_ident!("{}", name.to_case(Case::Pascal));
            quote! {
                #variant: #r#type
            }
        })
        .collect::<Vec<_>>();

    let prop_to_from_union = props_names
        .iter()
        .map(|name| {
            let variant = format_ident!("{}", name.to_case(Case::Snake));
            let r#type = format_ident!("{}", name.to_case(Case::Pascal));

            quote! {
                impl From<PropUnion> for #r#type {
                    fn from(value: PropUnion) -> Self {
                        unsafe {
                            value.#variant
                        }
                    }
                }

                impl From<#r#type> for PropUnion {
                    fn from(value: #r#type) -> Self {
                        PropUnion {
                            #variant: value
                        }
                    }
                }
            }
        })
        .collect::<Vec<_>>();

    quote! {
        #(#props)*

        #[derive(Clone, Copy)]
        union PropUnion {
            #(#prop_union_variants),*
        }

        #(#prop_to_from_union)*
    }
    .into()
}
