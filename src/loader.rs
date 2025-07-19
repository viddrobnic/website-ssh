use std::{
    ops::Deref,
    sync::{Arc, Mutex, MutexGuard},
};

use chrono::{DateTime, FixedOffset};
use serde::Deserialize;
use simple_rss_lib::{
    data::{Item, RefreshStatus},
    event::{Event, EventSender},
};

const BASE_URL: &str = "https://viddrobnic.com";

pub struct Lock<'a>(MutexGuard<'a, Vec<Item>>);

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct Post {
    title: String,
    url: String,
    pub_date: Option<DateTime<FixedOffset>>,
}

impl<'a> Deref for Lock<'a> {
    type Target = Vec<Item>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Clone)]
pub struct Loader {
    data: Arc<Mutex<Vec<Item>>>,
    version: Arc<Mutex<u16>>,
    sender: EventSender,
}

impl Loader {
    pub fn new(sender: EventSender) -> Self {
        Self {
            data: Arc::new(Mutex::new(vec![])),
            version: Arc::new(Mutex::new(0)),
            sender,
        }
    }
}

impl simple_rss_lib::data::Loader for Loader {
    type Guard<'a> = Lock<'a>;

    fn get_items(&self) -> Self::Guard<'_> {
        Lock(self.data.lock().unwrap())
    }

    fn get_version(&self) -> u16 {
        *self.version.lock().unwrap()
    }

    async fn refresh(&mut self) -> RefreshStatus {
        let resp = reqwest::get(format!("{BASE_URL}/api/pages.json")).await;
        let Ok(resp) = resp else {
            println!("[ERROR]: Failed to get pages: {}", resp.unwrap_err());
            return RefreshStatus::Error;
        };

        #[derive(Debug, Deserialize)]
        struct PageResponse {
            posts: Vec<Post>,
        }

        let resp = resp.json::<PageResponse>().await;
        let Ok(resp) = resp else {
            println!(
                "[ERROR]: Failed to deserialize response: {:?}",
                resp.unwrap_err()
            );
            return RefreshStatus::Error;
        };

        let items: Vec<_> = resp
            .posts
            .into_iter()
            .map(|page| Item {
                id: page.url.clone(),
                channel_name: String::new(),
                title: page.title,
                description: None,
                pub_date: page.pub_date,
                link: format!("{}{}", BASE_URL, page.url),
                read: false,
            })
            .collect();

        let about_url = items.get(0).map(|it| it.link.clone());

        let mut data = self.data.lock().unwrap();
        *data = items;

        let mut version = self.version.lock().unwrap();

        // If this is first data load and we got an about url, we
        // start loading about page as well
        if *version == 0
            && let Some(url) = about_url
        {
            let sender = self.sender.clone();
            tokio::spawn(async move {
                let text = Self::load_item(&url).await;
                sender.send(Event::LoadedItem(text));
            });
        }

        *version += 1;

        RefreshStatus::Ok
    }

    fn set_read(&mut self, _: usize, _: bool) {}

    async fn load_item(url: &str) -> String {
        let resp = reqwest::get(url).await;
        match resp {
            Err(err) => {
                println!("[ERROR]: Failed loading item: {err}");
                format!("Failed loading item: {err}")
            }
            Ok(resp) => match resp.text().await {
                Ok(data) => data,
                Err(err) => {
                    println!("[ERROR]: Failed loading item: {err}");
                    format!("Failed loading item: {err}")
                }
            },
        }
    }
}
