use std::pin::Pin;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

use crate::engine::err::EngineErr;
use crate::redis_communication::RedisRequest;
use crate::redis_window::{RedisWindow, RedisWindowBuilder};
use crate::scrape_window::ScrapeWindow;
use crate::thread_handler::ThreadHandler;

pub struct Engine {
    scrape_window: ScrapeWindow,
    redis_window: RedisWindow,
}

#[derive(Default)]
pub struct EngineBuilder {
    semaphore_size: usize,
    redis_url: String,
    max_size: u32,
    channel_buf: usize,
}

impl EngineBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn semaphore_size(mut self, semaphore_size: usize) -> Self {
        self.semaphore_size = semaphore_size;
        self
    }

    pub fn redis_url(mut self, redis_url: String) -> Self {
        self.redis_url = redis_url;
        self
    }

    pub fn max_size(mut self, max_size: u32) -> Self {
        self.max_size = max_size;
        self
    }

    pub fn channel_buf(mut self, channel_buf: usize) -> Self {
        self.channel_buf = channel_buf;
        self
    }

    pub async fn build(self) -> Result<Engine, EngineErr> {
        Ok(Engine {
            redis_window: RedisWindowBuilder::new()
                .channel_buf(self.channel_buf)
                .max_size(self.max_size)
                .redis_url(self.redis_url)
                .build()
                .await?,
            scrape_window: ScrapeWindow::new(self.semaphore_size),
        })
    }
}

impl Engine {
    pub async fn create_path<RR>(
        &self,
        redis_queue_keyword: String,
        redis_hash_keyword: String,
        scraper: impl Fn(reqwest::Client, String) -> Pin<Box<dyn Future<Output = String> + Send>>
        + Send
        + Sync
        + 'static,
    ) -> Result<ThreadHandler, EngineErr>
    where
        RR: RedisRequest + serde::de::DeserializeOwned,
    {
        let mut set = JoinSet::new();
        let token = CancellationToken::new();
        let redis_req_rx: Receiver<String>;
        let redis_result_tx: Sender<(String, String)>;

        match self
            .redis_window
            .redis_queue_factory(&mut set, token.child_token(), redis_queue_keyword)
            .await
        {
            Ok(rx) => {
                redis_req_rx = rx;
            }
            Err(e) => {
                tracing::error!("{}", &e);
                token.cancel();
                while set.join_next().await.is_some() {}
                return Err(e.into());
            }
        };

        match self
            .redis_window
            .redis_hash_factory(&mut set, token.child_token(), redis_hash_keyword)
            .await
        {
            Ok(tx) => {
                redis_result_tx = tx;
            }
            Err(e) => {
                tracing::error!("{}", &e);
                token.cancel();
                while set.join_next().await.is_some() {}
                return Err(e.into());
            }
        };

        let job_pusher = self.redis_window.job_push_queue_factory().await;

        self.scrape_window.spawn_scraper::<RR, _, _>(
            &mut set,
            token.child_token(),
            scraper,
            redis_req_rx,
            redis_result_tx,
            job_pusher,
        );

        Ok(ThreadHandler { token, set })
    }
}
