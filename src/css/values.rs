use super::props::ParseableProperty;

use cssparser::{
    color::{parse_hash_color, parse_named_color},
    ParseError, ParseErrorKind, Parser,
};
use url::Url;

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

impl Length {
    pub fn to_px(&self) -> f32 {
        match self {
            Length::Relative(_) => todo!(),
            Length::Absolute(absolute) => match absolute {
                Absolute::Px(px) => *px,
                _ => todo!(),
            },
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

impl PaddingWidth {
    pub fn to_px(&self) -> f32 {
        match self {
            PaddingWidth::Length(length) => length.to_px(),
            _ => todo!(),
        }
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

impl MarginWidth {
    pub fn to_px(&self) -> f32 {
        match self {
            MarginWidth::Length(length) => length.to_px(),
            _ => todo!(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BorderWidth {
    Thin,
    Medium,
    Thick,
    Length(Length),
}

impl<'i> ParseableProperty<'i> for BorderWidth {
    fn parse<'t>(input: &mut Parser<'i, 't>) -> Result<Self, ()> {
        input
            .try_parse(|input| input.expect_ident_matching("thin"))
            .map(|_| BorderWidth::Thin)
            .or_else(|_| {
                input
                    .try_parse(|input| input.expect_ident_matching("medium"))
                    .map(|_| BorderWidth::Medium)
                    .map_err(|_| ())
            })
            .or_else(|_| {
                input
                    .try_parse(|input| input.expect_ident_matching("thick"))
                    .map(|_| BorderWidth::Thick)
                    .map_err(|_| ())
            })
            .or_else(|_| input.try_parse(Length::parse).map(BorderWidth::Length))
    }
}

impl BorderWidth {
    pub fn to_px(&self) -> f32 {
        match self {
            BorderWidth::Length(length) => length.to_px(),
            _ => todo!(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color(pub u8, pub u8, pub u8);

impl<'i> ParseableProperty<'i> for Color {
    fn parse<'t>(input: &mut Parser<'i, 't>) -> Result<Self, ()> {
        match input.next().map_err(|_| ())? {
            cssparser::Token::Ident(cow_rc_str) => {
                parse_named_color(cow_rc_str).map(|(r, g, b)| Color(r, g, b))
            }
            cssparser::Token::Hash(cow_rc_str) | cssparser::Token::IDHash(cow_rc_str) => {
                parse_hash_color(cow_rc_str.as_bytes()).map(|(r, g, b, _)| Color(r, g, b))
            }
            cssparser::Token::Function(cow_rc_str) if *cow_rc_str == "rgb" => {
                let r = input.try_parse(|input| {
                    input
                        .parse_nested_block(|input| {
                            let arr = input.parse_comma_separated(
                                |input| -> Result<f32, ParseError<'_, ()>> {
                                    input.expect_number().map_err(|e| ParseError {
                                        kind: ParseErrorKind::Custom(()),
                                        location: e.location,
                                    })
                                },
                            )?;
                            let Some(&[r, g, b]) = arr.get(..3) else {
                                return Err(ParseError {
                                    kind: ParseErrorKind::Custom(()),
                                    location: input.current_source_location(),
                                });
                            };
                            Ok(Color(r as u8, g as u8, b as u8)) // TODO: Fix type conversion and propertly handle errors.
                        })
                        .map_err(|_| ())
                });
                if let Ok(c) = r {
                    return Ok(c);
                }

                input
                    .parse_nested_block(|input| {
                        let arr = input.parse_comma_separated(
                            |input| -> Result<f32, ParseError<'_, ()>> {
                                input.expect_percentage().map_err(|e| ParseError {
                                    kind: ParseErrorKind::Custom(()),
                                    location: e.location,
                                })
                            },
                        )?;
                        let Some(&[r, g, b]) = arr.get(..3) else {
                            return Err(ParseError {
                                kind: ParseErrorKind::Custom(()),
                                location: input.current_source_location(),
                            });
                        };
                        Ok(Color((r * 255.) as u8, (g * 255.) as u8, (b * 255.) as u8))
                    })
                    .map_err(|_| ())
            }
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Uri(Url); // lol

impl<'i> ParseableProperty<'i> for Uri {
    fn parse<'t>(input: &mut Parser<'i, 't>) -> Result<Self, ()> {
        let s = input.expect_url().map_err(|_| ())?;
        Url::parse(&s).map(Uri).map_err(|_| ())
    }
}
