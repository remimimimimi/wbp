use convert_case::{Case, Casing};
use css_vds_parser::VDS;

/// Indicates whether the fields in this struct must appear
/// in a fixed order (Sequence) or may appear in any order (AllOf).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Order {
    /// The items must be parsed in sequence.
    Ordered,
    /// The items may appear in any order (e.g. CSS `all-of` groups).
    Unordered,
}

/// A reference to a type: either a leaf (builtin) or one of your IR items.
#[derive(Debug, Clone, PartialEq)]
pub enum IrType {
    /// A keyword.
    Leaf(String),
    /// A predefined struct/enum by name.
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
    pub order: Order,
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

pub fn build_ir<'a>(name: &str, node: &VDS<'a>, items: &mut Vec<IrItem>) -> IrType {
    match node {
        VDS::Keyword(s) | VDS::Value(s) | VDS::Type(s) => IrType::Leaf(s.to_case(Case::Pascal)),

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
                    let field_name = format!("{}F{}", name, i);
                    let field_ty = build_ir(&format!("{}{}", name, i), child, items);
                    IrField {
                        name: field_name,
                        ty: field_ty,
                    }
                })
                .collect();
            items.push(IrItem::Struct(IrStruct {
                name: struct_name.clone(),
                order: Order::Ordered,
                fields,
            }));
            IrType::Named(struct_name)
        }

        VDS::AllOf(children) => {
            let struct_name = name.to_case(Case::Pascal);
            let fields = children
                .iter()
                .map(|child| {
                    if let Some(lit) = child.as_ext_ty() {
                        IrField {
                            name: lit.to_case(Case::Snake),
                            ty: IrType::Leaf(lit.to_string()),
                        }
                    } else {
                        let nested = format!("{}F{}", name, items.len());
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
                order: Order::Unordered,
                fields,
            }));
            IrType::Named(struct_name)
        }

        VDS::OneOrMoreOf(children) => {
            let struct_name = name.to_case(Case::Pascal);
            let fields = children
                .iter()
                .map(|child| {
                    let (field_name, ty) = if let Some(lit) = child.as_ext_ty() {
                        (
                            lit.to_case(Case::Snake),
                            IrType::Leaf(lit.to_case(Case::Pascal)),
                        )
                    } else {
                        let nested = format!("{}F{}", name, items.len());
                        (nested.clone(), build_ir(&nested, child, items))
                    };
                    IrField {
                        name: field_name,
                        ty,
                    }
                })
                .collect();
            items.push(IrItem::Struct(IrStruct {
                name: struct_name.clone(),
                order: Order::Unordered,
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
                    VDS::Value(s) | VDS::Type(s) => IrVariant {
                        name: s.to_case(Case::Pascal),
                        payload: Some(IrType::Leaf(s.to_case(Case::Pascal))),
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

#[test]
fn ir_testing() {
    use chumsky::Parser;
    use css_vds_parser::css_value_parser;

    // let input = " 	[ [<uri> ]* [ auto | crosshair | default | pointer | move | e-resize | ne-resize | nw-resize | n-resize | se-resize | sw-resize | s-resize | w-resize | text | wait | help | progress ] ] | inherit";
    // let input = "[['background-color' | 'background-colour'] || 'background-image' || 'background-repeat' || 'background-attachment' || 'background-position'] | inherit";
    // let input = "['background-color' | 'background-image' | 'background-repeat' | 'background-attachment' | 'background-position'] | inherit";
    let input = "['background-color' || 'background-image' || 'background-repeat' || 'background-attachment' || 'background-position'] | inherit";
    let vds = dbg!(css_value_parser().parse(input).unwrap());
    let mut items = vec![];
    build_ir(&"cursor".to_case(Case::Pascal), &vds, &mut items);
    dbg!(items);
}
