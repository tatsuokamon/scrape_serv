use crate::redis_lib::acquire::{AcquireConfigTrait, AcquireErr, ClientAcquireConfig};
use redis::{AsyncCommands, RedisError, aio::MultiplexedConnection};
use std::sync::Arc;
use tokio::{sync::mpsc::Receiver, task::JoinSet};
use tokio_util::sync::CancellationToken;

#[derive(thiserror::Error, Debug)]
pub enum RequestFetcherErr {
    #[error("")]
    AcquireErr(#[from] AcquireErr),
}

pub struct ReqFetchContract {
    pub channel_buf: usize,
    pub req_q_keyword: String,
    pub blocking_time: f64,
}

pub async fn invoke_req_fetcher(
    set: &mut JoinSet<()>,
    token: CancellationToken,

    client: Arc<redis::Client>,
    client_config: Arc<ClientAcquireConfig>,

    req_fetch_contract: ReqFetchContract,
) -> Result<Receiver<String>, RequestFetcherErr> {
    let conn = client_config.acquire(&client).await?;
    let (tx, rx) = tokio::sync::mpsc::channel(req_fetch_contract.channel_buf);

    set.spawn(async move {
        let mut conn = conn;

        loop {
            tokio::select! {
                fetch_result = req_fetcher_inner_process(&mut conn, req_fetch_contract.blocking_time, &req_fetch_contract.req_q_keyword) => {
                    match fetch_result {
                        Ok(fetched) => {
                            if let Err(e) = tx.send(fetched).await {
                                tracing::error!("{e}");
                            }
                        },
                        Err(e) => {
                            tracing::error!("{e}");
                            conn = client_config.acquire_anyway(&client).await;
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

async fn req_fetcher_inner_process(
    conn: &mut MultiplexedConnection,
    blocking_time: f64,
    req_q_keyword: &String,
) -> Result<String, RedisError> {
    conn.blpop::<&String, String>(req_q_keyword, blocking_time)
        .await
}
