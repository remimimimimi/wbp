//! HTML parsing and querying with CSS selectors.
//!
//! `scraper` is on [Crates.io][crate] and [GitHub][github].
//!
//! [crate]: https://crates.io/crates/scraper
//! [github]: https://github.com/programble/scraper
//!
//! Scraper provides an interface to Servo's `html5ever` and `selectors` crates, for browser-grade
//! parsing and querying.
//!
//! # Examples
//!
//! ## Parsing a document
//!
//! ```
//! use scraper::Html;
//!
//! let html = r#"
//!     <!DOCTYPE html>
//!     <meta charset="utf-8">
//!     <title>Hello, world!</title>
//!     <h1 class="foo">Hello, <i>world!</i></h1>
//! "#;
//!
//! let document = Html::parse_document(html);
//! ```
//!
//! ## Parsing a fragment
//!
//! ```
//! use scraper::Html;
//! let fragment = Html::parse_fragment("<h1>Hello, <i>world!</i></h1>");
//! ```
//!
//! ## Parsing a selector
//!
//! ```
//! use scraper::Selector;
//! let selector = Selector::parse("h1.foo").unwrap();
//! ```
//!
//! ## Selecting elements
//!
//! ```
//! use scraper::{Html, Selector};
//!
//! let html = r#"
//!     <ul>
//!         <li>Foo</li>
//!         <li>Bar</li>
//!         <li>Baz</li>
//!     </ul>
//! "#;
//!
//! let fragment = Html::parse_fragment(html);
//! let selector = Selector::parse("li").unwrap();
//!
//! for element in fragment.select(&selector) {
//!     assert_eq!("li", element.value().name());
//! }
//! ```
//!
//! ## Selecting descendent elements
//!
//! ```
//! use scraper::{Html, Selector};
//!
//! let html = r#"
//!     <ul>
//!         <li>Foo</li>
//!         <li>Bar</li>
//!         <li>Baz</li>
//!     </ul>
//! "#;
//!
//! let fragment = Html::parse_fragment(html);
//! let ul_selector = Selector::parse("ul").unwrap();
//! let li_selector = Selector::parse("li").unwrap();
//!
//! let ul = fragment.select(&ul_selector).next().unwrap();
//! for element in ul.select(&li_selector) {
//!     assert_eq!("li", element.value().name());
//! }
//! ```
//!
//! ## Accessing element attributes
//!
//! ```
//! use scraper::{Html, Selector};
//!
//! let fragment = Html::parse_fragment(r#"<input name="foo" value="bar">"#);
//! let selector = Selector::parse(r#"input[name="foo"]"#).unwrap();
//!
//! let input = fragment.select(&selector).next().unwrap();
//! assert_eq!(Some("bar"), input.value().attr("value"));
//! ```
//!
//! ## Serializing HTML and inner HTML
//!
//! ```
//! use scraper::{Html, Selector};
//!
//! let fragment = Html::parse_fragment("<h1>Hello, <i>world!</i></h1>");
//! let selector = Selector::parse("h1").unwrap();
//!
//! let h1 = fragment.select(&selector).next().unwrap();
//!
//! assert_eq!("<h1>Hello, <i>world!</i></h1>", h1.html());
//! assert_eq!("Hello, <i>world!</i>", h1.inner_html());
//! ```
//!
//! ## Accessing descendent text
//!
//! ```
//! use scraper::{Html, Selector};
//!
//! let fragment = Html::parse_fragment("<h1>Hello, <i>world!</i></h1>");
//! let selector = Selector::parse("h1").unwrap();
//!
//! let h1 = fragment.select(&selector).next().unwrap();
//! let text = h1.text().collect::<Vec<_>>();
//!
//! assert_eq!(vec!["Hello, ", "world!"], text);
//! ```

#![warn(
    missing_docs,
    missing_debug_implementations,
    missing_copy_implementations,
    trivial_casts,
    trivial_numeric_casts,
    unused_extern_crates,
    unused_import_braces,
    unused_qualifications,
    variant_size_differences
)]

pub use crate::html::element_ref::ElementRef;
pub use crate::html::node::{Element, Node};
pub use selectors::attr::CaseSensitivity;
pub use tendril_util::StrTendril;

pub mod element_ref;
pub mod error;
pub mod node;
pub mod selectable;

pub(crate) mod tendril_util {
    use html5ever::tendril;
    /// Primary string tendril type.
    pub type StrTendril = tendril::StrTendril;

    /// Return unaltered.
    pub fn make(s: StrTendril) -> StrTendril {
        s
    }
}

#[cfg(feature = "errors")]
use std::borrow::Cow;
use std::fmt;
use std::iter::FusedIterator;

use ego_tree::iter::Nodes;
use ego_tree::Tree;
use html5ever::tree_builder::QuirksMode;
use html5ever::{driver, QualName};
use html5ever::{local_name, namespace_url, ns};
use selectors::matching::SelectorCaches;
use tendril::TendrilSink;

use crate::html::element_ref::ElementNode;
use crate::selector::SelectorGroup;

/// An HTML tree.
///
/// Parsing does not fail hard. Instead, the `quirks_mode` is set and errors are added to the
/// `errors` field. The `tree` will still be populated as best as possible.
///
/// Implements the `TreeSink` trait from the `html5ever` crate, which allows HTML to be parsed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Html {
    #[cfg(feature = "errors")]
    /// Parse errors.
    pub errors: Vec<Cow<'static, str>>,

    /// The quirks mode.
    pub quirks_mode: QuirksMode,

    /// The node tree.
    pub tree: Tree<Node>,
}

impl Html {
    /// Creates an empty HTML document.
    pub fn new_document() -> Self {
        Html {
            #[cfg(feature = "errors")]
            errors: Vec::new(),
            quirks_mode: QuirksMode::NoQuirks,
            tree: Tree::new(Node::Document),
        }
    }

    /// Creates an empty HTML fragment.
    pub fn new_fragment() -> Self {
        Html {
            #[cfg(feature = "errors")]
            errors: Vec::new(),
            quirks_mode: QuirksMode::NoQuirks,
            tree: Tree::new(Node::Fragment),
        }
    }

    /// Parses a string of HTML as a document.
    ///
    /// This is a convenience method for the following:
    ///
    /// ```
    /// # extern crate html5ever;
    /// # extern crate scraper;
    /// # extern crate tendril;
    /// # fn main() {
    /// # let document = "";
    /// use html5ever::driver::{self, ParseOpts};
    /// use scraper::{Html, HtmlTreeSink};
    /// use tendril::TendrilSink;
    ///
    /// let parser = driver::parse_document(HtmlTreeSink::new(Html::new_document()), ParseOpts::default());
    /// let html = parser.one(document);
    /// # }
    /// ```
    pub fn parse_document(document: &str) -> Self {
        let parser =
            driver::parse_document(HtmlTreeSink::new(Self::new_document()), Default::default());
        parser.one(document)
    }

    /// Parses a string of HTML as a fragment.
    pub fn parse_fragment(fragment: &str) -> Self {
        let parser = driver::parse_fragment(
            HtmlTreeSink::new(Self::new_fragment()),
            Default::default(),
            QualName::new(None, ns!(html), local_name!("body")),
            Vec::new(),
        );
        parser.one(fragment)
    }

    /// Returns an iterator over elements matching a selector.
    pub fn select<'a, 'b>(&'a self, selector: &'b SelectorGroup) -> Select<'a, 'b, Node> {
        Select {
            inner: self.tree.nodes(),
            selector,
            caches: Default::default(),
        }
    }

    /// Returns the root `<html>` element.
    pub fn root_element(&self) -> ElementRef<Node> {
        let root_node = self
            .tree
            .root()
            .children()
            .find(|child| child.value().is_element())
            .expect("html node missing");
        ElementRef::wrap(root_node).unwrap()
    }

    // /// Serialize entire document into HTML.
    // pub fn html(&self) -> String {
    //     let opts = SerializeOpts {
    //         scripting_enabled: false, // It's not clear what this does.
    //         traversal_scope: serialize::TraversalScope::IncludeNode,
    //         create_missing_parent: false,
    //     };
    //     let mut buf = Vec::new();
    //     serialize(&mut buf, self, opts).unwrap();
    //     String::from_utf8(buf).unwrap()
    // }
}

/// Iterator over elements matching a selector.
pub struct Select<'a, 'b, E: ElementNode> {
    inner: Nodes<'a, E>,
    selector: &'b SelectorGroup,
    caches: SelectorCaches,
}

impl<E: ElementNode + fmt::Debug> fmt::Debug for Select<'_, '_, E> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("Select")
            .field("inner", &self.inner)
            .field("selector", &self.selector)
            .field("caches", &"..")
            .finish()
    }
}

impl<E: ElementNode> Clone for Select<'_, '_, E> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            selector: self.selector,
            caches: Default::default(),
        }
    }
}

impl<'a, E: ElementNode + Clone> Iterator for Select<'a, '_, E> {
    type Item = ElementRef<'a, E>;

    fn next(&mut self) -> Option<ElementRef<'a, E>> {
        for node in self.inner.by_ref() {
            if let Some(element) = ElementRef::wrap(node) {
                if element.parent().is_some()
                    && self
                        .selector
                        .matches_with_scope_and_cache(&element, None, &mut self.caches)
                {
                    return Some(element);
                }
            }
        }
        None
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let (_lower, upper) = self.inner.size_hint();

        (0, upper)
    }
}

impl<E: ElementNode + Clone> DoubleEndedIterator for Select<'_, '_, E> {
    fn next_back(&mut self) -> Option<Self::Item> {
        for node in self.inner.by_ref().rev() {
            if let Some(element) = ElementRef::wrap(node) {
                if element.parent().is_some()
                    && self
                        .selector
                        .matches_with_scope_and_cache(&element, None, &mut self.caches)
                {
                    return Some(element);
                }
            }
        }
        None
    }
}

impl<E: ElementNode + Clone> FusedIterator for Select<'_, '_, E> {}

use crate::html::node::{Comment, Doctype, ProcessingInstruction, Text};
use crate::html::tendril_util::make as make_tendril;
use ego_tree::NodeId;
use html5ever::expanded_name;
use html5ever::tree_builder::{ElementFlags, NodeOrText, TreeSink};
use html5ever::Attribute;
use std::cell::{Ref, RefCell};

/// Wraps `Html` instances as sinks to drive parsing
#[derive(Debug)]
pub struct HtmlTreeSink(pub RefCell<Html>);

impl HtmlTreeSink {
    /// Wrap a `Html`instance as a sink to drive parsing
    pub fn new(html: Html) -> Self {
        Self(RefCell::new(html))
    }
}

/// Note: does not support the `<template>` element.
impl TreeSink for HtmlTreeSink {
    type Output = Html;
    type Handle = NodeId;
    type ElemName<'a> = Ref<'a, QualName>;

    fn finish(self) -> Html {
        self.0.into_inner()
    }

    // Signal a parse error.
    fn parse_error(&self, msg: Cow<'static, str>) {
        #[cfg(feature = "errors")]
        self.0.borrow_mut().errors.push(msg);
        #[cfg(not(feature = "errors"))]
        let _ = msg;
    }

    // Set the document's quirks mode.
    fn set_quirks_mode(&self, mode: QuirksMode) {
        self.0.borrow_mut().quirks_mode = mode;
    }

    // Get a handle to the Document node.
    fn get_document(&self) -> Self::Handle {
        self.0.borrow().tree.root().id()
    }

    // Do two handles refer to the same node?
    fn same_node(&self, x: &Self::Handle, y: &Self::Handle) -> bool {
        x == y
    }

    // What is the name of this element?
    //
    // Should never be called on a non-element node; feel free to panic!.
    fn elem_name<'a>(&'a self, target: &Self::Handle) -> Ref<'a, QualName> {
        Ref::map(self.0.borrow(), |this| {
            &this
                .tree
                .get(*target)
                .unwrap()
                .value()
                .as_element()
                .unwrap()
                .name
        })
    }

    // Create an element.
    //
    // When creating a template element (name.expanded() == expanded_name!(html "template")), an
    // associated document fragment called the "template contents" should also be created. Later
    // calls to self.get_template_contents() with that given element return it.
    fn create_element(
        &self,
        name: QualName,
        attrs: Vec<Attribute>,
        _flags: ElementFlags,
    ) -> Self::Handle {
        let fragment = name.expanded() == expanded_name!(html "template");

        let mut this = self.0.borrow_mut();
        let mut node = this.tree.orphan(Node::Element(Element::new(name, attrs)));

        if fragment {
            node.append(Node::Fragment);
        }

        node.id()
    }

    // Create a comment node.
    fn create_comment(&self, text: StrTendril) -> Self::Handle {
        self.0
            .borrow_mut()
            .tree
            .orphan(Node::Comment(Comment {
                comment: make_tendril(text),
            }))
            .id()
    }

    // Append a DOCTYPE element to the Document node.
    fn append_doctype_to_document(
        &self,
        name: StrTendril,
        public_id: StrTendril,
        system_id: StrTendril,
    ) {
        let name = make_tendril(name);
        let public_id = make_tendril(public_id);
        let system_id = make_tendril(system_id);
        let doctype = Doctype {
            name,
            public_id,
            system_id,
        };
        self.0
            .borrow_mut()
            .tree
            .root_mut()
            .append(Node::Doctype(doctype));
    }

    // Append a node as the last child of the given node. If this would produce adjacent sibling
    // text nodes, it should concatenate the text instead.
    //
    // The child node will not already have a parent.
    fn append(&self, parent: &Self::Handle, child: NodeOrText<Self::Handle>) {
        let mut this = self.0.borrow_mut();
        let mut parent = this.tree.get_mut(*parent).unwrap();

        match child {
            NodeOrText::AppendNode(id) => {
                parent.append_id(id);
            }

            NodeOrText::AppendText(text) => {
                let text = make_tendril(text);

                let did_concat = parent.last_child().is_some_and(|mut n| match n.value() {
                    Node::Text(t) => {
                        t.text.push_tendril(&text);
                        true
                    }
                    _ => false,
                });

                if !did_concat {
                    parent.append(Node::Text(Text { text }));
                }
            }
        }
    }

    // Append a node as the sibling immediately before the given node. If that node has no parent,
    // do nothing and return Err(new_node).
    //
    // The tree builder promises that sibling is not a text node. However its old previous sibling,
    // which would become the new node's previous sibling, could be a text node. If the new node is
    // also a text node, the two should be merged, as in the behavior of append.
    //
    // NB: new_node may have an old parent, from which it should be removed.
    fn append_before_sibling(&self, sibling: &Self::Handle, new_node: NodeOrText<Self::Handle>) {
        let mut this = self.0.borrow_mut();

        if let NodeOrText::AppendNode(id) = new_node {
            this.tree.get_mut(id).unwrap().detach();
        }

        let mut sibling = this.tree.get_mut(*sibling).unwrap();
        if sibling.parent().is_some() {
            match new_node {
                NodeOrText::AppendNode(id) => {
                    sibling.insert_id_before(id);
                }

                NodeOrText::AppendText(text) => {
                    let text = make_tendril(text);

                    let did_concat = sibling.prev_sibling().is_some_and(|mut n| match n.value() {
                        Node::Text(t) => {
                            t.text.push_tendril(&text);
                            true
                        }
                        _ => false,
                    });

                    if !did_concat {
                        sibling.insert_before(Node::Text(Text { text }));
                    }
                }
            }
        }
    }

    // Detach the given node from its parent.
    fn remove_from_parent(&self, target: &Self::Handle) {
        self.0.borrow_mut().tree.get_mut(*target).unwrap().detach();
    }

    // Remove all the children from node and append them to new_parent.
    fn reparent_children(&self, node: &Self::Handle, new_parent: &Self::Handle) {
        self.0
            .borrow_mut()
            .tree
            .get_mut(*new_parent)
            .unwrap()
            .reparent_from_id_append(*node);
    }

    // Add each attribute to the given element, if no attribute with that name already exists. The
    // tree builder promises this will never be called with something else than an element.
    fn add_attrs_if_missing(&self, target: &Self::Handle, attrs: Vec<Attribute>) {
        let mut this = self.0.borrow_mut();
        let mut node = this.tree.get_mut(*target).unwrap();
        let element = match *node.value() {
            Node::Element(ref mut e) => e,
            _ => unreachable!(),
        };

        for attr in attrs {
            if let Err(idx) = element
                .attrs
                .binary_search_by(|(name, _)| name.cmp(&attr.name))
            {
                element
                    .attrs
                    .insert(idx, (attr.name, make_tendril(attr.value)));
            }
        }
    }

    // Get a handle to a template's template contents.
    //
    // The tree builder promises this will never be called with something else than a template
    // element.
    fn get_template_contents(&self, target: &Self::Handle) -> Self::Handle {
        self.0
            .borrow()
            .tree
            .get(*target)
            .unwrap()
            .first_child()
            .unwrap()
            .id()
    }

    // Mark a HTML <script> element as "already started".
    fn mark_script_already_started(&self, _node: &Self::Handle) {}

    // Create Processing Instruction.
    fn create_pi(&self, target: StrTendril, data: StrTendril) -> Self::Handle {
        let target = make_tendril(target);
        let data = make_tendril(data);
        self.0
            .borrow_mut()
            .tree
            .orphan(Node::ProcessingInstruction(ProcessingInstruction {
                target,
                data,
            }))
            .id()
    }

    fn append_based_on_parent_node(
        &self,
        element: &Self::Handle,
        prev_element: &Self::Handle,
        child: NodeOrText<Self::Handle>,
    ) {
        let has_parent = self
            .0
            .borrow()
            .tree
            .get(*element)
            .unwrap()
            .parent()
            .is_some();

        if has_parent {
            self.append_before_sibling(element, child)
        } else {
            self.append(prev_element, child)
        }
    }
}

// use std::io::Error;

// use html5ever::serialize::{Serialize, Serializer, TraversalScope};

// use crate::html::Html;

// impl Serialize for Html {
//     fn serialize<S: Serializer>(
//         &self,
//         serializer: &mut S,
//         traversal_scope: TraversalScope,
//     ) -> Result<(), Error> {
//         crate::html::node::serializable::serialize(self.tree.root(), serializer, traversal_scope)
//     }
// }

// #[cfg(test)]
// mod tests {
//     use crate::html::Html;

//     #[test]
//     fn test_serialize() {
//         let src = r#"<!DOCTYPE html><html lang="en"><head><meta charset="utf-8"></head><body><p>Hello world!</p></body></html>"#;
//         let html = Html::parse_document(src);
//         assert_eq!(html.html(), src);
//     }
// }

#[cfg(test)]
mod tests {
    use crate::html::Html;
    use crate::selector::SelectorGroup;

    #[test]
    fn tag_with_newline() {
        let selector = SelectorGroup::parse("a").unwrap();

        let document = Html::parse_fragment(
            r#"
        <a
                            href="https://github.com/causal-agent/scraper">

                            </a>
        "#,
        );

        let mut iter = document.select(&selector);
        let a = iter.next().unwrap();
        assert_eq!(
            a.value().attr("href"),
            Some("https://github.com/causal-agent/scraper")
        );
    }

    // #[test]
    // fn has_selector() {
    //     let document = Html::parse_fragment(
    //         r#"
    //         <div>
    //             <div id="foo">
    //                 Hi There!
    //             </div>
    //         </div>
    //         <ul>
    //             <li>first</li>
    //             <li>second</li>
    //             <li>third</li>
    //         </ul>
    //         "#,
    //     );

    //     let selector = Selector::parse("div:has(div#foo) + ul > li:nth-child(2)").unwrap();

    //     let mut iter = document.select(&selector);
    //     let li = iter.next().unwrap();
    //     assert_eq!(li.inner_html(), "second");
    // }

    #[test]
    fn root_element_fragment() {
        let html = Html::parse_fragment(r#"<a href="http://github.com">1</a>"#);
        let root_ref = html.root_element();
        let href = root_ref
            .select(&SelectorGroup::parse("a").unwrap())
            .next()
            .unwrap();
        // assert_eq!(href.inner_html(), "1");
        assert_eq!(href.value().attr("href").unwrap(), "http://github.com");
    }

    // #[test]
    // fn root_element_document_doctype() {
    //     let html = Html::parse_document("<!DOCTYPE html>\n<title>abc</title>");
    //     let root_ref = html.root_element();
    //     let title = root_ref
    //         .select(&Selector::parse("title").unwrap())
    //         .next()
    //         .unwrap();
    //     assert_eq!(title.inner_html(), "abc");
    // }

    // #[test]
    // fn root_element_document_comment() {
    //     let html = Html::parse_document("<!-- comment --><title>abc</title>");
    //     let root_ref = html.root_element();
    //     let title = root_ref
    //         .select(&Selector::parse("title").unwrap())
    //         .next()
    //         .unwrap();
    //     assert_eq!(title.inner_html(), "abc");
    // }

    // #[test]
    // fn select_is_reversible() {
    //     let html = Html::parse_document("<p>element1</p><p>element2</p><p>element3</p>");
    //     let selector = Selector::parse("p").unwrap();
    //     let result: Vec<_> = html
    //         .select(&selector)
    //         .rev()
    //         .map(|e| e.inner_html())
    //         .collect();
    //     assert_eq!(result, vec!["element3", "element2", "element1"]);
    // }

    #[test]
    fn select_has_a_size_hint() {
        let html = Html::parse_document("<p>element1</p><p>element2</p><p>element3</p>");
        let selector = SelectorGroup::parse("p").unwrap();
        let (lower, upper) = html.select(&selector).size_hint();
        assert_eq!(lower, 0);
        assert_eq!(upper, Some(10));
    }
}
