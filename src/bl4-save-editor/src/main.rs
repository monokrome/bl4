#[cfg(feature = "server")]
mod commands;
#[cfg(feature = "server")]
mod state;

#[cfg(feature = "server")]
mod server;

#[cfg(feature = "servo")]
mod menu;
#[cfg(feature = "servo")]
mod resources;
#[cfg(feature = "servo")]
mod servo_webview;
#[cfg(feature = "servo")]
mod webview;

#[cfg(feature = "server")]
#[tokio::main]
async fn main() {
    server::run().await;
}

#[cfg(feature = "servo")]
fn main() -> anyhow::Result<()> {
    use menu::{AppMenu, MenuId};
    use servo_webview::{ServoWebView, Waker, WakerEvent};
    use webview::{BackendMessage, FrontendMessage, WebViewHost};
    use winit::{
        application::ApplicationHandler,
        event::WindowEvent,
        event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
        window::{Window, WindowId},
    };

    // Prefer native Wayland on Linux to avoid EGL issues with XWayland + NVIDIA
    #[cfg(target_os = "linux")]
    if std::env::var("WINIT_UNIX_BACKEND").is_err() {
        std::env::set_var("WINIT_UNIX_BACKEND", "wayland");
    }

    // Initialize TLS provider (required by Servo)
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .expect("Failed to install crypto provider");

    // Install embedded Servo resources
    resources::EmbeddedResourceReader::install();

    println!("Starting BL4 Save Editor...");

    let event_loop = EventLoop::<WakerEvent>::with_user_event()
        .build()
        .expect("Failed to create event loop");
    event_loop.set_control_flow(ControlFlow::Wait);

    struct App {
        window: Option<Window>,
        webview: ServoWebView,
        waker: Option<Waker>,
        menu: Option<AppMenu>,
        html_content: &'static str,
    }

    impl App {
        fn new(event_loop: &EventLoop<WakerEvent>) -> Self {
            Self {
                window: None,
                webview: ServoWebView::new(),
                waker: Some(Waker::new(event_loop)),
                menu: None,
                html_content: include_str!("../assets/index.html"),
            }
        }

        fn handle_menu_event(&mut self, id: MenuId) {
            match id {
                MenuId::Open => {
                    println!("Menu: Open file");
                    self.webview.send_message(BackendMessage::Status {
                        message: "Opening file dialog...".to_string(),
                    });
                }
                MenuId::Save => println!("Menu: Save"),
                MenuId::SaveAs => println!("Menu: Save As"),
                MenuId::Exit => std::process::exit(0),
                MenuId::Undo => println!("Menu: Undo"),
                MenuId::Redo => println!("Menu: Redo"),
                MenuId::Preferences => println!("Menu: Preferences"),
                MenuId::About => println!("Menu: About"),
            }
        }

        fn handle_frontend_message(&mut self, msg: FrontendMessage) {
            match msg {
                FrontendMessage::Ready => {
                    println!("Frontend ready");
                    self.webview.send_message(BackendMessage::Status {
                        message: "Backend connected".to_string(),
                    });
                }
                FrontendMessage::OpenFile { path } => println!("Open file: {:?}", path),
                FrontendMessage::SaveFile { path } => println!("Save file: {:?}", path),
                FrontendMessage::SetValue { path, value } => {
                    println!("Set: {} = {:?}", path, value)
                }
                FrontendMessage::GetSaveData => println!("Get save data"),
                FrontendMessage::GetManifest { kind } => println!("Get manifest: {}", kind),
            }
        }
    }

    impl ApplicationHandler<WakerEvent> for App {
        fn resumed(&mut self, event_loop: &ActiveEventLoop) {
            if self.window.is_none() {
                let window_attrs = Window::default_attributes()
                    .with_title("BL4 Save Editor")
                    .with_inner_size(winit::dpi::LogicalSize::new(1200, 800));

                match event_loop.create_window(window_attrs) {
                    Ok(window) => {
                        println!("Window created successfully");

                        let menu = AppMenu::new();
                        menu.init_for_window(&window);
                        self.menu = Some(menu);

                        if let Some(waker) = self.waker.take() {
                            if let Err(e) = self.webview.init_with_waker(&window, waker) {
                                eprintln!("Failed to init Servo: {}", e);
                                event_loop.exit();
                                return;
                            }
                            self.webview
                                .create_webview(self.html_content, window.scale_factor());
                        }

                        self.window = Some(window);
                    }
                    Err(e) => {
                        eprintln!("Failed to create window: {}", e);
                        event_loop.exit();
                    }
                }
            }
        }

        fn user_event(&mut self, _event_loop: &ActiveEventLoop, _event: WakerEvent) {
            self.webview.spin();
        }

        fn window_event(
            &mut self,
            event_loop: &ActiveEventLoop,
            _id: WindowId,
            event: WindowEvent,
        ) {
            self.webview.spin();

            match event {
                WindowEvent::CloseRequested => {
                    println!("Close requested");
                    event_loop.exit();
                }
                WindowEvent::Resized(size) => {
                    self.webview.resize(size.width, size.height);
                }
                WindowEvent::RedrawRequested => {
                    if let Some(menu) = &self.menu {
                        if let Some(id) = menu.poll_events() {
                            self.handle_menu_event(id);
                        }
                    }

                    for msg in self.webview.poll_messages() {
                        self.handle_frontend_message(msg);
                    }

                    self.webview.render();

                    if let Some(window) = &self.window {
                        window.request_redraw();
                    }
                }
                _ => {}
            }
        }
    }

    let mut app = App::new(&event_loop);
    event_loop.run_app(&mut app)?;

    Ok(())
}

#[cfg(not(any(feature = "server", feature = "servo")))]
fn main() {
    eprintln!("Error: Must enable 'server' or 'servo' feature");
    eprintln!("  cargo run -p bl4-save-editor --features servo");
    eprintln!("  cargo run -p bl4-save-editor --features server");
    std::process::exit(1);
}
