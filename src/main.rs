use crate::painting::PixelBuffer;

use std::fs::File;
use std::io::Read;
use std::time::Instant;

use pixels::{Error, Pixels, SurfaceTexture};
use tiny_skia::Pixmap;
use tracing::*;
use tracing_subscriber::FmtSubscriber;
use winit::dpi::LogicalSize;
use winit::event::{Event, WindowEvent};
use winit::event_loop::EventLoop;
use winit::keyboard::KeyCode;
use winit::window::WindowBuilder;
use winit_input_helper::WinitInputHelper;

pub mod css;
pub mod dom;
pub mod html;
pub mod layout;
pub mod painting;
pub mod style;

const WIDTH: u32 = 500;
const HEIGHT: u32 = 500;

pub fn initialize_logging() {
    // a builder for `FmtSubscriber`.
    let subscriber = FmtSubscriber::builder()
        // all spans/events with a level higher than TRACE (e.g, debug, info, warn, etc.)
        // will be written to stdout.
        .with_max_level(Level::DEBUG)
        // completes the builder.
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");
}

pub fn read_source(filename: String) -> String {
    let mut s = String::new();
    File::open(filename)
        .unwrap()
        .read_to_string(&mut s)
        .unwrap();
    s
}

fn main() -> Result<(), Error> {
    initialize_logging();

    let html = read_source("test.html".into());
    let css = read_source("test.css".into());

    let root_node = html::parse(html);
    let stylesheet = css::parse(css);
    let style_root = style::style_tree(&root_node, &stylesheet);

    let event_loop = EventLoop::new().unwrap();
    let mut input = WinitInputHelper::new();
    let window = {
        let size = LogicalSize::new(WIDTH as f64, HEIGHT as f64);
        WindowBuilder::new()
            .with_title("Hello tiny-skia")
            .with_inner_size(size)
            .with_min_inner_size(size)
            .build(&event_loop)
            .unwrap()
    };

    let mut pixels = {
        let window_size = window.inner_size();
        let surface_texture = SurfaceTexture::new(window_size.width, window_size.height, &window);

        Pixels::new(WIDTH, HEIGHT, surface_texture)?
    };

    let mut drawing = Pixmap::new(WIDTH, HEIGHT).unwrap();
    let _now = Instant::now();

    let res = event_loop.run(|event, elwt| {
        // Draw the current frame
        if let Event::WindowEvent {
            event: WindowEvent::RedrawRequested,
            ..
        } = event
        {
            pixels.frame_mut().copy_from_slice(drawing.data());
            if let Err(err) = pixels.render() {
                log_error("pixels.render", err);
                elwt.exit();
                return;
            }
        }

        // Handle input events
        if input.update(&event) {
            // Close events
            if input.key_pressed(KeyCode::Escape) || input.close_requested() {
                elwt.exit();
                return;
            }

            // Resize the window
            if let Some(size) = input.window_resized() {
                if let Err(err) = pixels.resize_surface(size.width, size.height) {
                    log_error("pixels.resize_surface", err);
                    elwt.exit();
                    return;
                }
            }

            // Update internal state and request a redraw
            let layout_root = layout::layout_tree(
                &style_root,
                layout::Dimensions {
                    // TODO: Update when actual width and height updates.
                    content: layout::Rect {
                        width: WIDTH as f32,
                        height: HEIGHT as f32,
                        ..Default::default()
                    },
                    ..Default::default()
                },
            );
            drawing.paint(&layout_root);
            window.request_redraw();
        }
    });
    res.map_err(|e| Error::UserDefined(Box::new(e)))
}

fn log_error<E: std::error::Error + 'static>(method_name: &str, err: E) {
    error!("{method_name}() failed: {err}");
    // for source in err.sources().skip(1) {
    //     error!("  Caused by: {source}");
    // }
}
