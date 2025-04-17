//! Code for applying CSS styles to the DOM.
//!
//! This is not very interesting at the moment.  It will get much more
//! complicated if I add support for compound selectors.

use std::collections::HashMap;

use lightningcss::{
    declaration::DeclarationBlock,
    properties::{
        display::{self, DisplayKeyword, DisplayOutside},
        Property, PropertyId,
    },
    rules::style::StyleRule,
    stylesheet::StyleSheet,
};

use ego_tree::*;

use scraper::*;

/// Map from CSS property names to values.
pub type PropertyMap<'a> = DeclarationBlock<'a>;

// TODO: Rewrite with reference to the Node in other tree.
/// A node with associated style data.
#[derive(Clone, Debug, PartialEq)]
pub struct StyledNode<'i> {
    pub node: Node,
    pub specified_values: PropertyMap<'i>,
}

#[derive(PartialEq)]
pub enum Display {
    Inline,
    Block,
    None,
}

impl<'i> StyledNode<'i> {
    /// Return the specified value of a property if it exists, otherwise `None`.
    pub fn value(&'i self, name: PropertyId) -> Option<Property<'i>> {
        self.specified_values.get(&name).map(|p| p.0.into_owned())
    }

    /// Return the specified value of property `name`, or property
    /// `fallback_name` if that doesn't exist.
    pub fn lookup(&'i self, name: PropertyId, fallback_name: PropertyId) -> Option<Property<'i>> {
        self.value(name).or(self.value(fallback_name))
        // self.value(name)
        //     .unwrap_or_else(|| self.value(fallback_name).unwrap_or_else(|| default.clone()))
    }

    /// The value of the `display` property (defaults to inline).
    pub fn display(&'i self) -> Display {
        match self.value(PropertyId::Display).clone() {
            Some(Property::Display(d)) => match &d {
                display::Display::Keyword(DisplayKeyword::None) => Display::None,
                display::Display::Pair(display_pair) => match display_pair.outside {
                    DisplayOutside::Block => Display::Block,
                    DisplayOutside::Inline => Display::Inline,
                    _ => Display::Inline,
                },
                _ => Display::Inline, // TODO: Handle other possible variants.
            },
            _ => Display::Inline, // Couldn't not be anything except
                                  // some display or none, since we use `get` method to retreive
                                  // property.
        }
    }
}

/// Apply a stylesheet to an entire DOM tree, returning a StyledNode tree.
///
/// This finds only the specified values at the moment. Eventually it should be extended to find the
/// computed values too, including inherited values.
pub fn style_tree<'i>(
    root: &'i Tree<Node>,
    stylesheet: &'i StyleSheet<'i, '_>,
) -> Tree<StyledNode<'i>> {
    // todo!()

    root.map_ref(|n| StyledNode {
        node: n.clone(),
        specified_values: match n {
            e @ Node::Element(_) => {
                // Fucked up.
                let elem =
                    ElementRef::wrap(root.nodes().find(|n| n.value() == e).unwrap()).unwrap();
                specified_values(&elem, stylesheet)
            } // specified_values(element, stylesheet),
            _ => todo!(), // Just ignore styling of other elements, e.g. text for now.
        },
    })
}

// TODO: ElementRef instead of ElementData
/// Apply styles to a single element, returning the specified styles.
///
/// To do: Allow multiple UA/author/user stylesheets, and implement the cascade.
fn specified_values<'i>(
    elem: &scraper::ElementRef<'_>,
    stylesheet: &'i StyleSheet<'i, '_>,
) -> PropertyMap<'i> {
    let mut map = PropertyMap::new();
    let mut rules = matching_rules(elem, stylesheet);

    // let mut values = HashMap::new();
    // let mut rules = matching_rules(elem, stylesheet);

    // // Go through the rules from lowest to highest specificity.
    // rules.sort_by(|&(a, _), &(b, _)| a.cmp(&b));
    // for (_, rule) in rules {
    //     for declaration in &rule.declarations {
    //         values.insert(declaration.name.clone(), declaration.value.clone());
    //     }
    // }

    // values
    todo!()
}

// /// A single CSS rule and the specificity of its most specific matching selector.
// type MatchedRule<'a> = (Specificity, &'a Rule);

/// Find all CSS rules that match the given element.
fn matching_rules<'a>(
    elem: &scraper::ElementRef<'_>,
    stylesheet: &'a StyleSheet<'a, '_>,
) -> Vec<&'a StyleRule<'a>> {
    // For now, we just do a linear scan of all the rules.  For large
    // documents, it would be more efficient to store the rules in hash tables
    // based on tag name, id, class, etc.
    stylesheet
        .rules
        .0
        .iter()
        .filter_map(|r| match r {
            // We care only about style rules for now.
            // Note that we also ignore all nested rules.
            lightningcss::rules::CssRule::Style(style_rule) => {
                let selectors_string = style_rule.selectors.to_string();
                let scraper_selector = scraper::Selector::parse(&selectors_string).unwrap();
                if scraper_selector.matches(elem) {
                    Some(style_rule)
                } else {
                    None
                }
            }
            _ => None,
        })
        .collect()
}

/// Add all properties from the second declaration block to the first one.
fn merge_declaration_blocks<'a>(a: &mut DeclarationBlock<'a>, b: DeclarationBlock<'a>) {
    b.iter().for_each(|(p, i)| a.set(p.clone(), i));
}
