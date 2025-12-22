//! WebView abstraction for bl4-gui
//!
//! Provides a backend-agnostic interface for webview communication.
//! Currently targets Servo, but designed to be swappable.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::mpsc::{self, Receiver, Sender};

/// Messages from the frontend (JavaScript) to the backend (Rust)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum FrontendMessage {
    /// Request to open a save file
    OpenFile { path: Option<String> },
    /// Request to save the current file
    SaveFile { path: Option<String> },
    /// Request to modify a save value
    SetValue {
        path: String,
        value: serde_json::Value,
    },
    /// Request current save data
    GetSaveData,
    /// Request manifest data (categories, parts, etc.)
    GetManifest { kind: String },
    /// Frontend is ready to receive data
    Ready,
}

/// Messages from the backend (Rust) to the frontend (JavaScript)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum BackendMessage {
    /// Save data loaded successfully
    SaveLoaded {
        filename: String,
        data: serde_json::Value,
    },
    /// Save completed
    SaveComplete { filename: String },
    /// Error occurred
    Error { message: String },
    /// Manifest data response
    ManifestData {
        kind: String,
        data: serde_json::Value,
    },
    /// Status update
    Status { message: String },
}

/// Trait for webview backends
///
/// This abstraction allows swapping out the webview implementation
/// (Servo, wry, webview2, etc.) without changing app logic.
pub trait WebViewHost {
    /// Initialize the webview with the given window
    fn init(&mut self, window: &winit::window::Window) -> anyhow::Result<()>;

    /// Render a frame
    fn render(&mut self);

    /// Resize the webview
    fn resize(&mut self, width: u32, height: u32);

    /// Navigate to a URL or load HTML
    fn load_url(&mut self, url: &str);

    /// Load HTML content directly
    fn load_html(&mut self, html: &str);

    /// Evaluate JavaScript in the webview
    fn evaluate_script(&mut self, script: &str);

    /// Send a message to the frontend
    fn send_message(&mut self, msg: BackendMessage) {
        let json = serde_json::to_string(&msg).unwrap_or_default();
        self.evaluate_script(&format!("window.__bl4_receive({})", json));
    }

    /// Poll for messages from the frontend
    fn poll_messages(&mut self) -> Vec<FrontendMessage>;

    /// Check if the webview is ready
    fn is_ready(&self) -> bool;
}

/// Message bridge for async communication
pub struct MessageBridge {
    /// Channel for frontend → backend messages
    pub from_frontend: Receiver<FrontendMessage>,
    /// Sender that gets passed to the webview/JS context
    frontend_sender: Sender<FrontendMessage>,
    /// Pending messages to send to frontend
    to_frontend: Vec<BackendMessage>,
}

impl MessageBridge {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            from_frontend: rx,
            frontend_sender: tx,
            to_frontend: Vec::new(),
        }
    }

    /// Get a sender for the frontend to use
    pub fn frontend_sender(&self) -> Sender<FrontendMessage> {
        self.frontend_sender.clone()
    }

    /// Queue a message to send to frontend
    pub fn send_to_frontend(&mut self, msg: BackendMessage) {
        self.to_frontend.push(msg);
    }

    /// Take pending messages for the frontend
    pub fn take_frontend_messages(&mut self) -> Vec<BackendMessage> {
        std::mem::take(&mut self.to_frontend)
    }

    /// Poll for messages from frontend
    pub fn poll(&self) -> Vec<FrontendMessage> {
        let mut messages = Vec::new();
        while let Ok(msg) = self.from_frontend.try_recv() {
            messages.push(msg);
        }
        messages
    }
}

impl Default for MessageBridge {
    fn default() -> Self {
        Self::new()
    }
}

/// Placeholder webview that just logs - for initial testing without Servo
pub struct PlaceholderWebView {
    ready: bool,
    pending_messages: Vec<FrontendMessage>,
}

impl PlaceholderWebView {
    pub fn new() -> Self {
        Self {
            ready: false,
            pending_messages: Vec::new(),
        }
    }
}

impl Default for PlaceholderWebView {
    fn default() -> Self {
        Self::new()
    }
}

impl WebViewHost for PlaceholderWebView {
    fn init(&mut self, _window: &winit::window::Window) -> anyhow::Result<()> {
        println!("[PlaceholderWebView] Initialized");
        self.ready = true;
        // Simulate frontend ready message
        self.pending_messages.push(FrontendMessage::Ready);
        Ok(())
    }

    fn render(&mut self) {
        // No-op for placeholder
    }

    fn resize(&mut self, width: u32, height: u32) {
        println!("[PlaceholderWebView] Resize to {}x{}", width, height);
    }

    fn load_url(&mut self, url: &str) {
        println!("[PlaceholderWebView] Load URL: {}", url);
    }

    fn load_html(&mut self, html: &str) {
        println!("[PlaceholderWebView] Load HTML: {} bytes", html.len());
    }

    fn evaluate_script(&mut self, script: &str) {
        println!(
            "[PlaceholderWebView] Evaluate: {}...",
            &script[..script.len().min(50)]
        );
    }

    fn poll_messages(&mut self) -> Vec<FrontendMessage> {
        std::mem::take(&mut self.pending_messages)
    }

    fn is_ready(&self) -> bool {
        self.ready
    }
}

/// IPC endpoint - Servo intercepts requests to this URL
pub const IPC_ENDPOINT: &str = "http://bl4.localhost/api";

// JavaScript bridge code to inject into the webview
pub const JS_BRIDGE: &str = r#"
// BL4 WebView Bridge
window.__bl4_callbacks = {};
window.__bl4_callbackId = 0;

// Receive message from Rust backend
window.__bl4_receive = function(msg) {
    console.log('[BL4] Received:', msg);
    window.dispatchEvent(new CustomEvent('bl4:message', { detail: msg }));
};

// API call helper - intercepted by Servo's load_web_resource
async function bl4Call(endpoint, body) {
    const response = await fetch('http://bl4.localhost/api' + endpoint, {
        method: body ? 'POST' : 'GET',
        headers: { 'Content-Type': 'application/json' },
        body: body ? JSON.stringify(body) : undefined
    });
    const json = await response.json();
    if (!json.success) throw new Error(json.error || 'Unknown error');
    return json.data;
}

// Send message to Rust backend
window.bl4 = {
    // Legacy send method
    send: function(type, payload) {
        const msg = JSON.stringify({ type, payload });
        fetch('http://bl4.localhost/api', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: msg
        }).then(r => r.json())
          .then(response => {
              if (response) window.__bl4_receive(response);
          })
          .catch(err => console.error('[BL4] Send error:', err));
    },

    // Discover saves from default locations
    discoverSaves: function() {
        bl4Call('/discover').then(data => {
            window.__bl4_receive({ type: 'SavesDiscovered', data });
        }).catch(err => {
            window.__bl4_receive({ type: 'SavesDiscovered', data: { profiles: [] } });
        });
    },

    // Open a save file
    openSave: function(path, steamId) {
        bl4Call('/save/open', { path, steam_id: steamId }).then(data => {
            window.__bl4_receive({ type: 'SaveOpened', data });
        }).catch(err => {
            window.__bl4_receive({ type: 'Error', data: { message: err.message } });
        });
    },

    // Save changes
    saveChanges: function() {
        bl4Call('/save', {}).then(() => {
            window.__bl4_receive({ type: 'Saved', data: {} });
        }).catch(err => {
            window.__bl4_receive({ type: 'Error', data: { message: err.message } });
        });
    },

    onMessage: function(callback) {
        window.addEventListener('bl4:message', (e) => callback(e.detail));
    }
};

// Signal ready
window.bl4.send('Ready', null);
"#;
