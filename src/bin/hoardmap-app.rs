//! The native Mac app: the same server and page as the `hoardmap` command, shown in
//! a window instead of a browser tab. Closing the window quits the app.

use std::path::PathBuf;

use hoardmap::server::{self, Config};
use muda::{AboutMetadata, Menu, PredefinedMenuItem, Submenu};
use tao::dpi::LogicalSize;
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoop};
use tao::window::WindowBuilder;
use wry::WebViewBuilder;

/// Tried first, so the page keeps the same origin, and with it its saved view and
/// settings, from one launch to the next.
const PREFERRED_PORT: u16 = 48273;

fn main() {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| "/".into());
    let port = match server::spawn(Config {
        path: home,
        port: PREFERRED_PORT,
        linger: None,
        native: true,
        // Without Full Disk Access the page starts the scan when its start screen closes,
        // so a grant made from that screen applies to the first scan.
        scan_at_start: hoardmap::access::granted(),
    }) {
        Ok(port) => port,
        Err(error) => {
            eprintln!("hoardmap: {error}");
            std::process::exit(1);
        }
    };
    let origin = format!("http://127.0.0.1:{port}");

    let event_loop = EventLoop::new();
    let menu = menu_bar();
    menu.init_for_nsapp();

    let window = WindowBuilder::new()
        .with_title("Hoardmap")
        .with_inner_size(LogicalSize::new(1400.0, 900.0))
        .with_min_inner_size(LogicalSize::new(720.0, 480.0))
        .build(&event_loop)
        .expect("create the window");

    let allowed = origin.clone();
    let _webview = WebViewBuilder::new()
        .with_url(format!("{origin}/"))
        // The page is the only thing this window shows; links elsewhere stay out.
        .with_navigation_handler(move |url| url.starts_with(&allowed))
        .with_new_window_req_handler(|_, _| wry::NewWindowResponse::Deny)
        .build(&window)
        .expect("create the web view");

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        if let Event::WindowEvent {
            event: WindowEvent::CloseRequested,
            ..
        } = event
        {
            *control_flow = ControlFlow::Exit;
        }
    });
}

/// The standard app, Edit and Window menus; Edit is what makes copy and paste work in
/// the path field.
fn menu_bar() -> Menu {
    let about = AboutMetadata {
        name: Some("Hoardmap".into()),
        version: Some(env!("CARGO_PKG_VERSION").into()),
        ..Default::default()
    };
    let app = Submenu::with_items(
        "Hoardmap",
        true,
        &[
            &PredefinedMenuItem::about(None, Some(about)),
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::hide(None),
            &PredefinedMenuItem::hide_others(None),
            &PredefinedMenuItem::show_all(None),
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::quit(None),
        ],
    )
    .expect("app menu");
    let edit = Submenu::with_items(
        "Edit",
        true,
        &[
            &PredefinedMenuItem::undo(None),
            &PredefinedMenuItem::redo(None),
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::cut(None),
            &PredefinedMenuItem::copy(None),
            &PredefinedMenuItem::paste(None),
            &PredefinedMenuItem::select_all(None),
        ],
    )
    .expect("edit menu");
    let window = Submenu::with_items(
        "Window",
        true,
        &[
            &PredefinedMenuItem::minimize(None),
            &PredefinedMenuItem::fullscreen(None),
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::close_window(None),
        ],
    )
    .expect("window menu");
    Menu::with_items(&[&app, &edit, &window]).expect("menu bar")
}
