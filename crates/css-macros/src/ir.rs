use convert_case::{Case, Casing};
use css_vds_parser::VDS;

// TODO: Think about better name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StructKind {
    /// The items must be parsed in sequence.
    OrderedAnd,
    /// The items may appear in any order (e.g. CSS `all-of` groups).
    UnorderedAnd,
    /// At least one item from the struct must be present.
    OneOrMore,
}

/// A reference to a type: either a leaf (builtin) or one of your IR items.
#[derive(Debug, Clone, PartialEq)]
pub enum IrType {
    /// A leaf token.
    Leaf(String),
    /// A user-defined struct/enum by name
    Named(String),
    /// A repetition of some inner type.
    /// `min == 0, max == None` means `*` (ZeroOrMore),
    /// `min == 1, max == None` means `+` (OneOrMore),
    /// `min == 0, max == Some(1)` means `?` (Optional),
    /// `min == n, max == Some(m)` means a bounded range.
    Repetition {
        inner: Box<IrType>,
        min: usize,
        max: Option<usize>,
    },
}

/// One field in a struct.
#[derive(Debug, Clone, PartialEq)]
pub struct IrField {
    pub name: String,
    pub ty: IrType,
}

/// A named struct with zero or more fields.
#[derive(Debug, Clone, PartialEq)]
pub struct IrStruct {
    pub name: String,
    pub kind: StructKind,
    pub fields: Vec<IrField>,
}

/// One variant in an enum.
#[derive(Debug, Clone, PartialEq)]
pub struct IrVariant {
    pub name: String, // CamelCase
    pub payload: Option<IrType>,
}

/// A named enum with one or more variants.
#[derive(Debug, Clone, PartialEq)]
pub struct IrEnum {
    pub name: String,
    pub variants: Vec<IrVariant>,
}

/// A toplevel item: either a `struct` or an `enum`.
#[derive(Debug, Clone, PartialEq)]
pub enum IrItem {
    Struct(IrStruct),
    Enum(IrEnum),
}

pub fn build_ir(name: &str, node: &VDS<'_>, items: &mut Vec<IrItem>) -> IrType {
    match node {
        VDS::Keyword(s) | VDS::Value(s) => IrType::Leaf(s.to_case(Case::Pascal)),
        VDS::Type(s) => IrType::Named(s.to_case(Case::Pascal)),
        VDS::Optional(inner) => {
            let ty = build_ir(name, inner, items);
            IrType::Repetition {
                inner: Box::new(ty),
                min: 0,
                max: Some(1),
            }
        }
        VDS::ZeroOrMore(inner) => {
            let ty = build_ir(name, inner, items);
            IrType::Repetition {
                inner: Box::new(ty),
                min: 0,
                max: None,
            }
        }
        VDS::OneOrMore(inner) => {
            let ty = build_ir(name, inner, items);
            IrType::Repetition {
                inner: Box::new(ty),
                min: 1,
                max: None,
            }
        }
        VDS::Range(inner, min, max) => {
            let ty = build_ir(name, inner, items);
            IrType::Repetition {
                inner: Box::new(ty),
                min: *min,
                max: Some(*max),
            }
        }

        VDS::Sequence(children) => {
            let struct_name = name.to_case(Case::Pascal);
            let fields = children
                .iter()
                .enumerate()
                .map(|(i, child)| {
                    let field_name = format!("{}F{}", name, i).to_case(Case::Snake);
                    let field_ty = build_ir(
                        &format!("{}F{}", name.to_case(Case::Pascal), i),
                        child,
                        items,
                    );
                    IrField {
                        name: field_name,
                        ty: field_ty,
                    }
                })
                .collect();
            items.push(IrItem::Struct(IrStruct {
                name: struct_name.clone(),
                kind: StructKind::OrderedAnd,
                fields,
            }));
            IrType::Named(struct_name)
        }

        VDS::AllOf(children) => {
            let struct_name = name.to_case(Case::Pascal);
            let fields = children
                .iter()
                .map(|child| match child {
                    VDS::Keyword(lit) | VDS::Value(lit) => IrField {
                        name: lit.to_case(Case::Snake),
                        ty: IrType::Leaf(lit.to_string()),
                    },
                    VDS::Type(lit) => IrField {
                        name: lit.to_case(Case::Snake),
                        ty: IrType::Named(lit.to_string()),
                    },
                    _ => {
                        let nested = format!("{}F{}", name, items.len()).to_case(Case::Snake);
                        let ty = build_ir(&nested, child, items);
                        IrField {
                            name: nested.clone(),
                            ty,
                        }
                    }
                })
                .collect();
            items.push(IrItem::Struct(IrStruct {
                name: struct_name.clone(),
                kind: StructKind::UnorderedAnd,
                fields,
            }));
            IrType::Named(struct_name)
        }

        VDS::OneOrMoreOf(children) => {
            let struct_name = name.to_case(Case::Pascal);
            let fields = children
                .iter()
                .map(|child| {
                    let (field_name, ty) = match child {
                        VDS::Keyword(_) => unreachable!(),
                        VDS::Value(s) => (
                            s.to_case(Case::Snake),
                            IrType::Leaf(s.to_case(Case::Pascal)),
                        ),
                        VDS::Type(s) => (
                            s.to_case(Case::Snake),
                            IrType::Named(s.to_case(Case::Pascal)),
                        ),
                        _ => {
                            let nested = format!("{}F{}", name, items.len()).to_case(Case::Snake);
                            (nested.clone(), build_ir(&nested, child, items))
                        }
                    };

                    IrField {
                        name: field_name,
                        ty,
                    }
                })
                .collect();
            items.push(IrItem::Struct(IrStruct {
                name: struct_name.clone(),
                kind: StructKind::OneOrMore,
                fields,
            }));
            IrType::Named(struct_name)
        }

        VDS::Choice(options) => {
            let enum_name = (name).to_case(Case::Pascal);
            let variants = options
                .iter()
                .map(|opt| match opt {
                    VDS::Keyword(s) => IrVariant {
                        name: s.to_case(Case::Pascal),
                        payload: None,
                    },
                    VDS::Value(s) => IrVariant {
                        name: s.to_case(Case::Pascal),
                        payload: Some(IrType::Leaf(s.to_case(Case::Pascal))),
                    },
                    VDS::Type(s) => IrVariant {
                        name: s.to_case(Case::Pascal),
                        payload: Some(IrType::Named(s.to_case(Case::Pascal))),
                    },
                    _ => {
                        let var_name = format!("{}V{}", name.to_case(Case::Pascal), items.len());
                        let payload = build_ir(&var_name, opt, items);
                        IrVariant {
                            name: var_name.clone(),
                            payload: Some(payload),
                        }
                    }
                })
                .collect();
            items.push(IrItem::Enum(IrEnum {
                name: enum_name.clone(),
                variants,
            }));
            IrType::Named(enum_name)
        }
    }
}

// #[test]
// fn ir_testing() {
//     use chumsky::Parser;
//     use css_vds_parser::css_value_parser;

//     // let input = " 	[ [<uri> ]* [ auto | crosshair | default | pointer | move | e-resize | ne-resize | nw-resize | n-resize | se-resize | sw-resize | s-resize | w-resize | text | wait | help | progress ] ] | inherit";
//     // let input = "[['background-color' | 'background-colour'] || 'background-image' || 'background-repeat' || 'background-attachment' || 'background-position'] | inherit";
//     // let input = "['background-color' | 'background-image' | 'background-repeat' | 'background-attachment' | 'background-position'] | inherit";
//     // let input = "['background-color' || 'background-image' || 'background-repeat' || 'background-attachment' || 'background-position'] | inherit";
//     let input = "
// [ [ <percentage> | <length> | left | center | right ]
//   [ <percentage> | <length> | top | center | bottom ]? ]
// | [ [ left | center | right ]
//     || [ top | center | bottom ] ]
// | inherit";
//     let vds = dbg!(css_value_parser().parse(input).unwrap());
//     let mut items = vec![];
//     build_ir(&"cursor".to_case(Case::Pascal), &vds, &mut items);
//     dbg!(items);
// }
