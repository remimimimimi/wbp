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
    /// Return the specified value by reference of a property if it exists, otherwise `None`.
    pub fn get<'a, T: Property>(&'a self) -> Option<&'a T>
    where
        &'a T: From<&'a PropUnion>,
    {
        self.props.get()
    }

    /// Return the specified value of a property if it exists, otherwise `None`.
    pub fn value<T: Property + Clone>(&self) -> Option<T>
    where
        for<'a> &'a T: From<&'a PropUnion>,
    {
        self.get().cloned()
    }

    /// Return the specified value of property `name`, or property `fallback_name` if that doesn't
    /// exist, or value `default` if neither does.
    pub fn lookup<T: Property + Clone, U: Property + Clone>(&self, f: fn(U) -> T, default: T) -> T
    where
        for<'a> &'a T: From<&'a PropUnion>,
        for<'a> &'a U: From<&'a PropUnion>,
    {
        self.value::<T>()
            .unwrap_or_else(move || self.value::<U>().map(f).unwrap_or_else(|| default))
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

// TODO: Allow user stylesheet. Don't forget to change doc comment below.
/// Apply styles to a single element, returning the specified styles.
///
/// This is place where cascade part of CSS implemented.
///
/// Following [CSS 2.1
/// specification](https://www.w3.org/TR/2011/REC-CSS2-20110607/cascade.html#cascade)
/// this function first finds all declarations that applicable to the
/// element, then sorts different sources by importance and applies
/// most important on top of less important.
///
/// Specifically, in ascending order of precedence (omitting user stylesheets):
///
/// 1. user agent declarations
/// 1. user normal declarations
/// 1. author normal declarations
/// 1. author important declarations
/// 1. user important declarations
fn specified_values(elem: &ElementRef<Node>, stylesheet: &StyleSheet) -> Props {
    let mut props = Props::new();
    // Apply User Agent declarations
    let user_agent_stylesheet = &*crate::css::DEAFULT_STYLESHEET;
    let mut user_agent_rules = matching_rules(elem, user_agent_stylesheet);
    user_agent_rules.sort_by(|&(a, _), &(b, _)| a.cmp(&b));
    user_agent_rules.sort_by(|&(a, _), &(b, _)| a.cmp(&b));
    for (_, rule) in user_agent_rules {
        // We assume that there is no important rules in user agent stylesheet.
        let rule_props = &rule.declarations;
        props.extend(rule_props);
    }

    // Get Author declarations
    let mut rules = matching_rules(elem, stylesheet);
    // Go through the rules from lowest to highest specificity.
    rules.sort_by(|&(a, _), &(b, _)| a.cmp(&b));

    // NOTE: Do not merge those two loops, it will make semantics incorrect. For example
    // ```css
    // div { display: block !important; }
    // div { padding: 12px; display: inline; }
    // ```
    // Will make div inline if we merge those loops.

    // Assign regular properties
    for (_, rule) in &rules {
        let rule_props = &rule.declarations;
        props.extend(rule_props);
    }

    // Override regular author properties with important ones.
    for (_, rule) in &rules {
        let important_rule_props = &rule.important_declarations;
        props.extend(important_rule_props);
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
