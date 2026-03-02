use std::sync::Arc;

use bb8::{Pool, PooledConnection};
use bb8_redis::RedisConnectionManager;
use tokio::{
    sync::mpsc::{Receiver, Sender},
    task::JoinSet,
};
use tokio_util::sync::CancellationToken;

use crate::{
    redis_communication::RedisRequest,
    redis_lib::{AcquireConfigTrait, PoolAcquireConfig, RedisLibErr, is_recentry_got},
    serv_engine::{ProcessItem, create_identifier},
};

#[derive(thiserror::Error, Debug)]
pub enum PriorProcessErr {
    #[error("{0}")]
    SerdeJson(#[from] serde_json::Error),

    #[error("{0}")]
    RedisLibErr(#[from] RedisLibErr),
}

type ProcessResult<T> = Result<T, PriorProcessErr>;

async fn check_if_recently_got(
    url: &String,
    conn: &mut PooledConnection<'_, RedisConnectionManager>,
) -> ProcessResult<bool> {
    let identifier = create_identifier(url);
    Ok(is_recentry_got(&identifier, conn).await?)
}

async fn _prior_process_inner<RR>(
    received: String,
    conn: &mut PooledConnection<'_, RedisConnectionManager>,
) -> ProcessResult<ProcessItem>
where
    RR: serde::de::DeserializeOwned + RedisRequest,
{
    let (url, id, job_id, idx, is_forced) = {
        let redis_req: RR = serde_json::from_str(&received)?;
        Ok::<_, serde_json::Error>((
            redis_req.get_url(),
            redis_req.get_id(),
            redis_req.get_job_id(),
            redis_req.index(),
            redis_req.is_forced(),
        ))
    }?;

    Ok(ProcessItem {
        need_request: is_forced || !check_if_recently_got(&url, conn).await?,
        id,
        job_id,
        url,
        idx,
    })
}

pub async fn invoke_prior_process<RR>(
    set: &mut JoinSet<()>,
    token: CancellationToken,
    pool: Arc<Pool<RedisConnectionManager>>,
    pool_config: Arc<PoolAcquireConfig>,

    mut receiver_from_redis: Receiver<String>,
    tx_of_process_info: Sender<ProcessItem>,
) -> ProcessResult<()>
where
    RR: RedisRequest + serde::de::DeserializeOwned,
{
    set.spawn(async move {
        tokio::select! {
            _ = async move {
                let pool = pool;
                let mut conn = pool_config.acquire_anyway(&pool).await;

                while let Some(received) = receiver_from_redis.recv().await {
                    match _prior_process_inner::<RR>(
                        received,
                        &mut conn,
                    ).await {
                        Ok(item) => {
                            if let Err(e) = tx_of_process_info.send(item).await {
                                tracing::error!("{e}");
                            }
                        },
                        Err(e) => {
                            tracing::error!("{e}");
                            conn = pool_config.acquire_anyway(&pool).await;
                        }
                    };
                }

            } => {
            },
            _ = token.cancelled() => {
            }
        }
    });

    Ok(())
}
