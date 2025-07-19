use std::{collections::HashMap, ops::DerefMut, sync::Arc};

use ratatui::{
    Terminal, TerminalOptions, Viewport, layout::Rect, prelude::CrosstermBackend, style::Stylize,
    widgets::Paragraph,
};
use russh::{Channel, ChannelId, Pty, keys::ssh_key, server::*};
use simple_rss_lib::{
    app::{App, AppConfig},
    event::{Event, EventBus, EventSender, KeyboardEvent},
};
use tokio::sync::{Mutex, oneshot};

use crate::{
    app::{AppRunner, AppSession},
    loader::Loader,
    server::TerminalHandle,
};

#[derive(Default)]
pub struct Handler {
    apps: HashMap<ChannelId, Arc<Mutex<AppSession>>>,
    event_senders: HashMap<ChannelId, EventSender>,

    close_tokens: HashMap<ChannelId, oneshot::Sender<()>>,
}

impl Handler {
    pub fn new() -> Self {
        Self::default()
    }

    fn get_session(&self, channel: &ChannelId) -> anyhow::Result<Arc<Mutex<AppSession>>> {
        let session = self
            .apps
            .get(channel)
            .ok_or_else(|| anyhow::anyhow!("Invalid channel Id"))?;
        Ok(session.clone())
    }
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

        // The correct viewport area will be set when the client request a pty.
        // These are some dummy values large enough for overflow not to happen.
        let options = TerminalOptions {
            viewport: Viewport::Fixed(Rect::new(0, 0, 100, 100)),
        };

        let terminal = Terminal::with_options(backend, options)?;
        let bus = EventBus::new();
        let app = App::new(
            AppConfig {
                item_list_custom_empty_msg: Some(
                    Paragraph::new("Loading items...").centered().bold(),
                ),
                disable_read_status: true,
                disable_channel_names: true,
                disable_browser_open: true,
            },
            bus.get_sender(),
            Loader::new(),
            30,
        );

        let app_session = Arc::new(Mutex::new(AppSession { terminal, app }));
        let (tx, rx) = oneshot::channel();

        self.apps.insert(channel.id(), app_session.clone());
        self.event_senders.insert(channel.id(), bus.get_sender());
        self.close_tokens.insert(channel.id(), tx);

        let runner = AppRunner::new(app_session, bus, session.handle(), channel.id(), rx);
        tokio::spawn(runner.start());

        Ok(true)
    }

    /// The client's window size has changed.
    async fn window_change_request(
        &mut self,
        channel: ChannelId,
        col_width: u32,
        row_height: u32,
        _: u32,
        _: u32,
        handle_session: &mut Session,
    ) -> Result<(), Self::Error> {
        let rect = Rect {
            x: 0,
            y: 0,
            width: col_width as u16,
            height: row_height as u16,
        };

        let session = self.get_session(&channel)?;
        let mut sess_lock = session.lock().await;
        let sess = sess_lock.deref_mut();

        sess.terminal.resize(rect)?;
        sess.terminal.draw(|f| sess.app.draw(f))?;

        handle_session.channel_success(channel)?;
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
        handle_session: &mut Session,
    ) -> Result<(), Self::Error> {
        let rect = Rect {
            x: 0,
            y: 0,
            width: col_width as u16,
            height: row_height as u16,
        };

        let session = self.get_session(&channel)?;
        let mut sess_lock = session.lock().await;
        let sess = sess_lock.deref_mut();

        sess.terminal.resize(rect)?;
        sess.terminal.draw(|f| sess.app.draw(f))?;

        handle_session.channel_success(channel)?;
        Ok(())
    }

    async fn data(
        &mut self,
        channel: ChannelId,
        data: &[u8],
        _: &mut Session,
    ) -> Result<(), Self::Error> {
        // This should probably be a more correct way of handling ansii escape codes.
        // But this seems to do the trick in cases I tested.
        let event = match data {
            [b'h'] | [27, 91, 68] => KeyboardEvent::Left,
            [b'l'] | [27, 91, 67] => KeyboardEvent::Right,
            [b'k'] | [27, 91, 65] => KeyboardEvent::Up,
            [b'j'] | [27, 91, 66] => KeyboardEvent::Down,
            [b'q'] | [27] => KeyboardEvent::Back,
            [13] => KeyboardEvent::Enter,
            [b' '] => KeyboardEvent::Space,
            [b'o'] => KeyboardEvent::Open,
            [b'?'] => KeyboardEvent::Help,

            // Ignore other events
            _ => return Ok(()),
        };
        println!("[DEBUG]: Got event: {event:?}");

        let sender = self.event_senders.get(&channel);
        if let Some(sender) = sender {
            sender.send(Event::Keyboard(event));
        }

        Ok(())
    }
}
