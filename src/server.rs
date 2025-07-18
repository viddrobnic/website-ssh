use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use ratatui::{Terminal, prelude::CrosstermBackend};
use russh::keys::{PrivateKey, ssh_key};
use russh::*;
use russh::{keys::ssh_key::rand_core::OsRng, server::*};
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};

use crate::handler::Handler;

pub type SshTerminal = Terminal<CrosstermBackend<TerminalHandle>>;

pub struct TerminalHandle {
    sender: UnboundedSender<Vec<u8>>,
    // The sink collects the data which is finally sent to sender.
    sink: Vec<u8>,
}

impl TerminalHandle {
    pub async fn start(handle: Handle, channel_id: ChannelId) -> Self {
        let (sender, mut receiver) = unbounded_channel::<Vec<u8>>();
        tokio::spawn(async move {
            while let Some(data) = receiver.recv().await {
                // Ignore this result. Usually error happens when user exits
                // the app and connection is broken mid render.
                let res = handle.data(channel_id, data.into()).await;
                if res.is_err() {
                    break;
                }
            }
        });
        Self {
            sender,
            sink: Vec::new(),
        }
    }
}

// The crossterm backend writes to the terminal handle.
impl std::io::Write for TerminalHandle {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.sink.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        let result = self.sender.send(self.sink.clone());
        if let Err(err) = result {
            return Err(std::io::Error::new(std::io::ErrorKind::BrokenPipe, err));
        }

        self.sink.clear();
        Ok(())
    }
}

pub struct AppServer {
    port: u16,
    key_path: PathBuf,
}

impl AppServer {
    pub fn new(port: u16, key_path: PathBuf) -> Self {
        Self { port, key_path }
    }

    pub async fn run(&mut self) -> anyhow::Result<()> {
        let config = Config {
            inactivity_timeout: Some(Duration::from_secs(3600)),
            auth_rejection_time: Duration::from_secs(3),
            auth_rejection_time_initial: Some(Duration::from_secs(0)),
            keys: vec![get_key(&self.key_path)?],
            nodelay: true,
            ..Default::default()
        };

        self.run_on_address(Arc::new(config), ("0.0.0.0", self.port))
            .await?;
        Ok(())
    }
}

impl Server for AppServer {
    type Handler = Handler;

    fn new_client(&mut self, _: Option<std::net::SocketAddr>) -> Self::Handler {
        println!("[DEBUG]: New connection");
        Handler::new()
    }
}

fn get_key(path: &Path) -> anyhow::Result<PrivateKey> {
    // Try reading existing key
    let key = PrivateKey::read_openssh_file(path);
    if let Ok(key) = key {
        println!("[INFO]: Read private key");
        return Ok(key);
    }

    // Create a new key
    println!("[INFO]: Creating new private key");
    let key = PrivateKey::random(&mut OsRng, ssh_key::Algorithm::Ed25519)?;
    key.write_openssh_file(path, ssh_key::LineEnding::LF)?;

    Ok(key)
}
