//! Code for applying CSS styles to the DOM.
//!
//! This is not very interesting at the moment.  It will get much more
//! complicated if I add support for compound selectors.

use ego_tree::*;

use crate::css::{props::*, Rule, StyleSheet};
use crate::html::*;

// TODO: Rewrite with reference to the Node in other tree.
/// A node with associated style data.
#[derive(Clone, Debug, PartialEq)]
pub struct StyledNode {
    pub node: Node,
    pub props: Props,
}

impl StyledNode {
    /// Return the specified value of a property if it exists, otherwise `None`.
    pub fn value<'a, T: Property>(&'a self) -> Option<&'a T>
    where
        &'a T: From<&'a PropUnion>,
    {
        self.props.get()
    }

    /// The value of the `display` property (defaults to inline).
    pub fn display(&self) -> Display {
        match self.value() {
            Some(Display::Block) => Display::Block,
            Some(Display::None) => Display::None,
            // NOTE: There is much more variants, but currently we ignore them!
            _ => Display::Inline,
        }
    }
}

/// Apply a stylesheet to an entire DOM tree, returning a StyledNode tree.
///
/// This finds only the specified values at the moment. Eventually it should be extended to find the
/// computed values too, including inherited values.
pub fn style_tree(root: &Tree<Node>, stylesheet: &StyleSheet) -> Tree<StyledNode> {
    root.map_ref(|n| StyledNode {
        node: n.clone(),
        props: match n {
            Node::Element(element) => specified_values(element, stylesheet),
            _ => Props::new(), // Just ignore styling of other elements, e.g. text for now.
        },
    })
}

// TODO: ElementRef instead of ElementData
/// Apply styles to a single element, returning the specified styles.
///
/// To do: Allow multiple UA/author/user stylesheets, and implement the cascade.
fn specified_values(elem: &crate::html::node::Element, stylesheet: &StyleSheet) -> Props {
    let mut props = Props::new();
    let mut rules = matching_rules(elem, stylesheet);

    // Go through the rules from lowest to highest specificity.
    rules.sort_by(|&(a, _), &(b, _)| a.cmp(&b));
    for (_, rule) in rules {
        let rule_props = &rule.declarations;
        props.extend(rule_props);
    }

    props
}

type Specificity = u32;
/// A single CSS rule and the specificity of its most specific matching selector.
type MatchedRule<'a> = (Specificity, &'a Rule);

/// Find all CSS rules that match the given element.
fn matching_rules<'a>(
    elem: &crate::html::node::Element,
    stylesheet: &'a StyleSheet,
) -> Vec<MatchedRule<'a>> {
    // For now, we just do a linear scan of all the rules.  For large
    // documents, it would be more efficient to store the rules in hash tables
    // based on tag name, id, class, etc.
    stylesheet
        .iter()
        .filter_map(|rule| match_rule(elem, rule))
        .collect()
}

/// If `rule` matches `elem`, return a `MatchedRule`. Otherwise return `None`.
fn match_rule<'a>(elem: &crate::html::node::Element, rule: &'a Rule) -> Option<MatchedRule<'a>> {
    todo!()

    // let highest_specificity_matching_selector =
    //     rule.selectors.matching_selector(ElementRef::wrap(elem));
    // if let Some(s) = highest_specificity_matching_selector {
    //     todo!()
    // } else {
    //     todo!()
    // }
    // // Find the first (most specific) matching selector.
    // rule.selectors
    //     .iter()
    //     .find(|selector| matches(elem, selector))
    //     .map(|selector| (selector.specificity(), rule))
}

// /// Selector matching:
// fn matches(elem: &crate::html::node::Element, selector: &Selector) -> bool {
//     match selector {
//         Selector::Simple(s) => matches_simple_selector(elem, s),
//     }
// }

// fn matches_simple_selector(elem: &crate::html::node::Element, selector: &SimpleSelector) -> bool {
//     // TODO: Check full name instead of just local one, but shood be good enough for now.
//     // Check type selector
//     if selector
//         .tag_name
//         .iter()
//         .any(|name| elem.name.local.as_ref() != *name)
//     {
//         return false;
//     }

//     // Check ID selector
//     if selector.id.iter().any(|id| elem.id() != Some(id)) {
//         return false;
//     }

//     // Check class selectors
//     if selector
//         .class
//         .iter()
//         .any(|class| !elem.classes().any(|c| c == class.as_str()))
//     {
//         return false;
//     }

//     // We didn't find any non-matching selector components.
//     true
// }
