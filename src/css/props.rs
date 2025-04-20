use std::collections::HashMap;

use css_macros::css_properties;
use cssparser::Parser;

pub trait ParseableProperty<'i>: Sized {
    fn parse<'t>(input: &mut Parser<'i, 't>) -> Result<Self, ()>;
}

pub type PropIndex = u8;

/// Ad-hoc property needed for efficient representation in memory
/// using hash map. Macro will automatically assign unique number of
/// each property. For now we assume that it there is less than 256
/// different properties.
pub trait Indexable {
    const ID: PropIndex;
}

/// Relative length.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Relative {
    /// the 'font-size' of the relevant font
    Em(f32),
    /// the 'x-height' of the relevant font
    Ex(f32),
}

/// Absolute length
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Absolute {
    /// Inches
    In(f32),
    /// Centimeters
    Cm(f32),
    /// Millimeters
    Mm(f32),
    /// Points
    Pt(f32),
    /// Picas
    Pc(f32), // == 12pt
    /// Pixels
    Px(f32),
}

// TODO: Move values to different module.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Length {
    Relative(Relative),
    Absolute(Absolute),
}

impl<'i> ParseableProperty<'i> for Length {
    fn parse<'t>(input: &mut Parser<'i, 't>) -> Result<Self, ()> {
        match input.next().map_err(|_| ())? {
            cssparser::Token::Dimension { value, unit, .. } => {
                match unit.to_ascii_lowercase().as_str() {
                    "em" => Ok(Self::Relative(Relative::Em(*value))),
                    "ex" => Ok(Self::Relative(Relative::Ex(*value))),

                    "in" => Ok(Self::Absolute(Absolute::In(*value))),
                    "cm" => Ok(Self::Absolute(Absolute::Cm(*value))),
                    "mm" => Ok(Self::Absolute(Absolute::Mm(*value))),
                    "pt" => Ok(Self::Absolute(Absolute::Pt(*value))),
                    "pc" => Ok(Self::Absolute(Absolute::Pc(*value))),
                    "px" => Ok(Self::Absolute(Absolute::Px(*value))),

                    _ => Err(()),
                }
            }
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
/// Contains normalized value in range from 0.0 to 1.0.
pub struct Percentage(f32);

impl<'i> ParseableProperty<'i> for Percentage {
    fn parse<'t>(input: &mut Parser<'i, 't>) -> Result<Self, ()> {
        match input.next().map_err(|_| ())? {
            cssparser::Token::Percentage { unit_value, .. } => Ok(Self(*unit_value)),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PaddingWidth {
    Length(Length),
    Percentage(Percentage),
}

impl<'i> ParseableProperty<'i> for PaddingWidth {
    fn parse<'t>(input: &mut Parser<'i, 't>) -> Result<Self, ()> {
        input
            .try_parse(Length::parse)
            .map(PaddingWidth::Length)
            .or_else(|_| {
                input
                    .try_parse(Percentage::parse)
                    .map(PaddingWidth::Percentage)
            })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MarginWidth {
    Length(Length),
    Percentage(Percentage),
    Auto,
}

impl<'i> ParseableProperty<'i> for MarginWidth {
    fn parse<'t>(input: &mut Parser<'i, 't>) -> Result<Self, ()> {
        input
            .try_parse(Length::parse)
            .map(MarginWidth::Length)
            .or_else(|_| {
                input
                    .try_parse(Percentage::parse)
                    .map(MarginWidth::Percentage)
            })
            .or_else(|_| {
                input
                    .try_parse(|input| input.expect_ident_matching("auto"))
                    .map(|_| MarginWidth::Auto)
                    .map_err(|_| ())
            })
    }
}

css_properties!("src/css/props.json");

pub struct Props(HashMap<PropIndex, PropUnion>);

impl Props {
    pub fn get<T: Indexable + From<PropUnion>>(&self) -> Option<T> {
        self.0.get(&T::ID).map(|&pu| pu.into())
    }

    pub fn set<T: Indexable + Into<PropUnion>>(&mut self, value: T) {
        self.0.insert(T::ID, value.into());
    }
}
