//! Servo-based WebView implementation
//!
//! Implements WebViewHost trait using the Servo browser engine.

use std::cell::RefCell;
use std::rc::Rc;

use euclid::Scale;
use servo::{
    RenderingContext, Servo, ServoBuilder, WebView, WebViewBuilder, WindowRenderingContext,
};
use url::Url;
use winit::event_loop::EventLoop;
use winit::raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use winit::window::Window;

use crate::webview::{BackendMessage, FrontendMessage, WebViewHost, JS_BRIDGE};

/// Event sent to wake the event loop
#[derive(Debug)]
pub struct WakerEvent;

/// Waker that signals the winit event loop
#[derive(Clone)]
pub struct Waker(winit::event_loop::EventLoopProxy<WakerEvent>);

impl Waker {
    pub fn new(event_loop: &EventLoop<WakerEvent>) -> Self {
        Self(event_loop.create_proxy())
    }
}

impl embedder_traits::EventLoopWaker for Waker {
    fn clone_box(&self) -> Box<dyn embedder_traits::EventLoopWaker> {
        Box::new(self.clone())
    }

    fn wake(&self) {
        let _ = self.0.send_event(WakerEvent);
    }
}

/// Servo-based WebView
pub struct ServoWebView {
    servo: Option<Servo>,
    rendering_context: Option<Rc<WindowRenderingContext>>,
    webview: Option<WebView>,
    pending_messages: Vec<FrontendMessage>,
    ready: bool,
}

impl ServoWebView {
    pub fn new() -> Self {
        Self {
            servo: None,
            rendering_context: None,
            webview: None,
            pending_messages: Vec::new(),
            ready: false,
        }
    }

    /// Initialize Servo with the given window and event loop waker
    pub fn init_with_waker(&mut self, window: &Window, waker: Waker) -> anyhow::Result<()> {
        let display_handle = window
            .display_handle()
            .map_err(|e| anyhow::anyhow!("Failed to get display handle: {}", e))?;
        let window_handle = window
            .window_handle()
            .map_err(|e| anyhow::anyhow!("Failed to get window handle: {}", e))?;

        // Ensure window has valid size
        let size = window.inner_size();
        if size.width == 0 || size.height == 0 {
            return Err(anyhow::anyhow!(
                "Window has zero size: {}x{}",
                size.width,
                size.height
            ));
        }

        println!(
            "Creating rendering context with size: {}x{}",
            size.width, size.height
        );

        let rendering_context = Rc::new(
            WindowRenderingContext::new(display_handle, window_handle, size)
                .map_err(|e| anyhow::anyhow!("Failed to create rendering context: {:?}", e))?,
        );

        let _ = rendering_context.make_current();

        let servo = ServoBuilder::default()
            .event_loop_waker(Box::new(waker))
            .build();
        servo.setup_logging();

        self.servo = Some(servo);
        self.rendering_context = Some(rendering_context);
        self.ready = true;

        Ok(())
    }

    /// Create a webview and load content
    pub fn create_webview(&mut self, html: &str, scale_factor: f64) {
        let servo = match &self.servo {
            Some(s) => s,
            None => return,
        };
        let rendering_context = match &self.rendering_context {
            Some(rc) => rc.clone(),
            None => return,
        };

        // Create a data URL from the HTML
        let html_with_bridge = format!("{}<script>{}</script>", html, JS_BRIDGE);
        let data_url = format!(
            "data:text/html;charset=utf-8,{}",
            urlencoding::encode(&html_with_bridge)
        );
        let url = Url::parse(&data_url).unwrap_or_else(|_| Url::parse("about:blank").unwrap());

        let webview = WebViewBuilder::new(servo, rendering_context)
            .url(url)
            .hidpi_scale_factor(Scale::new(scale_factor as f32))
            .build();

        self.webview = Some(webview);
    }

    /// Spin the Servo event loop
    pub fn spin(&self) {
        if let Some(servo) = &self.servo {
            servo.spin_event_loop();
        }
    }

    /// Paint and present
    pub fn paint(&self) {
        if let Some(webview) = &self.webview {
            webview.paint();
        }
        if let Some(rc) = &self.rendering_context {
            rc.present();
        }
    }
}

impl Default for ServoWebView {
    fn default() -> Self {
        Self::new()
    }
}

impl WebViewHost for ServoWebView {
    fn init(&mut self, _window: &Window) -> anyhow::Result<()> {
        // Note: Full init requires the event loop waker, which is done via init_with_waker
        // This is called from the App, which doesn't have access to the event loop
        // So we defer actual initialization
        Ok(())
    }

    fn render(&mut self) {
        self.spin();
        self.paint();
    }

    fn resize(&mut self, width: u32, height: u32) {
        if let Some(webview) = &self.webview {
            webview.resize(winit::dpi::PhysicalSize::new(width, height));
        }
    }

    fn load_url(&mut self, url: &str) {
        if let Some(webview) = &self.webview {
            if let Ok(parsed) = Url::parse(url) {
                webview.load(parsed);
            }
        }
    }

    fn load_html(&mut self, html: &str) {
        // Store HTML to load when webview is created
        // Actual loading happens in create_webview
        let _ = html; // Will be used when we have proper init flow
    }

    fn evaluate_script(&mut self, _script: &str) {
        // TODO: Servo script evaluation
        // This requires accessing the script thread
    }

    fn poll_messages(&mut self) -> Vec<FrontendMessage> {
        std::mem::take(&mut self.pending_messages)
    }

    fn is_ready(&self) -> bool {
        self.ready
    }
}
