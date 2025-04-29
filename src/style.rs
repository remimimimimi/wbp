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
pub fn style_tree(tree: &Tree<Node>, stylesheet: &StyleSheet) -> Tree<StyledNode> {
    let f = |nr: NodeRef<Node>, stylesheet: &StyleSheet| {
        StyledNode {
            node: nr.value().clone(),
            props: match ElementRef::wrap(nr) {
                Some(er) => specified_values(&er, stylesheet),
                _ => Props::new(), // Just ignore styling of other elements, e.g. text for now.
            },
        }
    };

    fn style_tree_rec(
        mut style_node: NodeMut<StyledNode>,
        dom_node: NodeRef<Node>,
        stylesheet: &StyleSheet,
        f: fn(NodeRef<Node>, &StyleSheet) -> StyledNode,
    ) {
        for child in dom_node.children() {
            style_tree_rec(
                style_node.append(f(child, stylesheet)),
                child,
                stylesheet,
                f,
            )
        }
    }

    let root_value = tree.root();
    let mut style_tree = Tree::new(f(root_value, stylesheet));
    let style_root = style_tree.root_mut();
    let root = tree.root();

    // TODO: Optimize tree traversal to avoid recursion using algorithm of `NodeMut::for_each_descendant`.
    style_tree_rec(style_root, root, stylesheet, f);

    style_tree
}

// TODO: ElementRef instead of ElementData
/// Apply styles to a single element, returning the specified styles.
///
/// To do: Allow multiple UA/author/user stylesheets, and implement the cascade.
fn specified_values(elem: &ElementRef<Node>, stylesheet: &StyleSheet) -> Props {
    let mut props = Props::new();
    let mut rules = matching_rules(&elem, stylesheet);

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
fn matching_rules<'a>(elem: &ElementRef<Node>, stylesheet: &'a StyleSheet) -> Vec<MatchedRule<'a>> {
    // For now, we just do a linear scan of all the rules.  For large
    // documents, it would be more efficient to store the rules in hash tables
    // based on tag name, id, class, etc.
    stylesheet
        .iter()
        .filter_map(|rule| match_rule(elem, rule))
        .collect()
}

/// If `rule` matches `elem`, return a `MatchedRule`. Otherwise return `None`.
fn match_rule<'a>(elem: &ElementRef<Node>, rule: &'a Rule) -> Option<MatchedRule<'a>> {
    let highest_specificity_matching_selector = rule.selectors.matching_selector(elem);
    highest_specificity_matching_selector.map(|s| (s.specificity(), rule))
}
