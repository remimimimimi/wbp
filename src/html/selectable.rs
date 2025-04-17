//! Provides the [`Selectable`] to abstract over collections of elements

use crate::html::element_ref::{self, ElementNode, ElementRef};
use crate::html::{Html, Node};
use crate::selector::SelectorGroup;

/// Trait to abstract over collections of elements to which a [CSS selector][Selector] can be applied
///
/// The mainly enables writing helper functions which are generic over [`Html`] and [`ElementRef`], e.g.
///
/// ```
/// use scraper::{selectable::Selectable, selector::Selector};
///
/// fn text_of_first_match<'a, S>(selectable: S, selector: &Selector) -> Option<String>
/// where
///     S: Selectable<'a>,
/// {
///     selectable.select(selector).next().map(|element| element.text().collect())
/// }
/// ```
pub trait Selectable<'a, E: ElementNode + 'a> {
    /// Iterator over [element references][ElementRef] matching a [CSS selector[Selector]
    type Select<'b>: Iterator<Item = ElementRef<'a, E>>;

    /// Applies the given `selector` to the collection of elements represented by `self`
    fn select(self, selector: &SelectorGroup) -> Self::Select<'_>;
}

impl<'a> Selectable<'a, Node> for &'a Html {
    type Select<'b> = crate::html::Select<'a, 'b, Node>;

    fn select(self, selector: &SelectorGroup) -> Self::Select<'_> {
        Html::select(self, selector)
    }
}

impl<'a, E: ElementNode + Clone> Selectable<'a, E> for ElementRef<'a, E> {
    type Select<'b> = element_ref::Select<'a, 'b, E>;

    fn select(self, selector: &SelectorGroup) -> Self::Select<'_> {
        ElementRef::select(&self, selector)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn select_one<'a, S, E: ElementNode>(
        selectable: S,
        selector: &SelectorGroup,
    ) -> Option<ElementRef<'a, E>>
    where
        S: Selectable<'a, E>,
    {
        selectable.select(selector).next()
    }

    #[test]
    fn html_and_element_ref_are_selectable() {
        let fragment = Html::parse_fragment(
            r#"<select class="foo"><option value="bar">foobar</option></select>"#,
        );

        let selector = SelectorGroup::parse("select.foo").unwrap();
        let element = select_one(&fragment, &selector).unwrap();

        let selector = SelectorGroup::parse("select.foo option[value='bar']").unwrap();
        let _element = select_one(element, &selector).unwrap();
    }
}
