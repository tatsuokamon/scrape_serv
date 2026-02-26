mod acquire;
mod req_fetch;

use bb8::PooledConnection;
use bb8_redis::RedisConnectionManager;
use redis::{AsyncCommands, RedisError};
use sha2::Digest;
use std::string::FromUtf8Error;

pub use acquire::{AcquireConfigTrait, ClientAcquireConfig, PoolAcquireConfig};
pub use req_fetch::{ReqFetchContract, RequestFetcherErr, invoke_req_fetcher};

#[derive(thiserror::Error, Debug)]
pub enum RedisLibErr {
    #[error("{0}")]
    FromUtf8Err(#[from] FromUtf8Error),

    #[error("{0}")]
    RedisErr(#[from] RedisError),
}

pub async fn is_recentry_got(
    identifier: &String,
    conn: &mut PooledConnection<'_, RedisConnectionManager>,
) -> Result<bool, RedisLibErr> {
    Ok(conn
        .get::<&String, Option<String>>(identifier)
        .await?
        .is_some())
}

pub async fn update_job_status(
    job_id: &String,
    task_id: &String,
    conn: &mut PooledConnection<'_, RedisConnectionManager>,
) -> Result<(), RedisLibErr> {
    Ok(conn.lpush::<&str, &str, ()>(job_id, task_id).await?)
}

pub async fn update_recently_got(
    identifier: &String,
    conn: &mut PooledConnection<'_, RedisConnectionManager>,
    storage_time: usize,
) -> Result<(), RedisLibErr> {
    Ok(redis::cmd("SET")
        .arg(identifier)
        .arg(1)
        .arg("NX")
        .arg("EX")
        .arg(storage_time)
        .query_async(&mut **conn)
        .await?)
}

pub async fn push_result(
    keyword: &String,
    task_id: &String,
    result: &String,
    conn: &mut PooledConnection<'_, RedisConnectionManager>,
) -> Result<(), RedisLibErr> {
    Ok(conn.hset(keyword, task_id, result).await?)
}
