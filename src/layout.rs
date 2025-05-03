//! Basic CSS block layout.

use crate::css::{
    Unit::Px,
    Value::{Keyword, Length},
};
use crate::style::{Display, StyledNode};
use std::{default::Default, vec};

pub use self::BoxType::{AnonymousBlock, BlockNode, InlineNode, LineBox};

use ego_tree::*;
use log::trace;

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
    // only created in block containers
    LineBox,
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
            AnonymousBlock | LineBox => None,
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

    // Lay out an anonymous block element and its descendants (inline elements for now)
    fn layout_anonymous(&mut self, containing_block: Dimensions);

    /// Lay out a block-level element and its descendants.
    fn layout_block(&mut self, containing_block: Dimensions);

    /// Lay out a line box and its descendants (inline elements for now)
    fn layout_linebox(&mut self, containing_block: Dimensions);

    fn calculate_linebox_position(&mut self, containing_block: Dimensions);

    /// lay out
    fn layout_inline(&mut self, containing_block: Dimensions);

    fn layout_inline_children(&mut self, containing_block: Dimensions);

    fn calculate_inline_position(&mut self, containing_block: Dimensions);

    fn calculate_inline_width(&mut self);

    fn calculate_inline_height(&mut self);

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
        match self.value().box_type {
            BlockNode(_) => self.layout_block(containing_block),
            AnonymousBlock => self.layout_anonymous(containing_block),
            LineBox => self.layout_linebox(containing_block),
            InlineNode(_) => self.layout_inline(containing_block),
        }
    }

    fn layout_linebox(&mut self, containing_block: Dimensions) {
        // width and height should be known by now
        // TODO: write a separate function for position calculation
        self.calculate_linebox_position(containing_block);
        let d = self.value().dimensions;
        // TODO: write a separate function for children layouting
        // method call layouting for InlineNode or text
        self.for_each_child(|c| {
            c.layout(d);
        });
    }

    fn calculate_linebox_position(&mut self, containing_block: Dimensions) {
        let d = &mut self.value().dimensions;
        d.content.x = containing_block.content.x + d.margin.left + d.border.left + d.padding.left;

        // Position the box below all the previous boxes in the container.
        d.content.y = containing_block.content.height
            + containing_block.content.y
            + d.margin.top
            + d.border.top
            + d.padding.top;
    }

    fn layout_inline(&mut self, containing_block: Dimensions) {
        self.calculate_inline_position(containing_block);
        self.calculate_inline_width();
        self.calculate_inline_height();

        let d = self.value().dimensions;

        self.layout_inline_children(d);
    }

    fn calculate_inline_position(&mut self, containing_block: Dimensions) {
        let mut accumulated_width = 0.0;
        self.for_each_prev_sibling(|s| {
            // TODO: generalize the width calculation, because inline elements can also have TextBox as children
            s.calculate_inline_width();
            accumulated_width += s.value().dimensions.margin_box().width;
        });
        let d = &mut self.value().dimensions;
        d.content.x = containing_block.content.x
            + accumulated_width
            + d.margin.left
            + d.border.left
            + d.padding.left;
        self.value().dimensions.content.y =
            containing_block.content.y + d.margin.top + d.border.top + d.padding.top;
    }

    fn layout_inline_children(&mut self, containing_block: Dimensions) {
        self.for_each_child(|c| {
            c.layout(containing_block);
        });
    }

    fn layout_anonymous(&mut self, containing_block: Dimensions) {
        // TODO: write a separate function for the width calculation
        let d = &mut self.value().dimensions;
        let total = d.margin_box().width - d.content.width;
        d.content.width = containing_block.content.width - total;
        d.content.height = 0.0;
        // TODO: Write a separate function for position calculation
        d.content.x = containing_block.content.x + d.margin.left + d.border.left + d.padding.left;

        // Position the box below all the previous boxes in the container.
        d.content.y = containing_block.content.height
            + containing_block.content.y
            + d.margin.top
            + d.border.top
            + d.padding.top;

        // TODO: write a separate function for layouting children
        // LineBoxes have the same width as its containing anonymous blocks

        let anonymous_block_width = d.content.width;

        let children_ids = self
            .as_ref()
            .children()
            .map(|c| c.id())
            .collect::<Vec<NodeId>>();

        let mut accumulated_width = 0.0;

        let mut nodes_ids_per_line: Vec<Vec<NodeId>> = vec![vec![]];
        let mut line_index = 0;
        for child_id in children_ids.iter() {
            let mut inline_box = self.tree().get_mut(*child_id).unwrap();
            // TODO: recursively ask the inline box to find its width
            inline_box.calculate_inline_width();
            let inline_box_width = inline_box.value().dimensions.margin_box().width;
            dbg!(
                inline_box_width,
                inline_box.value().dimensions.content.width
            );
            // TODO: for TextBox and possibly other types fragmentation should occur here
            if accumulated_width + inline_box_width >= anonymous_block_width {
                line_index += 1;
                nodes_ids_per_line.push(vec![]);
                nodes_ids_per_line[line_index].push(*child_id);
                accumulated_width = inline_box_width;
            } else {
                accumulated_width += inline_box_width;
                nodes_ids_per_line[line_index].push(*child_id);
            }
        }

        for child_id in children_ids {
            let mut child = self.tree().get_mut(child_id).unwrap();
            child.detach();
        }

        for line_inline_box_ids in nodes_ids_per_line {
            // Calculate line box dimensions.

            // TODO: calculate the width and the height of the LineBox in separate functions
            let mut maximum_inline_box_height = 0.0;
            for inline_box_id in line_inline_box_ids.clone() {
                let mut inline_box = self.tree().get_mut(inline_box_id).unwrap();
                inline_box.calculate_inline_height();
                let inline_box_height = inline_box.value().dimensions.margin_box().height;
                maximum_inline_box_height = f32::max(maximum_inline_box_height, inline_box_height);
            }
            dbg!(maximum_inline_box_height);
            let mut line_node = self.append(LayoutBox::new(LineBox));
            line_node.value().dimensions.content.width = anonymous_block_width;
            line_node.value().dimensions.content.height = maximum_inline_box_height;

            // Append children
            for inline_box_id in line_inline_box_ids {
                line_node.append_id(inline_box_id);
            }
        }

        // TODO: write a separate function for anonymous block children layouting
        self.for_each_child(|child| {
            let parent_dimensions = child.parent().unwrap().value().dimensions;
            child.layout(parent_dimensions);

            let child_layouted_dimensions = child.value().dimensions;
            // SAFE: We already checked that parent is in there.
            let mut parent = unsafe { child.parent().unwrap_unchecked() };
            parent.value().dimensions.content.height +=
                child_layouted_dimensions.margin_box().height;
        });

        // Default height of the anonymous box is set to zero
        // incremently layout the line boxes similarly to block elements
        // width and height of the line boxes is already known
        // position should depend on the height of the anonymous box (add it to the end as in block layout)
        // children are simply layed out in line one after one

        // TODO: layout the line boxes

        // We know the width of the containing block (it is calculated prior to layouting the children)
        // the height of the anonymous block will be then used to calculate the total height of the containg block
        // TODO:
        // + decide LineBoxes' width based on the width of the containing box
        // + compute inline box width for each child (may depend on the children of a child and so on)
        // + put inline boxes in LineBoxes
        // + find the LineBox height from the height of the tallest inline box child
        // + layout the LineBox elements
        // + stack the LineBox vertically
        // + set the height for anonymous block box (sum of heights of all LineBoxes)
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

    fn calculate_inline_width(&mut self) {
        // TODO: make a separate function that calculates margins/paddings/borders??
        let style = self.value().get_style_node().unwrap();

        let auto = Keyword("auto".to_string());

        // margin, border, and padding have initial value 0.
        let zero = Length(0.0, Px);

        let mut margin_left = style.lookup("margin-left", "margin", &zero);
        let mut margin_right = style.lookup("margin-right", "margin", &zero);
        let border_left = style.lookup("border-left-width", "border-width", &zero);
        let border_right = style.lookup("border-right-width", "border-width", &zero);
        let padding_left = style.lookup("padding-left", "padding", &zero);
        let padding_right = style.lookup("padding-right", "padding", &zero);

        if margin_left == auto {
            margin_left = zero.clone();
        }
        if margin_right == auto {
            margin_right = zero.clone();
        }

        let mut accumulated_width = 0.0;

        dbg!(accumulated_width);
        self.for_each_child(|c| {
            // TODO: generalize the width calculation, because inline elements can also have TextBox as children
            c.calculate_inline_width();
            accumulated_width += c.value().dimensions.margin_box().width;
        });

        if accumulated_width == 0.0 {
            // TODO: change this default value to something else when text support is added
            accumulated_width = 100.0;
        }

        let d = &mut self.value().dimensions;
        d.padding.left = padding_left.to_px();
        d.padding.right = padding_right.to_px();

        d.border.left = border_left.to_px();
        d.border.right = border_right.to_px();

        d.margin.left = margin_left.to_px();
        d.margin.right = margin_right.to_px();

        d.content.width = accumulated_width;
    }

    fn calculate_inline_height(&mut self) {
        // TODO: should depend on text inside or replaced content
        let mut max_height = 0.0;
        self.for_each_child(|c| {
            // TODO: generalize the height calculation, because inline elements can also have TextBox as children
            c.calculate_inline_height();
            max_height = f32::max(max_height, c.value().dimensions.margin_box().height);
        });

        if max_height == 0.0 {
            // TODO: change this default value to something else when text support is added
            max_height = 50.0;
        }
        let d: &mut Dimensions = &mut self.value().dimensions;
        d.content.height = max_height;
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

    // TODO: think thorugh what inline container should be
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
                // TODO: possibly create an unnecessary anonymous block e.g. if all children are inline
                Err(mut slf) => {
                    slf.append(LayoutBox::new(AnonymousBlock));
                    slf.into_last_child().unwrap()
                }
            },
            // LineBox must have a parent
            LineBox => slf0.into_parent().unwrap(),
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
