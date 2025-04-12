use crate::painting::PixelBuffer;

use std::fs::{self, File};
use std::io::Read;
use std::num::NonZeroU32;
use std::sync::{mpsc, Arc, Mutex};
use std::time::UNIX_EPOCH;

use tiny_skia::Pixmap;

use tracing::*;
use tracing_subscriber::FmtSubscriber;

use winit::event::{Event, KeyEvent, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::Window;

pub mod css;
pub mod dom;
pub mod html;
pub mod layout;
pub mod painting;
pub mod style;
pub mod winit_app;

const HTML_FILE_PATH: &str = "test.html";
const CSS_FILE_PATH: &str = "test.css";

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

pub fn read_source(filename: &str) -> String {
    let mut s = String::new();
    File::open(filename)
        .unwrap()
        .read_to_string(&mut s)
        .unwrap();
    s
}

pub fn file_modified_time_in_seconds(path: &str) -> u64 {
    fs::metadata(path)
        .unwrap()
        .modified()
        .unwrap()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

type Surface = softbuffer::Surface<Arc<Window>, Arc<Window>>;

fn render_thread(
    window: Arc<Window>,
    do_render: mpsc::Receiver<Arc<Mutex<Surface>>>,
    done: mpsc::Sender<()>,
) {
    loop {
        debug!("waiting for render...");
        let Ok(surface) = do_render.recv() else {
            info!("main thread destroyed");
            break;
        };

        // Perform the rendering.
        let mut surface = surface.lock().unwrap();
        if let (Some(width), Some(height)) = {
            let size = window.inner_size();
            debug!("got size: {size:?}");
            (NonZeroU32::new(size.width), NonZeroU32::new(size.height))
        } {
            debug!("resizing...");
            surface.resize(width, height).unwrap();

            let mut buffer = surface.buffer_mut().unwrap();

            if let Some(mut pixmap) = Pixmap::new(width.get(), height.get()) {
                pixmap.fill(tiny_skia::Color::WHITE);

                let html = read_source(HTML_FILE_PATH);
                let css = read_source(CSS_FILE_PATH);

                let root_node = html::parse(html.clone());
                let stylesheet = css::parse(css.clone());
                let style_root = style::style_tree(&root_node, &stylesheet);

                // Update internal state and request a redraw
                let layout_root = layout::layout_tree(
                    &style_root,
                    layout::Dimensions {
                        content: layout::Rect {
                            width: width.get() as f32,
                            height: height.get() as f32,
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                );
                pixmap.paint(&layout_root);

                // Copy tiny_skia pixmap to the window buffer
                let bytes = pixmap.data();

                assert!(
                    bytes.len() % 4 == 0,
                    "The length of the slice must be divisible by 4"
                );

                for (i, pix) in bytes.chunks(4).enumerate() {
                    // Since we already checked that lenght is
                    // divisible by 4 we can safely assume
                    // that each chunk will be slice of 4
                    // elements.
                    let pix_assembled = u32::from_be_bytes(
                        <[u8; 4]>::try_from([0, pix[0], pix[1], pix[2]])
                            .expect("Ups, I did it again..."),
                    );
                    // Otherwise on window resize we will get panic, since actual window buffer will change, but pixmap won't.
                    if i < buffer.len() {
                        buffer[i] = pix_assembled;
                    }
                }
            } else {
                // FIXME: Should actually not occur due to
                // check in the outer if let (width and height
                // are nonzero).
                warn!("Cannot create pixel map");
            }

            debug!("presenting...");
            buffer.present().unwrap();
        }

        // We're done, tell the main thread to keep going.
        done.send(()).ok();
    }
}

pub fn entry(event_loop: EventLoop<()>) {
    let mut html_last_changed = dbg!(file_modified_time_in_seconds(HTML_FILE_PATH));
    let mut css_last_changed = dbg!(file_modified_time_in_seconds(CSS_FILE_PATH));

    let app = winit_app::WinitAppBuilder::with_init(
        |elwt| {
            let attributes = Window::default_attributes();
            let window = Arc::new(elwt.create_window(attributes).unwrap());

            let context = softbuffer::Context::new(window.clone()).unwrap();

            // Spawn a thread to handle rendering for this specific surface. The channels will
            // be closed and the thread will be stopped whenever this surface (the returned
            // context below) is dropped, so that it can all be recreated again (on Android)
            // when a new surface is created.
            let (start_render, do_render) = mpsc::channel();
            let (render_done, finish_render) = mpsc::channel();
            debug!("starting thread...");
            std::thread::spawn({
                let window = window.clone();
                move || render_thread(window, do_render, render_done)
            });

            (window, context, start_render, finish_render)
        },
        |_elwt, (window, context, _start_render, _finish_render)| {
            debug!("making surface...");
            Arc::new(Mutex::new(
                softbuffer::Surface::new(context, window.clone()).unwrap(),
            ))
        },
    )
    .with_event_handler(move |state, surface, event, elwt| {
        let (window, _context, start_render, finish_render) = state;
        elwt.set_control_flow(ControlFlow::Wait);

        // Reload files if they changed on drive.
        let html_last_changed_now = file_modified_time_in_seconds(HTML_FILE_PATH);
        let css_last_changed_now = file_modified_time_in_seconds(CSS_FILE_PATH);

        if html_last_changed != html_last_changed_now || css_last_changed != css_last_changed_now {
            html_last_changed = html_last_changed_now;
            css_last_changed = css_last_changed_now;
            window.request_redraw();
        }

        match event {
            Event::WindowEvent {
                window_id,
                event: WindowEvent::RedrawRequested,
            } if window_id == window.id() => {
                debug!("Redraw requested");
                let Some(surface) = surface else {
                    error!("RedrawRequested fired before Resumed or after Suspended");
                    return;
                };
                // Start the render and then finish it.
                start_render.send(surface.clone()).unwrap();
                finish_render.recv().unwrap();
            }
            Event::WindowEvent {
                event:
                    WindowEvent::CloseRequested
                    | WindowEvent::KeyboardInput {
                        event:
                            KeyEvent {
                                logical_key: Key::Named(NamedKey::Escape),
                                ..
                            },
                        ..
                    },
                window_id,
            } if window_id == window.id() => {
                elwt.exit();
            }
            _ => {}
        }
    });

    winit_app::run_app(event_loop, app);
}

fn main() {
    use winit::event_loop::EventLoop;
    initialize_logging();
    entry(EventLoop::new().unwrap())
}
