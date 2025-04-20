use convert_case::{Case, Casing};
use std::{env::current_dir, fs::File, io::read_to_string};

use litrs::Literal;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use serde::Deserialize;

#[derive(Deserialize, Debug, PartialEq)]
struct Property {
    name: String,
    values: String,
    initial_value: String,
    inherited: bool,
}

type Properties = Vec<Property>;

fn gen_variant(variant: &str) -> TokenStream {
    // TODO: add support for other syntax.
    variant
        .strip_prefix("<")
        .and_then(|s| s.strip_suffix(">"))
        .map(|s| {
            let ident = format_ident!("{}", s.to_case(Case::Pascal));
            // If we get identifier in form <ident>, then this means
            // that this is built in variant and it require manual
            // implementation, which we should provide alongsise macro
            // invocation.
            quote! {#ident(#ident)}
        })
        .unwrap_or_else(|| {
            // This case means that it is a keyword
            let ident = format_ident!("{}", variant.to_case(Case::Pascal));
            quote! {#ident}
        })
}

fn gen_property(i: u8, prop: &Property) -> TokenStream {
    let name = format_ident!("{}", prop.name.to_case(Case::Pascal));

    let variants = prop
        .values
        .split("|")
        .map(str::trim)
        .map(gen_variant)
        .collect::<Vec<_>>();

    quote! {
        #[derive(Debug, Clone, Copy, PartialEq)]
        pub enum #name {
            #(#variants),*
        }

        impl Indexable for #name {
            const ID: PropIndex = #i;
        }
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
        pub union PropUnion {
            #(#prop_union_variants),*
        }

        #(#prop_to_from_union)*
    }
    .into()
}
