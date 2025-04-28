//! This crate is strictly for internal usage of wbp and use to generate properties structs based on the json description. See `props.json` and `props.rs` for example usage.

use std::{fs::File, io::read_to_string};

use chumsky::Parser as _;
use convert_case::{Case, Casing};
use css_vds_parser::*;
use ir::*;
use litrs::Literal;
use proc_macro2::Ident;
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

pub(crate) fn gen_struct_parser_body(ir_struct: &IrStruct) -> TokenStream {
    fn rec_gen_parser(ty: &IrType, from_rep: bool) -> TokenStream {
        match ty {
            IrType::Leaf(predef_ty) | IrType::Named(predef_ty) => {
                let predef_ty_ident = format_ident!("{}", predef_ty);
                let mut res = quote! {
                    input.try_parse(#predef_ty_ident::parse)
                };
                if !from_rep {
                    res = quote! {
                        #res.map_err(|_| ())
                    }
                }
                res
            }
            IrType::Repetition { inner, min, max } => {
                let inner_parser = rec_gen_parser(inner, true);
                let max = max.unwrap_or(usize::MAX);
                let ret_val = match (min, max) {
                    (0, 1) => quote! {
                        res.first().cloned()
                    },
                    _ => quote! {
                        res
                    },
                };

                // TODO: Move this loop to function.
                quote! {
                    input.try_parse(|input| {
                        let mut i = #min;
                        let mut res = vec![];

                        while let (Ok(v), true) = (#inner_parser, i <= #max) {
                            res.push(v);
                            i += 1;
                        }

                        if (#min <= res.len() && res.len() <= #max) {
                            Ok(#ret_val)
                        } else {
                            Err(())
                        }
                    })
                }
            }
        }
    }

    let struct_name = format_ident!("{}", ir_struct.name);

    let parser = match ir_struct.kind {
        StructKind::OrderedAnd => {
            let fields_parsers = ir_struct.fields.iter().map(|IrField { name, ty }| {
                let name = format_ident!("{}", name);
                let parser = rec_gen_parser(ty, false);
                quote! {
                    let #name = #parser?;
                }
            });

            let fields_ids = ir_struct.fields.iter().map(|f| format_ident!("{}", f.name));

            let constructor = quote! {
                #struct_name {
                    #(#fields_ids),*
                }
            };
            quote! {
                #(#fields_parsers)*

                Ok(#constructor)
            }
        }
        StructKind::UnorderedAnd => todo!("Should be easy to implement, but currently unneeded."),
        StructKind::OneOrMore => {
            let initial_values = ir_struct.fields.iter().map(|f| {
                let name = format_ident!("{}", f.name);
                quote! {
                    let mut #name = None;
                }
            });

            let parsers = ir_struct.fields.iter().map(|f| {
                let name = format_ident!("{}", f.name);
                let field_parser = rec_gen_parser(&f.ty, false);
                quote! {
                    if #name.is_none() {
                        if let Ok(v) = #field_parser {
                            #name = Some(v);
                            any = true;
                            continue;
                        }
                    }
                }
            });

            let fields_ids = ir_struct.fields.iter().map(|f| format_ident!("{}", f.name));

            let constructor = quote! {
                #struct_name {
                    #(#fields_ids),*
                }
            };

            quote! {
                #(#initial_values)*
                let mut any = false;

                loop {
                    #(#parsers)*

                    break;
                }

                if any {
                    Ok(#constructor)
                } else {
                    Err(())
                }
            }
        }
    };

    quote! {
        input.try_parse(|input| {#parser})
    }
}

pub(crate) fn gen_enum_parser_body(IrEnum { name, variants }: &IrEnum) -> TokenStream {
    let enum_name = format_ident!("{}", name);
    let mut variants_parsers = variants.iter().map(|v| {
        // Get something first, fix later they said...
        fn rec_gen_parser(
            enum_name: &Ident,
            variant_name: &Ident,
            payload: Option<&IrType>,
            from_rep: bool,
        ) -> TokenStream {
            match payload {
                Some(ty) => match ty {
                    IrType::Leaf(predef_ty) | IrType::Named(predef_ty) => {
                        let predef_ty_ident = format_ident!("{}", predef_ty);
                        let mut res = quote! {
                            input.try_parse(#predef_ty_ident::parse)
                        };
                        if !from_rep {
                            res = quote! {
                                #res.map(#enum_name::#variant_name)
                                    .map_err(|_| ())
                            }
                        }
                        res
                    }
                    IrType::Repetition { inner, min, max } => {
                        let inner_parser =
                            rec_gen_parser(enum_name, variant_name, Some(&**inner), true);
                        let max = max.unwrap_or(usize::MAX);
                        // TODO: Move this loop to function.
                        quote! {
                            input.try_parse(|input| {
                                let mut i = #min;
                                let mut res = vec![];
                                while let (Ok(v), true) = (#inner_parser, i <= #max) {
                                    res.push(v);
                                    i += 1;
                                }
                                if (#min <= res.len() && res.len() <= #max) {
                                    Ok(#enum_name::#variant_name(res))
                                } else {
                                    Err(())
                                }
                            })
                        }
                    }
                },
                None => {
                    let tok = variant_name.to_string().to_case(Case::Kebab);
                    quote! {
                        input.try_parse(|input| input.expect_ident_matching(#tok))
                            .map(|_| #enum_name::#variant_name)
                            .map_err(|_| ())
                    }
                }
            }
        }

        rec_gen_parser(
            &enum_name,
            &format_ident!("{}", v.name),
            v.payload.as_ref(),
            false,
        )
    });

    let variants_parser = variants_parsers
        .next()
        .into_iter()
        .chain(variants_parsers.map(|p| {
            quote! {
                .or_else(|_| {#p})
            }
        }))
        .fold(quote! {}, |acc, p| {
            quote! {
                #acc #p
            }
        });

    quote! {
        #variants_parser
    }
}

pub(crate) fn gen_parser_body(item: &IrItem) -> TokenStream {
    match item {
        IrItem::Struct(ir_struct) => gen_struct_parser_body(ir_struct),
        IrItem::Enum(ir_enum) => gen_enum_parser_body(ir_enum),
    }
}

pub(crate) fn gen_parser(item: &IrItem) -> TokenStream {
    let name = match item {
        IrItem::Struct(ir_struct) => &ir_struct.name,
        IrItem::Enum(ir_enum) => &ir_enum.name,
    };
    let name = format_ident!("{}", name);
    let parser_body = gen_parser_body(item);
    quote! {
        impl<'i> ParseableProperty<'i> for #name {
            fn parse<'t>(input: &mut Parser<'i, 't>) -> Result<Self, ()> {
                #parser_body
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
    let IrVariant { name, payload } = v;
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
        IrItem::Struct(IrStruct {
            name,
            kind, // We don't need order to emit struct.
            fields,
        }) => {
            let name = format_ident!("{}", name.to_case(Case::Pascal));
            let fields = fields.iter().map(|IrField { name, ty }| {
                let mut ty = gen_type(ty);
                let name = format_ident!("{}", name.to_case(Case::Snake));

                if matches!(kind, StructKind::OneOrMore) {
                    // We want to make field optional if we want at least one of them present.
                    ty = quote! {Option<#ty>};
                }

                quote! {
                    pub #name: #ty
                }
            });
            quote! {
                #[derive(Debug, Clone, PartialEq)]
                pub struct #name {
                    #(#fields),*
                }
            }
        }
        IrItem::Enum(IrEnum { name, variants }) => {
            let variants = variants.iter().map(gen_variant);
            let name = format_ident!("{}", name.to_case(Case::Pascal));

            let res = quote! {
                #[derive(Debug, Clone, PartialEq)]
                pub enum #name {
                    #(#variants),*
                }
            };

            res
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

    let parsers = items.iter().map(gen_parser);

    let decls = items
        .iter()
        .take(items.len() - 1)
        .map(gen_declaration)
        .chain(items.last().map(|x| {
            // For the last item we want to manually generate Copy trait, since Vec is not copy, and therefore derive don't want to derive it for newly defined struct.
            // let name = match x {
            //     IrItem::Struct(ir_struct) => ir_struct.name.clone(),
            //     IrItem::Enum(ir_enum) => ir_enum.name.clone(),
            // };
            // let ident = format_ident!("{}", name);
            let decl = gen_declaration(x);
            quote! {
                #decl
                // impl Copy for #ident { }
            }
        }));

    quote! {
        #(#decls)*

        impl Indexable for #ident {
            const ID: PropIndex = #i;
        }

        #(#parsers)*
    }
}

#[proc_macro]
pub fn css_properties(tokens: proc_macro::TokenStream) -> proc_macro::TokenStream {
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
            let ty = format_ident!("{}", name.to_case(Case::Pascal));
            quote! {
                #variant: std::mem::ManuallyDrop<#ty>
            }
        })
        .collect::<Vec<_>>();

    let prop_to_from_union = props_names
        .iter()
        .map(|name| {
            let variant = format_ident!("{}", name.to_case(Case::Snake));
            let ty = format_ident!("{}", name.to_case(Case::Pascal));

            quote! {
                impl<'a> From<&'a PropUnion> for &'a #ty {
                    fn from(value: &PropUnion) -> &#ty {
                        unsafe {
                            &value.#variant
                        }
                    }
                }

                impl From<#ty> for PropUnion {
                    fn from(value: #ty) -> Self {
                        PropUnion {
                            #variant: std::mem::ManuallyDrop::new(value)
                        }
                    }
                }
            }
        })
        .collect::<Vec<_>>();

    let arms = props_names.iter().map(|&name| {
        let ty = format_ident!("{}", name.to_case(Case::Pascal));
        quote! {
            #name => {#ty::parse(input).map(|t| (#ty::ID, t.into()))}
        }
    });

    let prop_union_parse = quote! {
        pub fn parse<'i, 't>(prop_name: &str, input: &mut Parser<'i, 't>) -> Result<(PropIndex, Self), ()> {
            match prop_name.to_lowercase().as_str() {
                #(#arms)*
                _ => Err(())
            }
        }
    };

    let props_kw = props_names.iter().map(|name| {
        let struct_name_id = format_ident!("{}", name.to_case(Case::Pascal));
        quote! {
            if let Some(v) = self.get::<#struct_name_id>() {
                let _ = debug_struct.field(#name, v);
            }
        }
    });

    let clone_props = props_names.iter().map(|name| {
        let struct_name_id = format_ident!("{}", name.to_case(Case::Pascal));
        quote! {
            if let Some(v) = self.get::<#struct_name_id>() {
                props.set(v.clone());
            }
        }
    });

    quote! {
        #(#props)*

        pub(super) union PropUnion {
            #(#prop_union_variants),*
        }

        #(#prop_to_from_union)*

        impl PropUnion {
            #prop_union_parse
        }

        impl fmt::Debug for Props {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                let mut debug_struct = f.debug_struct("Props");

                #(#props_kw)*

                debug_struct.finish()
            }
        }

        impl Clone for Props {
            fn clone(&self) -> Self {
                let mut props = Self::new();

                #(#clone_props)*

                props
            }
        }
    }
    .into()
}
