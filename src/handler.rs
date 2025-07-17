use std::time::Duration;

use ratatui::{Terminal, TerminalOptions, Viewport, layout::Rect, prelude::CrosstermBackend};
use russh::{Channel, ChannelId, Pty, keys::ssh_key, server::*};
use simple_rss_lib::{
    app::App,
    event::{Event, EventBus, EventSender},
};

use crate::{
    loader::Loader,
    server::{SshTerminal, TerminalHandle},
};

const TICK_FPS: f64 = 30.0;

struct HandlerData {
    terminal: SshTerminal,
    bus: EventBus,
    app: App<Loader>,
}

#[derive(Default)]
pub struct Handler {
    data: Option<HandlerData>,
}

impl Handler {
    pub fn new() -> Self {
        Self::default()
    }
}

async fn start_ticker(sender: EventSender) {
    println!("[INFO]: Starting new ticker");

    let tick_rate = Duration::from_secs_f64(1.0 / TICK_FPS);
    let mut tick = tokio::time::interval(tick_rate);
    loop {
        let tick_delay = tick.tick();
        tokio::select! {
          _ = sender.closed() => {
            break;
          }
          _ = tick_delay => {
            sender.send(Event::Tick);
          }
        };
    }

    println!("[INFO]: Stopping ticker")
}

impl russh::server::Handler for Handler {
    type Error = anyhow::Error;

    async fn auth_publickey(
        &mut self,
        _: &str,
        _: &ssh_key::PublicKey,
    ) -> Result<Auth, Self::Error> {
        Ok(Auth::Accept)
    }

    async fn channel_open_session(
        &mut self,
        channel: Channel<Msg>,
        session: &mut Session,
    ) -> Result<bool, Self::Error> {
        let terminal_handle = TerminalHandle::start(session.handle(), channel.id()).await;
        let backend = CrosstermBackend::new(terminal_handle);

        // The correct viewport area will be set when the client request a pty
        let options = TerminalOptions {
            viewport: Viewport::Fixed(Rect::default()),
        };

        let terminal = Terminal::with_options(backend, options)?;
        let bus = EventBus::new();
        let sender = bus.get_sender();
        let app = App::new(bus.get_sender(), Loader::new(), 30);

        tokio::spawn(start_ticker(sender));
        self.data = Some(HandlerData { terminal, bus, app });

        Ok(true)
    }

    /// The client's window size has changed.
    async fn window_change_request(
        &mut self,
        _: ChannelId,
        col_width: u32,
        row_height: u32,
        _: u32,
        _: u32,
        _: &mut Session,
    ) -> Result<(), Self::Error> {
        let rect = Rect {
            x: 0,
            y: 0,
            width: col_width as u16,
            height: row_height as u16,
        };

        let data = self.data.as_mut().unwrap();

        data.terminal.resize(rect)?;
        data.terminal.draw(|f| data.app.draw(f))?;

        Ok(())
    }

    async fn pty_request(
        &mut self,
        channel: ChannelId,
        _: &str,
        col_width: u32,
        row_height: u32,
        _: u32,
        _: u32,
        _: &[(Pty, u32)],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        let rect = Rect {
            x: 0,
            y: 0,
            width: col_width as u16,
            height: row_height as u16,
        };

        let data = self.data.as_mut().unwrap();

        data.terminal.resize(rect)?;
        data.terminal.draw(|f| data.app.draw(f))?;

        session.channel_success(channel)?;

        Ok(())
    }

    async fn data(
        &mut self,
        chan: ChannelId,
        data: &[u8],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        // TODO: Implement me
        if data[0] == 'q' as u8 {
            session.close(chan)?;
        }

        println!("[DEBUG]: Got data: {:?}", data);
        Ok(())
    }
}
