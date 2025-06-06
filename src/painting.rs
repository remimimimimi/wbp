// This type replaces Canvas from the original article.
use crate::{
    css::{
        props::{Background, BorderColor, PropUnion, Property},
        values::Color,
    },
    layout::{AnonymousBlock, BlockNode, InlineNode, LayoutBox, Rect},
};

use ego_tree::*;
use tiny_skia::Pixmap;

#[derive(Debug)]
pub enum DisplayCommand {
    SolidColor(Color, Rect),
}

pub type DisplayList = Vec<DisplayCommand>;

pub fn build_display_list(layout_root: NodeRef<'_, LayoutBox>) -> DisplayList {
    let mut list = Vec::new();
    render_layout_box(&mut list, layout_root);
    list
}

fn render_layout_box(list: &mut DisplayList, layout_box: NodeRef<'_, LayoutBox>) {
    render_background(list, layout_box);
    render_borders(list, layout_box);
    for child in layout_box.children() {
        render_layout_box(list, child);
    }
}

fn render_background(list: &mut DisplayList, layout_box: NodeRef<'_, LayoutBox>) {
    // todo!()
    if let Some(color) = get_color::<Background>(layout_box) {
        list.push(DisplayCommand::SolidColor(
            color,
            layout_box.value().dimensions.border_box(),
        ));
    }
}

fn render_borders(list: &mut DisplayList, layout_box: NodeRef<LayoutBox>) {
    let color = match get_color::<BorderColor>(layout_box) {
        Some(color) => color,
        _ => return,
    };

    let d = &layout_box.value().dimensions;
    let border_box = d.border_box();

    // Left border
    list.push(DisplayCommand::SolidColor(
        color,
        Rect {
            x: border_box.x,
            y: border_box.y,
            width: d.border.left,
            height: border_box.height,
        },
    ));

    // Right border
    list.push(DisplayCommand::SolidColor(
        color,
        Rect {
            x: border_box.x + border_box.width - d.border.right,
            y: border_box.y,
            width: d.border.right,
            height: border_box.height,
        },
    ));

    // Top border
    list.push(DisplayCommand::SolidColor(
        color,
        Rect {
            x: border_box.x,
            y: border_box.y,
            width: border_box.width,
            height: d.border.top,
        },
    ));

    // Bottom border
    list.push(DisplayCommand::SolidColor(
        color,
        Rect {
            x: border_box.x,
            y: border_box.y + border_box.height - d.border.bottom,
            width: border_box.width,
            height: d.border.bottom,
        },
    ));
}

/// Return the specified color for CSS property `name`, or None if no color was specified.
fn get_color<T: Property + Clone + Into<Color>>(layout_box: NodeRef<LayoutBox>) -> Option<Color>
where
    for<'a> &'a T: From<&'a PropUnion>,
{
    match &layout_box.value().box_type {
        BlockNode(style) | InlineNode(style) => style.value::<T>().map(|v| v.into()),
        AnonymousBlock => None,
    }
}

/// Represents backend for painting.
pub trait PixelBuffer: Sized {
    /// Paint one item on pixel buffer.
    fn paint_item(&mut self, item: &DisplayCommand);

    /// Paint a tree of LayoutBoxes to an array of pixels.
    fn paint(&mut self, layout_root: NodeRef<'_, LayoutBox>) {
        let display_list = build_display_list(layout_root);
        for item in display_list {
            self.paint_item(&item);
        }
    }
}

impl PixelBuffer for Pixmap {
    fn paint_item(&mut self, item: &DisplayCommand) {
        match item {
            DisplayCommand::SolidColor(color, rect) => {
                let mut paint = tiny_skia::Paint::default();
                paint.set_color_rgba8(color.0, color.1, color.2, 255);
                self.fill_rect(
                    tiny_skia::Rect::from_xywh(rect.x, rect.y, rect.width, rect.height).unwrap(),
                    &paint,
                    tiny_skia::Transform::identity(),
                    None,
                );
            }
        }
    }
}
