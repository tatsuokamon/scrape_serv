use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use bb8::Pool;
use bb8_redis::RedisConnectionManager;
use redis::Client;
use tokio::sync::mpsc::Sender;
use tokio::{sync::mpsc::Receiver, task::JoinSet};
use tokio_util::sync::CancellationToken;

use crate::redis_window::err::RedisWindowErr;
use crate::redis_window::library::{
    job_push_queue_factory as _job_push_queue_factory, redis_hash_factory as _redis_hash_factory,
    redis_queue_factory as _redis_queue_factory,
};

pub struct RedisWindow {
    pool: Arc<Pool<RedisConnectionManager>>,
    channel_buf: usize,
    client: Arc<Client>,
}

impl RedisWindow {
    pub async fn redis_queue_factory(
        &self,
        set: &mut JoinSet<()>,
        token: CancellationToken,
        queue_name: String,
    ) -> Result<Receiver<String>, RedisWindowErr> {
        _redis_queue_factory(
            set,
            token,
            self.client.clone(),
            queue_name,
            self.channel_buf,
        )
        .await
    }

    pub async fn redis_hash_factory(
        &self,
        set: &mut JoinSet<()>,
        token: CancellationToken,
        hash_name: String,
    ) -> Result<Sender<(String, String)>, RedisWindowErr> {
        // (id, serialized_redis_response)
        _redis_hash_factory(set, token, self.pool.clone(), hash_name, self.channel_buf).await
    }

    pub async fn job_push_queue_factory(
        &self,
    ) -> impl Fn(String, String) -> Pin<Box<dyn Future<Output = Result<(), RedisWindowErr>> + Send>>
    + 'static
    + Send
    + Sync {
        _job_push_queue_factory(self.pool.clone()).await
    }
}

#[derive(Default)]
pub struct RedisWindowBuilder {
    redis_url: String,
    max_size: u32,
    channel_buf: usize,
}

impl RedisWindowBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn max_size(mut self, size: u32) -> Self {
        self.max_size = size;
        self
    }

    pub fn redis_url(mut self, url: impl Into<String>) -> Self {
        self.redis_url = url.into();
        self
    }

    pub fn channel_buf(mut self, buf: usize) -> Self {
        self.channel_buf = buf;
        self
    }

    pub async fn build(self) -> Result<RedisWindow, RedisWindowErr> {
        let manager = RedisConnectionManager::new(self.redis_url.clone())?;
        let client = Client::open(self.redis_url)?;
        Ok(RedisWindow {
            pool: Arc::new(
                Pool::builder()
                    .max_size(self.max_size)
                    .connection_timeout(Duration::from_secs(30))
                    .build(manager)
                    .await?,
            ),
            channel_buf: self.channel_buf,
            client: Arc::new(client),
        })
    }
}
