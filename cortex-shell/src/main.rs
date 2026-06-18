/// TPT Cortex Shell — native desktop host for the Cortex PWA.
///
/// Opens a desktop window containing a WebView that loads the Cortex Svelte
/// PWA. WebSocket traffic to the Go daemon (`ws://127.0.0.1:9911`) is handled
/// directly by the WebView; no proxy is needed since it is loopback.
///
/// URL resolution order:
///   1. `CORTEX_DEV_URL` env var (e.g. `http://127.0.0.1:5173` for Vite dev)
///   2. `cortex-pwa/dist/index.html` relative to the executable directory
///   3. Built-in placeholder HTML instructing the user to build the PWA
use wry::{
    application::{
        dpi::LogicalSize,
        event::{Event, WindowEvent},
        event_loop::{ControlFlow, EventLoop},
        window::WindowBuilder,
    },
    WebViewBuilder,
};

const PLACEHOLDER_HTML: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>TPT Cortex</title>
  <style>
    * { box-sizing: border-box; margin: 0; padding: 0; }
    body {
      font-family: system-ui, -apple-system, sans-serif;
      display: flex;
      flex-direction: column;
      align-items: center;
      justify-content: center;
      height: 100vh;
      background: #0f172a;
      color: #e2e8f0;
      gap: 1rem;
    }
    h1 { font-size: 1.75rem; font-weight: 700; }
    p  { color: #94a3b8; font-size: 0.95rem; text-align: center; max-width: 480px; }
    code {
      background: #1e293b;
      padding: 0.15em 0.4em;
      border-radius: 4px;
      font-family: monospace;
    }
  </style>
</head>
<body>
  <h1>TPT Cortex Shell</h1>
  <p>PWA bundle not found. Build it with:</p>
  <p><code>cd cortex-pwa &amp;&amp; npm run build</code></p>
  <p>Or set <code>CORTEX_DEV_URL=http://127.0.0.1:5173</code> to load from the Vite dev server.</p>
</body>
</html>"#;

fn main() -> wry::Result<()> {
    let event_loop = EventLoop::new();

    let window = WindowBuilder::new()
        .with_title("TPT Cortex")
        .with_inner_size(LogicalSize::new(1_024u32, 768u32))
        .with_min_inner_size(LogicalSize::new(480u32, 320u32))
        .build(&event_loop)?;

    let webview = build_webview(window)?;

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => *control_flow = ControlFlow::Exit,

            // Retain the webview for the lifetime of the event loop.
            Event::MainEventsCleared => {
                let _ = &webview;
            }
            _ => {}
        }
    });
}

fn build_webview(window: wry::application::window::Window) -> wry::Result<wry::WebView> {
    // 1. Developer override: load from a running Vite server.
    if let Ok(url) = std::env::var("CORTEX_DEV_URL") {
        eprintln!("[cortex-shell] dev mode: loading {url}");
        return WebViewBuilder::new(window)?
            .with_url(&url)?
            .with_devtools(true)
            .build();
    }

    // 2. Production: load the built PWA bundle from disk (next to the executable).
    if let Some(dist_path) = find_pwa_dist() {
        let url = format!("file://{}", dist_path.display());
        eprintln!("[cortex-shell] loading PWA from {url}");
        return WebViewBuilder::new(window)?
            .with_url(&url)?
            .build();
    }

    // 3. Fallback: embedded placeholder.
    eprintln!("[cortex-shell] no PWA bundle found; showing placeholder");
    WebViewBuilder::new(window)?
        .with_html(PLACEHOLDER_HTML.to_string())?
        .build()
}

/// Look for `cortex-pwa/dist/index.html` relative to the executable.
fn find_pwa_dist() -> Option<std::path::PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let exe_dir = exe.parent()?;

    // When running from the Cargo target directory the project root is three
    // levels up (target/debug/ or target/release/).
    let candidates = [
        exe_dir.join("cortex-pwa/dist/index.html"),
        exe_dir.join("../../cortex-pwa/dist/index.html"),
        exe_dir.join("../../../cortex-pwa/dist/index.html"),
    ];

    candidates
        .into_iter()
        .find(|p| p.exists())
        .map(|p| p.canonicalize().unwrap_or(p))
}
