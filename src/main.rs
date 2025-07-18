use std::path::PathBuf;

use clap::Parser;

use server::AppServer;

mod app;
mod handler;
mod loader;
mod server;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    /// Port on which the server runs.
    #[arg(short, long, default_value_t = 22)]
    port: u16,

    /// Location of the server's private key.
    #[arg(long, default_value = "private_key")]
    key_path: PathBuf,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let mut server = AppServer::new(cli.port, cli.key_path);
    server.run().await.expect("Failed running server")
}
