// We want to be as safe as possible, so forbid implementing Indexable and access to PropUnion.
#![allow(private_bounds)]
use std::{collections::HashMap, fmt};

use css_macros::css_properties;
use cssparser::Parser;
use log::warn;
use sealed::sealed;

use super::values;

// TODO: Think about other name for this trait, since values also use it.
pub trait ParseableProperty<'i>: Sized {
    fn parse<'t>(input: &mut Parser<'i, 't>) -> Result<Self, ()>;
}

pub type PropIndex = u8;

/// Ad-hoc trait needed for efficient representation in memory
/// using hash map. Macro will automatically assign unique number of
/// each property. For now we assume that it there is less than 256
/// different properties.
#[sealed]
pub trait Property {
    const ID: PropIndex;
}

css_properties!("src/css/props.json");

impl MarginTop {
    pub fn to_px(&self) -> f32 {
        match self {
            MarginTop::Inherit => todo!(),
            MarginTop::MarginWidth(margin_width) => margin_width.to_px(),
        }
    }
}

impl MarginRight {
    pub fn to_px(&self) -> f32 {
        match self {
            MarginRight::Inherit => todo!(),
            MarginRight::MarginWidth(margin_width) => margin_width.to_px(),
        }
    }
}

impl MarginBottom {
    pub fn to_px(&self) -> f32 {
        match self {
            MarginBottom::Inherit => todo!(),
            MarginBottom::MarginWidth(margin_width) => margin_width.to_px(),
        }
    }
}

impl MarginLeft {
    pub fn to_px(&self) -> f32 {
        match self {
            MarginLeft::Inherit => todo!(),
            MarginLeft::MarginWidth(margin_width) => margin_width.to_px(),
        }
    }
}

impl BorderTopWidth {
    pub fn to_px(&self) -> f32 {
        match self {
            BorderTopWidth::Inherit => todo!(),
            BorderTopWidth::BorderWidth(border_width) => border_width.to_px(),
        }
    }
}

impl BorderRightWidth {
    pub fn to_px(&self) -> f32 {
        match self {
            BorderRightWidth::Inherit => todo!(),
            BorderRightWidth::BorderWidth(border_width) => border_width.to_px(),
        }
    }
}

impl BorderBottomWidth {
    pub fn to_px(&self) -> f32 {
        match self {
            BorderBottomWidth::Inherit => todo!(),
            BorderBottomWidth::BorderWidth(border_width) => border_width.to_px(),
        }
    }
}

impl BorderLeftWidth {
    pub fn to_px(&self) -> f32 {
        match self {
            BorderLeftWidth::Inherit => todo!(),
            BorderLeftWidth::BorderWidth(border_width) => border_width.to_px(),
        }
    }
}

impl PaddingTop {
    pub fn to_px(&self) -> f32 {
        match self {
            PaddingTop::Inherit => todo!(),
            PaddingTop::PaddingWidth(padding_width) => padding_width.to_px(),
        }
    }
}

impl PaddingRight {
    pub fn to_px(&self) -> f32 {
        match self {
            PaddingRight::Inherit => todo!(),
            PaddingRight::PaddingWidth(padding_width) => padding_width.to_px(),
        }
    }
}

impl PaddingBottom {
    pub fn to_px(&self) -> f32 {
        match self {
            PaddingBottom::Inherit => todo!(),
            PaddingBottom::PaddingWidth(padding_width) => padding_width.to_px(),
        }
    }
}

impl PaddingLeft {
    pub fn to_px(&self) -> f32 {
        match self {
            PaddingLeft::Inherit => todo!(),
            PaddingLeft::PaddingWidth(padding_width) => padding_width.to_px(),
        }
    }
}

impl Width {
    pub fn to_px(&self) -> f32 {
        match self {
            Width::Length(length) => length.to_px(),
            Width::Auto => {
                warn!("Setting auto width to zero.");
                0.0
            }
            _ => todo!(),
        }
    }
}

impl From<BackgroundColor> for values::Color {
    fn from(value: BackgroundColor) -> Self {
        match value {
            BackgroundColor::Transparent => todo!(),
            BackgroundColor::Inherit => todo!(),
            BackgroundColor::Color(color) => color,
        }
    }
}

impl From<Background> for values::Color {
    fn from(value: Background) -> Self {
        match value {
            Background::Inherit => todo!(),
            Background::BackgroundV0(bg) => bg
                .background_color
                .map(|bg| bg.into())
                .unwrap_or(values::Color(255, 255, 255)),
        }
    }
}

impl From<BorderColor> for values::Color {
    fn from(value: BorderColor) -> Self {
        match value {
            BorderColor::Inherit => todo!(),
            BorderColor::BorderColorV0(border_color_v0s) => match border_color_v0s.len() {
                1 => match border_color_v0s.first() {
                    Some(BorderColorV0::Color(c)) => *c,
                    _ => todo!(),
                },
                _ => todo!(),
            },
        }
    }
}

// TODO: Implement drop, since every variant of prop union is manual drop.
#[derive(Default)]
pub struct Props(HashMap<PropIndex, PropUnion>);

impl Props {
    /// Create new empty property map.
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    /// Get property value.
    pub fn get<'a, T: Property>(&'a self) -> Option<&'a T>
    where
        &'a T: From<&'a PropUnion>,
    {
        self.0.get(&T::ID).map(|pu| pu.into())
    }

    /// Set property to value.
    pub fn set<T: Property + Into<PropUnion>>(&mut self, value: T) {
        self.0.insert(T::ID, value.into());
    }

    /// Set property by `Indexable::ID`.
    /// # Safety
    /// This function is save to use when index is matches some type that implements `Indexable` with corresponding `ID`.
    pub unsafe fn set_idx<T: Into<PropUnion>>(&mut self, idx: PropIndex, value: T) {
        self.0.insert(idx, value.into());
    }

    /// Overwrite properties from key value pairs of `other` props.
    pub fn extend(&mut self, other: &Props) {
        for (&k, v) in other.0.iter() {
            unsafe { self.set_idx(k, v.clone_variant(k)) }
        }
    }
}

// #[test]
// fn my_test() {
//     use cssparser::*;

//     let input = "rgb(255, 255, 255) inherit repeat inherit inherit";
//     let mut input = ParserInput::new(input);
//     let mut parser = Parser::new(&mut input);

//     let _ = dbg!(Background::parse(&mut parser));

//     // let u = PropUnion {
//     //     margin: std::mem::ManuallyDrop::new(Margin::Inherit),
//     // };
//     // let x = unsafe {
//     //     PropUnion {
//     //         margin: u.margin.clone(),
//     //     }
//     // };
// }
