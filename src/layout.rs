//! Basic CSS block layout.

use crate::css::{
    Unit::Px,
    Value::{Keyword, Length},
};
use crate::style::{Display, StyledNode};
use std::default::Default;

pub use self::BoxType::{AnonymousBlock, BlockNode, InlineNode};

use ego_tree::*;

// CSS box model. All sizes are in px.

#[derive(Clone, Copy, Default, Debug)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Clone, Copy, Default, Debug)]
pub struct Dimensions {
    /// Position of the content area relative to the document origin:
    pub content: Rect,
    // Surrounding edges:
    pub padding: EdgeSizes,
    pub border: EdgeSizes,
    pub margin: EdgeSizes,
}

#[derive(Clone, Copy, Default, Debug)]
pub struct EdgeSizes {
    pub left: f32,
    pub right: f32,
    pub top: f32,
    pub bottom: f32,
}

/// A node in the layout tree.
#[derive(Clone, Debug)]
pub struct LayoutBox {
    pub dimensions: Dimensions,
    pub box_type: BoxType,
}

type LayoutTree = Tree<LayoutBox>;

#[derive(Clone, Debug, PartialEq)]
pub enum BoxType {
    BlockNode(StyledNode),
    InlineNode(StyledNode),
    AnonymousBlock,
}

impl LayoutBox {
    fn new(box_type: BoxType) -> LayoutBox {
        LayoutBox {
            box_type,
            dimensions: Default::default(), // initially set all fields to 0.0
        }
    }

    fn get_style_node(&self) -> Option<&StyledNode> {
        match &self.box_type {
            BlockNode(node) | InlineNode(node) => Some(node),
            AnonymousBlock => None,
        }
    }
}

/// Transform a style tree into a layout tree.
pub fn layout_tree(
    root_style_node: NodeRef<'_, StyledNode>,
    mut containing_block: Dimensions,
) -> LayoutTree {
    // The layout algorithm expects the container height to start at 0.
    // TODO: Save the initial containing block height, for calculating percent heights.
    containing_block.content.height = 0.0;

    // TODO: Change to Tree::with_capacity.
    let mut layout_tree = Tree::new(LayoutBox::new(match root_style_node.value().display() {
        Display::Block => BlockNode(root_style_node.value().clone()),
        Display::Inline => InlineNode(root_style_node.value().clone()),
        Display::None => panic!("Root node has display: none."),
    }));

    build_layout_tree(layout_tree.root_mut(), root_style_node);
    layout_tree.root_mut().layout(containing_block);
    layout_tree
}

/// Build the tree of LayoutBoxes, but don't perform any layout calculations yet.
fn build_layout_tree<'a>(
    mut layout_box_node: NodeMut<'a, LayoutBox>,
    style_node: NodeRef<'a, StyledNode>,
) {
    for child in style_node.children() {
        match child.value().display() {
            Display::Block => build_layout_tree(
                layout_box_node.append(LayoutBox::new(BlockNode(child.value().clone()))),
                child,
            ),
            Display::Inline => build_layout_tree(
                layout_box_node
                    .get_inline_container()
                    .append(LayoutBox::new(InlineNode(child.value().clone()))),
                child,
            ),
            Display::None => {}
        }
    }
}

/// Ad-hoc trait to extend `NodeMut<'_, LayoutBox>` type.
pub trait Layoutable {
    /// Lay out a box and its descendants.
    fn layout(&mut self, containing_block: Dimensions);

    /// Lay out a block-level element and its descendants.
    fn layout_block(&mut self, containing_block: Dimensions);

    /// Calculate the width of a block-level non-replaced element in normal flow.
    ///
    /// http://www.w3.org/TR/CSS2/visudet.html#blockwidth
    ///
    /// Sets the horizontal margin/padding/border dimensions, and the `width`.
    fn calculate_block_width(&mut self, containing_block: Dimensions);

    /// Finish calculating the block's edge sizes, and position it within its containing block.
    ///
    /// http://www.w3.org/TR/CSS2/visudet.html#normal-block
    ///
    /// Sets the vertical margin/padding/border dimensions, and the `x`, `y` values.
    fn calculate_block_position(&mut self, containing_block: Dimensions);

    /// Lay out the block's children within its content area.
    // Sets `self.dimensions.height` to the total content height.
    fn layout_block_children(&mut self);

    /// Height of a block-level non-replaced element in normal flow with overflow visible.
    fn calculate_block_height(&mut self);

    /// Where a new inline child should go.
    fn get_inline_container(&mut self) -> NodeMut<'_, LayoutBox>;
}

impl Layoutable for NodeMut<'_, LayoutBox> {
    fn layout(&mut self, containing_block: Dimensions) {
        // TODO: Support other display types
        if let BlockNode(_) = &self.value().box_type {
            self.layout_block(containing_block)
        }
    }

    fn layout_block(&mut self, containing_block: Dimensions) {
        // Child width can depend on parent width, so we need to calculate this box's width before
        // laying out its children.
        self.calculate_block_width(containing_block);

        // Determine where the box is located within its container.
        self.calculate_block_position(containing_block);

        // Recursively lay out the children of this box.
        self.layout_block_children();

        // Parent height can depend on child height, so `calculate_height` must be called after the
        // children are laid out.
        self.calculate_block_height();
    }

    fn calculate_block_width(&mut self, containing_block: Dimensions) {
        let style = self.value().get_style_node().unwrap();

        // `width` has initial value `auto`.
        let auto = Keyword("auto".to_string());
        let mut width = style.value("width").unwrap_or(auto.clone());

        // margin, border, and padding have initial value 0.
        let zero = Length(0.0, Px);

        let mut margin_left = style.lookup("margin-left", "margin", &zero);
        let mut margin_right = style.lookup("margin-right", "margin", &zero);

        let border_left = style.lookup("border-left-width", "border-width", &zero);
        let border_right = style.lookup("border-right-width", "border-width", &zero);

        let padding_left = style.lookup("padding-left", "padding", &zero);
        let padding_right = style.lookup("padding-right", "padding", &zero);

        let total: f32 = [
            &margin_left,
            &margin_right,
            &border_left,
            &border_right,
            &padding_left,
            &padding_right,
            &width,
        ]
        .iter()
        .map(|v| v.to_px())
        .sum();

        // If width is not auto and the total is wider than the container, treat auto margins as 0.
        if width != auto && total > containing_block.content.width {
            if margin_left == auto {
                margin_left = Length(0.0, Px);
            }
            if margin_right == auto {
                margin_right = Length(0.0, Px);
            }
        }

        // Adjust used values so that the above sum equals `containing_block.width`.
        // Each arm of the `match` should increase the total width by exactly `underflow`,
        // and afterward all values should be absolute lengths in px.
        let underflow = containing_block.content.width - total;

        match (width == auto, margin_left == auto, margin_right == auto) {
            // If the values are overconstrained, calculate margin_right.
            (false, false, false) => {
                margin_right = Length(margin_right.to_px() + underflow, Px);
            }

            // If exactly one size is auto, its used value follows from the equality.
            (false, false, true) => {
                margin_right = Length(underflow, Px);
            }
            (false, true, false) => {
                margin_left = Length(underflow, Px);
            }

            // If width is set to auto, any other auto values become 0.
            (true, _, _) => {
                if margin_left == auto {
                    margin_left = Length(0.0, Px);
                }
                if margin_right == auto {
                    margin_right = Length(0.0, Px);
                }

                if underflow >= 0.0 {
                    // Expand width to fill the underflow.
                    width = Length(underflow, Px);
                } else {
                    // Width can't be negative. Adjust the right margin instead.
                    width = Length(0.0, Px);
                    margin_right = Length(margin_right.to_px() + underflow, Px);
                }
            }

            // If margin-left and margin-right are both auto, their used values are equal.
            (false, true, true) => {
                margin_left = Length(underflow / 2.0, Px);
                margin_right = Length(underflow / 2.0, Px);
            }
        }

        let d = &mut self.value().dimensions;
        d.content.width = width.to_px();

        d.padding.left = padding_left.to_px();
        d.padding.right = padding_right.to_px();

        d.border.left = border_left.to_px();
        d.border.right = border_right.to_px();

        d.margin.left = margin_left.to_px();
        d.margin.right = margin_right.to_px();
    }

    fn calculate_block_position(&mut self, containing_block: Dimensions) {
        let v = self.value();
        let style = v.get_style_node().unwrap().clone();
        let d = &mut v.dimensions;

        // margin, border, and padding have initial value 0.
        let zero = Length(0.0, Px);

        // If margin-top or margin-bottom is `auto`, the used value is zero.
        d.margin.top = style.lookup("margin-top", "margin", &zero).to_px();
        d.margin.bottom = style.lookup("margin-bottom", "margin", &zero).to_px();

        d.border.top = style
            .lookup("border-top-width", "border-width", &zero)
            .to_px();
        d.border.bottom = style
            .lookup("border-bottom-width", "border-width", &zero)
            .to_px();

        d.padding.top = style.lookup("padding-top", "padding", &zero).to_px();
        d.padding.bottom = style.lookup("padding-bottom", "padding", &zero).to_px();

        d.content.x = containing_block.content.x + d.margin.left + d.border.left + d.padding.left;

        // Position the box below all the previous boxes in the container.
        d.content.y = containing_block.content.height
            + containing_block.content.y
            + d.margin.top
            + d.border.top
            + d.padding.top;
    }

    fn layout_block_children(&mut self) {
        self.for_each_child(|child| {
            let parent_dimensions = child.parent().unwrap().value().dimensions;
            child.layout(parent_dimensions);

            let child_layouted_dimensions = child.value().dimensions;
            // SAFE: We already checked that parent is in there.
            let mut parent = unsafe { child.parent().unwrap_unchecked() };
            parent.value().dimensions.content.height +=
                child_layouted_dimensions.margin_box().height;
        });
    }

    fn calculate_block_height(&mut self) {
        // If the height is set to an explicit length, use that exact length.
        // Otherwise, just keep the value set by `layout_block_children`.
        if let Some(Length(h, Px)) = self.value().get_style_node().unwrap().value("height") {
            self.value().dimensions.content.height = h;
        }
    }

    fn get_inline_container(&mut self) -> NodeMut<'_, LayoutBox> {
        let id = self.id();
        let mut slf0 = self.tree().get_mut(id).unwrap();
        match &slf0.value().box_type {
            InlineNode(_) | AnonymousBlock => slf0,
            BlockNode(_) => match slf0.into_last_child() {
                Ok(mut n) => {
                    if n.value().box_type == AnonymousBlock {
                        n
                    } else {
                        let _ = n.insert_after(LayoutBox::new(AnonymousBlock));
                        n.into_next_sibling().unwrap()
                    }
                }
                Err(mut slf) => {
                    slf.append(LayoutBox::new(AnonymousBlock));
                    slf.into_last_child().unwrap()
                }
            },
        }
    }
}

impl Rect {
    pub fn expanded_by(self, edge: EdgeSizes) -> Rect {
        Rect {
            x: self.x - edge.left,
            y: self.y - edge.top,
            width: self.width + edge.left + edge.right,
            height: self.height + edge.top + edge.bottom,
        }
    }
}

impl Dimensions {
    /// The area covered by the content area plus its padding.
    pub fn padding_box(self) -> Rect {
        self.content.expanded_by(self.padding)
    }
    /// The area covered by the content area plus padding and borders.
    pub fn border_box(self) -> Rect {
        self.padding_box().expanded_by(self.border)
    }
    /// The area covered by the content area plus padding, borders, and margin.
    pub fn margin_box(self) -> Rect {
        self.border_box().expanded_by(self.margin)
    }
}
