// We want to be as safe as possible, so forbid implementing Indexable and access to PropUnion.
#![allow(private_bounds)]

use std::collections::HashMap;

use css_macros::css_properties;
use cssparser::Parser;

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
trait Indexable {
    const ID: PropIndex;
}

css_properties!("src/css/props.json");

// TODO: Implement drop, since every variant of prop union is manual drop.
pub struct Props(HashMap<PropIndex, PropUnion>);

impl Props {
    pub fn get<'a, T: Indexable>(&'a self) -> Option<&'a T>
    where
        &'a T: From<&'a PropUnion>,
    {
        self.0.get(&T::ID).map(|pu| pu.into())
    }

    pub fn set<T: Indexable + Into<PropUnion>>(&mut self, value: T) {
        self.0.insert(T::ID, value.into());
    }
}
