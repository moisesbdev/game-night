//! The native window: the system's own web view (WebView2 on Windows, WebKitGTK on Linux, WKWebView on macOS) showing the
//! local page. Nothing but Game Night loads in it; links to other sites open in your usual browser.

use tao::dpi::LogicalSize;
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoop};
use tao::window::{Icon, WindowBuilder};

/// The little moon, drawn at run time (no image files to ship): a sea-glass disc with a dark rim.
fn icon() -> Option<Icon> {
    const N: u32 = 64;
    let mut px = Vec::with_capacity((N * N * 4) as usize);
    for y in 0..N {
        for x in 0..N {
            let (dx, dy) = (x as f32 - 31.5, y as f32 - 33.5);
            let d = (dx * dx + dy * dy).sqrt();
            let (r, g, b, a) = if d < 25.0 {
                (0x9f, 0xe3, 0xdc, 255)
            } else if d < 28.5 {
                (0x12, 0x34, 0x3b, 255)
            } else {
                (0, 0, 0, 0)
            };
            px.extend_from_slice(&[r, g, b, a]);
        }
    }
    Icon::from_rgba(px, N, N).ok()
}

/// The program's own icon (embedded at build time on Windows), else the drawn disc.
fn app_icon() -> Option<Icon> {
    #[cfg(windows)]
    {
        use tao::platform::windows::IconExtWindows;
        if let Ok(i) = Icon::from_resource(1, None) {
            return Some(i);
        }
    }
    icon()
}

pub fn run(url: &str) -> Result<(), String> {
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("Game Night")
        .with_inner_size(LogicalSize::new(1280.0, 860.0))
        .with_min_inner_size(LogicalSize::new(720.0, 540.0))
        .with_window_icon(app_icon())
        .build(&event_loop)
        .map_err(|e| e.to_string())?;
    let origin = url.trim_end_matches('/').to_string();
    let home = origin.clone();
    let builder = wry::WebViewBuilder::new()
        .with_url(url)
        .with_background_color((13, 31, 35, 255))
        .with_devtools(cfg!(debug_assertions))
        .with_navigation_handler(move |u: String| {
            let ours = u.starts_with(&home) || u == "about:blank";
            if !ours && (u.starts_with("https://") || u.starts_with("steam://")) {
                let _ = crate::launch::open_url(&u); // a link to elsewhere opens in the usual browser, never inside the window
            }
            ours
        })
        .with_new_window_req_handler(move |u: String, _| {
            if u.starts_with("https://") {
                let _ = crate::launch::open_url(&u);
            }
            wry::NewWindowResponse::Deny
        });
    #[cfg(any(target_os = "linux", target_os = "dragonfly", target_os = "freebsd", target_os = "netbsd", target_os = "openbsd"))]
    let webview = {
        use tao::platform::unix::WindowExtUnix;
        use wry::WebViewBuilderExtUnix;
        builder.build_gtk(window.default_vbox().ok_or("no GTK box")?).map_err(|e| e.to_string())?
    };
    #[cfg(not(any(target_os = "linux", target_os = "dragonfly", target_os = "freebsd", target_os = "netbsd", target_os = "openbsd")))]
    let webview = builder.build(&window).map_err(|e| e.to_string())?;
    let _keep = (webview, origin);
    event_loop.run(move |event, _, flow| {
        *flow = ControlFlow::Wait;
        if let Event::WindowEvent { event: WindowEvent::CloseRequested, .. } = event {
            *flow = ControlFlow::Exit;
        }
    })
}
