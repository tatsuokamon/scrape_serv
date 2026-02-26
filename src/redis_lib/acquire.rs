use std::{sync::Arc, time::Duration};

use bb8::{Pool, PooledConnection, RunError};
use bb8_redis::RedisConnectionManager;
use redis::{AsyncConnectionConfig, aio::MultiplexedConnection};

type ThreadSafeBackoffFn = dyn Fn(Duration) -> Duration + Send + Sync + 'static;

#[derive(thiserror::Error, Debug)]
pub enum AcquireErr {
    #[error("{0}")]
    RunErr(#[from] RunError<redis::RedisError>),

    #[error("{0}")]
    RedisErr(#[from] redis::RedisError),

    #[error("")]
    OverRetry,
}

pub trait AcquireConfigTrait<Src, E>: Send {
    type Output<'a>
    where
        Src: 'a;

    async fn acquire<'b>(&self, src: &'b Src) -> Result<Self::Output<'b>, E>;
    async fn acquire_with_retry<'b>(&self, src: &'b Src, retry: i32)
    -> Result<Self::Output<'b>, E>;
    async fn acquire_anyway<'b>(&self, src: &'b Src) -> Self::Output<'b>;
}

pub struct PoolAcquireConfig {
    pub init_backoff: Duration,
    pub backoff_next: Arc<ThreadSafeBackoffFn>,
}

pub struct ClientAcquireConfig {
    pub init_backoff: Duration,
    pub backoff_next: Arc<ThreadSafeBackoffFn>,
    pub async_config: AsyncConnectionConfig,
}

impl AcquireConfigTrait<Arc<Pool<RedisConnectionManager>>, AcquireErr> for PoolAcquireConfig {
    type Output<'a> = PooledConnection<'a, RedisConnectionManager>;

    async fn acquire<'b>(
        &self,
        src: &'b Arc<Pool<RedisConnectionManager>>,
    ) -> Result<PooledConnection<'b, RedisConnectionManager>, AcquireErr> {
        Ok(src.get().await?)
    }

    async fn acquire_with_retry<'b>(
        &self,
        src: &'b Arc<Pool<RedisConnectionManager>>,
        retry: i32,
    ) -> Result<Self::Output<'b>, AcquireErr> {
        let mut tempt = 0;
        let mut backoff = self.init_backoff;

        while tempt < retry {
            match self.acquire(src).await {
                Ok(conn) => {
                    return Ok(conn);
                }
                Err(e) => {
                    tracing::error!("{}", &e);

                    tempt += 1;
                    if retry <= tempt {
                        return Err(e);
                    }

                    tokio::time::sleep(backoff).await;
                    backoff = (self.backoff_next)(backoff);
                }
            }
        }

        Err(AcquireErr::OverRetry)
    }

    async fn acquire_anyway<'b>(
        &self,
        src: &'b Arc<Pool<RedisConnectionManager>>,
    ) -> Self::Output<'b> {
        let mut backoff = self.init_backoff;

        loop {
            match self.acquire(src).await {
                Ok(conn) => {
                    return conn;
                }
                Err(e) => {
                    tracing::error!("{}", &e);
                    tokio::time::sleep(backoff).await;
                    backoff = (self.backoff_next)(backoff);
                }
            }
        }
    }
}

impl AcquireConfigTrait<Arc<redis::Client>, AcquireErr> for ClientAcquireConfig {
    type Output<'a>
        = MultiplexedConnection
    where
        Arc<redis::Client>: 'a;

    async fn acquire<'b>(
        &self,
        src: &'b Arc<redis::Client>,
    ) -> Result<Self::Output<'b>, AcquireErr> {
        Ok(src
            .get_multiplexed_async_connection_with_config(&self.async_config)
            .await?)
    }

    async fn acquire_with_retry<'b>(
        &self,
        src: &'b Arc<redis::Client>,
        retry: i32,
    ) -> Result<Self::Output<'b>, AcquireErr> {
        let mut tempt = 0;
        let mut backoff = self.init_backoff;

        while tempt < retry {
            match self.acquire(src).await {
                Ok(conn) => {
                    return Ok(conn);
                }
                Err(e) => {
                    tracing::error!("{}", &e);

                    tempt += 1;
                    if retry <= tempt {
                        return Err(e);
                    }

                    tokio::time::sleep(backoff).await;
                    backoff = (self.backoff_next)(backoff);
                }
            }
        }

        Err(AcquireErr::OverRetry)
    }

    async fn acquire_anyway<'b>(&self, src: &'b Arc<redis::Client>) -> Self::Output<'b> {
        let mut backoff = self.init_backoff;

        loop {
            match self.acquire(src).await {
                Ok(conn) => {
                    return conn;
                }
                Err(e) => {
                    tracing::error!("{}", &e);
                    tokio::time::sleep(backoff).await;
                    backoff = (self.backoff_next)(backoff);
                }
            }
        }
    }
}
