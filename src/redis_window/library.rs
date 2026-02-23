use std::{pin::Pin, sync::Arc, time::Duration};

use bb8::Pool;
use bb8_redis::RedisConnectionManager;
use redis::{AsyncCommands, AsyncConnectionConfig, Client, aio::MultiplexedConnection};
use tokio::{
    sync::mpsc::{Receiver, Sender},
    task::JoinSet,
};
use tokio_util::sync::CancellationToken;

use crate::redis_window::err::RedisWindowErr;

pub async fn redis_queue_factory(
    set: &mut JoinSet<()>,
    token: CancellationToken,
    client: Arc<Client>,
    queue_name: String,
    channel_buf: usize,
) -> Result<Receiver<String>, RedisWindowErr> {
    let (tx, rx) = tokio::sync::mpsc::channel(channel_buf);

    set.spawn(async move {
        loop {
            let mut conn: MultiplexedConnection;
            match client
                .get_multiplexed_async_connection_with_config(
                    &AsyncConnectionConfig::new().set_response_timeout(None),
                )
                .await
            {
                Ok(got_conn) => {
                    conn = got_conn;
                }
                Err(e) => {
                    tracing::error!("{}", e);
                    tokio::time::sleep(Duration::from_secs(3)).await;
                    continue;
                }
            };

            tokio::select! {
                received_result = conn.blpop::<_, (String, String)>(&queue_name, 5.0) => {
                    match received_result {
                        Ok(received) => {
                            let (_, received_redis_request) = received;
                            if let Err(e) = tx.send(received_redis_request).await {
                                tracing::error!("{}", e);
                            }
                        },
                        Err(e) => {
                            tracing::error!("{}", e);
                        }
                    }
                },
                _ = token.cancelled() => {
                    break;
                }
            }
        }
    });

    Ok(rx)
}

pub async fn job_push_queue_factory(
    pool: Arc<Pool<RedisConnectionManager>>,
) -> impl Fn(String, String) -> Pin<Box<dyn Future<Output = Result<(), RedisWindowErr>> + Send>>
+ 'static
+ Send {
    move |job_id: String, id: String| {
        let pool = pool.clone();

        Box::pin(async move {
            let mut conn = pool.get().await?;
            conn.lpush::<_, _, ()>(job_id, id).await?;
            Ok(())
        })
    }
}

pub async fn redis_hash_factory(
    set: &mut JoinSet<()>,
    token: CancellationToken,
    pool: Arc<Pool<RedisConnectionManager>>,
    hash_name: String,
    channel_buf: usize,
) -> Result<Sender<(String, String)>, RedisWindowErr> {
    // (id, Serialized RedisResponse)
    let (tx, mut rx) = tokio::sync::mpsc::channel(channel_buf);

    set.spawn(async move {
        loop {
            tokio::select! {
                redis_result_string_result = rx.recv() => {
                    match redis_result_string_result {
                        Some(received) => {
                            let (id, serialized_redis_response) = received;
                            match pool.get().await {
                                Ok(mut conn) => {
                                    if let Err(e) = conn.hset::<_, _, _, ()>(&hash_name, id, serialized_redis_response).await {
                                        tracing::error!("{}", e);
                                    };
                                },
                                Err(e) => {
                                    tracing::error!("{}", e);
                                }
                            }
                        },
                        None => {
                            tracing::error!("recevier may be droppped: redis_hash_factory");
                            break;
                        }
                    }
                },
                _ = token.cancelled() => {
                    break;
                }
            }
        }
    });

    Ok(tx)
}
