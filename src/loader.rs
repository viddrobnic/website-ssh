use std::sync::Arc;

use simple_rss_lib::data::{Data, RefreshStatus};

#[derive(Clone)]
pub struct Loader {
    data: Arc<std::sync::Mutex<Data>>,
}

impl Loader {
    pub fn new() -> Self {
        Self {
            data: Arc::new(std::sync::Mutex::new(Data {
                channels: vec![],
                items: vec![],
            })),
        }
    }
}

impl simple_rss_lib::data::Loader for Loader {
    fn get_data(&self) -> std::sync::MutexGuard<simple_rss_lib::data::Data> {
        self.data.lock().unwrap()
    }

    fn get_version(&self) -> u16 {
        0
    }

    async fn refresh(&mut self) -> RefreshStatus {
        RefreshStatus::Ok
    }

    fn set_read(&mut self, _: usize, _: bool) {}

    async fn load_item(_: &str) -> String {
        String::new()
    }
}
