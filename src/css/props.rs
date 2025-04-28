// We want to be as safe as possible, so forbid implementing Indexable and access to PropUnion.
#![allow(private_bounds)]
use std::{collections::HashMap, fmt};

use css_macros::css_properties;
use cssparser::Parser;
use sealed::sealed;

use super::values::*;

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

#[test]
fn my_test() {
    use cssparser::*;

    let input = "transparent inherit repeat inherit inherit";
    let mut input = ParserInput::new(input);
    let mut parser = Parser::new(&mut input);

    let _ = dbg!(Background::parse(&mut parser));

    // let u = PropUnion {
    //     margin: std::mem::ManuallyDrop::new(Margin::Inherit),
    // };
    // let x = unsafe {
    //     PropUnion {
    //         margin: u.margin.clone(),
    //     }
    // };
}
