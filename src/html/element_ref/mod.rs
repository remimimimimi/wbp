//! Element references.

use std::fmt::{self, Debug};
use std::iter::FusedIterator;
use std::ops::Deref;

use ego_tree::iter::{Edge, Traverse};
use ego_tree::NodeRef;
use selectors::matching::SelectorCaches;

use crate::html::node::Element;
use crate::html::{Node, SelectorGroup};

use super::node;

/// Trait that represents any kind of node that can be treated as
/// element. Useful for allowing using selectors for different
/// representations of a DOM.
pub trait ElementNode {
    /// Check if it is really an element node.
    fn is_element(&self) -> bool;

    /// Check if it is a document.
    fn is_document(&self) -> bool;

    /// Check if it is a text.
    fn is_text(&self) -> bool;

    /// Get element out of the node.
    fn as_element(&self) -> &Element;

    /// Get element out of the node.
    fn as_text(&self) -> Option<&node::Text>;
}

impl ElementNode for Node {
    fn is_element(&self) -> bool {
        self.is_element()
    }

    fn is_document(&self) -> bool {
        self.is_document()
    }

    fn is_text(&self) -> bool {
        self.is_text()
    }

    fn as_element(&self) -> &Element {
        self.as_element().unwrap()
    }

    fn as_text(&self) -> Option<&node::Text> {
        self.as_text()
    }
}

/// Wrapper around a reference to an element node.
///
/// This wrapper implements the `Element` trait from the `selectors` crate, which allows it to be
/// matched against CSS selectors.
#[derive(Copy, PartialEq, Eq)]
pub struct ElementRef<'a, E: ElementNode> {
    node: NodeRef<'a, E>,
}

impl<E: ElementNode> Clone for ElementRef<'_, E> {
    fn clone(&self) -> Self {
        Self { node: self.node }
    }
}

impl<'a, E: ElementNode> ElementRef<'a, E> {
    fn new(node: NodeRef<'a, E>) -> Self {
        ElementRef { node }
    }

    /// Wraps a `NodeRef` only if it references a `Node::Element`.
    pub fn wrap(node: NodeRef<'a, E>) -> Option<ElementRef<'a, E>> {
        if node.value().is_element() {
            Some(ElementRef::new(node))
        } else {
            None
        }
    }

    /// Returns the `Element` referenced by `self`.
    pub fn value(&self) -> &'a Element {
        self.node.value().as_element()
    }

    /// Returns an iterator over descendent elements matching a selector.
    pub fn select<'b>(&self, selector: &'b SelectorGroup) -> Select<'a, 'b, E> {
        let mut inner = self.traverse();
        inner.next(); // Skip Edge::Open(self).

        Select {
            scope: self.clone(),
            inner,
            selector,
            caches: Default::default(),
        }
    }

    // fn serialize(&self, traversal_scope: TraversalScope) -> String {
    //     let opts = SerializeOpts {
    //         scripting_enabled: false, // It's not clear what this does.
    //         traversal_scope,
    //         create_missing_parent: false,
    //     };
    //     let mut buf = Vec::new();
    //     serialize(&mut buf, self, opts).unwrap();
    //     String::from_utf8(buf).unwrap()
    // }

    // /// Returns the HTML of this element.
    // pub fn html(&self) -> String {
    //     self.serialize(TraversalScope::IncludeNode)
    // }

    // /// Returns the inner HTML of this element.
    // pub fn inner_html(&self) -> String {
    //     self.serialize(TraversalScope::ChildrenOnly(None))
    // }

    /// Returns the value of an attribute.
    pub fn attr(&self, attr: &str) -> Option<&'a str> {
        self.value().attr(attr)
    }

    /// Returns an iterator over descendent text nodes.
    pub fn text(&self) -> Text<'a, E> {
        Text {
            inner: self.traverse(),
        }
    }

    /// Iterate over all child nodes which are elements
    ///
    /// # Example
    ///
    /// ```
    /// # use scraper::Html;
    /// let fragment = Html::parse_fragment("foo<span>bar</span><a>baz</a>qux");
    ///
    /// let children = fragment.root_element().child_elements().map(|element| element.value().name()).collect::<Vec<_>>();
    /// assert_eq!(children, ["span", "a"]);
    /// ```
    pub fn child_elements(&self) -> impl Iterator<Item = ElementRef<'a, E>> {
        self.children().filter_map(ElementRef::wrap)
    }

    /// Iterate over all descendent nodes which are elements
    ///
    /// # Example
    ///
    /// ```
    /// # use scraper::Html;
    /// let fragment = Html::parse_fragment("foo<span><b>bar</b></span><a><i>baz</i></a>qux");
    ///
    /// let descendants = fragment.root_element().descendent_elements().map(|element| element.value().name()).collect::<Vec<_>>();
    /// assert_eq!(descendants, ["html", "span", "b", "a", "i"]);
    /// ```
    pub fn descendent_elements(&self) -> impl Iterator<Item = ElementRef<'a, E>> {
        self.descendants().filter_map(ElementRef::wrap)
    }
}

impl<E: ElementNode> Debug for ElementRef<'_, E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Debug::fmt(self.value(), f)
    }
}

impl<'a, E: ElementNode> Deref for ElementRef<'a, E> {
    type Target = NodeRef<'a, E>;
    fn deref(&self) -> &NodeRef<'a, E> {
        &self.node
    }
}

/// Iterator over descendent elements matching a selector.
pub struct Select<'a, 'b, E: ElementNode> {
    scope: ElementRef<'a, E>,
    inner: Traverse<'a, E>,
    selector: &'b SelectorGroup,
    caches: SelectorCaches,
}

impl<E: ElementNode + Debug> Debug for Select<'_, '_, E> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("Select")
            .field("scope", &self.scope)
            .field("inner", &self.inner)
            .field("selector", &self.selector)
            .field("caches", &"..")
            .finish()
    }
}

impl<E: ElementNode> Clone for Select<'_, '_, E> {
    fn clone(&self) -> Self {
        Self {
            scope: self.scope.clone(),
            inner: self.inner.clone(),
            selector: self.selector,
            caches: Default::default(),
        }
    }
}

impl<'a, E: ElementNode + Clone> Iterator for Select<'a, '_, E> {
    type Item = ElementRef<'a, E>;

    fn next(&mut self) -> Option<ElementRef<'a, E>> {
        for edge in &mut self.inner {
            if let Edge::Open(node) = edge {
                if let Some(element) = ElementRef::wrap(node) {
                    if self.selector.matches_with_scope_and_cache(
                        &element,
                        Some(self.scope.clone()),
                        &mut self.caches,
                    ) {
                        return Some(element);
                    }
                }
            }
        }
        None
    }
}

impl<E: ElementNode + Clone> FusedIterator for Select<'_, '_, E> {}

/// Iterator over descendent text nodes.
#[derive(Debug, Clone)]
pub struct Text<'a, E: ElementNode> {
    inner: Traverse<'a, E>,
}

impl<'a, E: ElementNode> Iterator for Text<'a, E> {
    type Item = &'a str;

    fn next(&mut self) -> Option<&'a str> {
        for edge in &mut self.inner {
            if let Edge::Open(node) = edge {
                if let Some(text) = node.value().as_text() {
                    return Some(&**text);
                }
            }
        }
        None
    }
}

impl<E: ElementNode> FusedIterator for Text<'_, E> {}

mod element;
// mod serializable;

// #[cfg(test)]
// mod tests {
//     use crate::html::selector::Selector;
//     use crate::html::Html;

//     #[test]
//     fn test_scope() {
//         let html = r"
//             <div>
//                 <b>1</b>
//                 <span>
//                     <span><b>2</b></span>
//                     <b>3</b>
//                 </span>
//             </div>
//         ";
//         let fragment = Html::parse_fragment(html);
//         let sel1 = Selector::parse("div > span").unwrap();
//         let sel2 = Selector::parse(":scope > b").unwrap();

//         let element1 = fragment.select(&sel1).next().unwrap();
//         let element2 = element1.select(&sel2).next().unwrap();
//         assert_eq!(element2.inner_html(), "3");
//     }
// }
