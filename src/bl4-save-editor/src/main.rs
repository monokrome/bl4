#[cfg(feature = "server")]
mod commands;
#[cfg(feature = "server")]
mod state;

#[cfg(feature = "server")]
mod server;

#[cfg(feature = "server")]
#[tokio::main]
async fn main() {
    server::run().await;
}

#[cfg(not(feature = "server"))]
fn main() {
    eprintln!("Error: Must enable 'server' feature");
    eprintln!("  cargo run -p bl4-save-editor --features server");
    std::process::exit(1);
}
