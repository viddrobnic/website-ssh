use server::AppServer;

mod app;
mod handler;
mod loader;
mod server;

#[tokio::main]
async fn main() {
    let mut server = AppServer::new();
    server.run().await.expect("Failed running server")
}
