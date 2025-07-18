use std::{ops::DerefMut, sync::Arc, time::Duration};

use russh::{ChannelId, server::*};
use simple_rss_lib::{
    app::App,
    event::{Event, EventBus, KeyboardEvent},
};
use tokio::sync::{Mutex, oneshot};

use crate::{loader::Loader, server::SshTerminal};

const TICK_FPS: f64 = 30.0;

pub struct AppSession {
    pub terminal: SshTerminal,
    pub app: App<Loader>,
}

/// A task that runs in a background. Responsible for processing
/// events and redrawing the app.
pub struct AppRunner {
    app_session: Arc<Mutex<AppSession>>,
    bus: EventBus,

    session_handle: Handle,
    channel: ChannelId,

    // Used to detect when the handler was closed
    close_receiver: oneshot::Receiver<()>,
}

impl AppRunner {
    pub fn new(
        app_session: Arc<Mutex<AppSession>>,
        bus: EventBus,
        session_handle: Handle,
        channel: ChannelId,
        close_receiver: oneshot::Receiver<()>,
    ) -> Self {
        Self {
            app_session,
            bus,
            session_handle,
            channel,
            close_receiver,
        }
    }

    pub async fn start(mut self) {
        println!("[INFO]: Starting AppRunner");

        let tick_rate = Duration::from_secs_f64(1.0 / TICK_FPS);
        let mut tick = tokio::time::interval(tick_rate);

        loop {
            let tick_delay = tick.tick();
            let bus_event = self.bus.next();
            let event = tokio::select! {
                _ = tick_delay => Event::Tick,
                Some(val) = bus_event => val,
                _ = &mut self.close_receiver => break,
            };

            let mut sess_lock = self.app_session.lock().await;
            let sess = sess_lock.deref_mut();

            let state = sess.app.handle_event(&event);

            if state.is_handled() {
                let res = sess.terminal.draw(|f| sess.app.draw(f));
                match res {
                    Ok(_) => continue,
                    Err(err) => {
                        println!("[ERROR]: Failed to render: {err}");
                        let _ = self.session_handle.close(self.channel).await;
                        break;
                    }
                }
            }

            if event == Event::Keyboard(KeyboardEvent::Back) {
                println!("[DEBUG]: User exited");
                let _ = self.session_handle.close(self.channel).await;
                break;
            }
        }

        println!("[INFO]: Stopping AppRunner")
    }
}
