use std::{pin::Pin, sync::Arc};

use bb8::{Pool, PooledConnection};
use bb8_redis::RedisConnectionManager;
use sha2::Digest;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

use crate::{
    redis_communication::{RedisRequest, RedisResponse},
    redis_lib::{
        AcquireConfigTrait, ClientAcquireConfig, PoolAcquireConfig, RedisLibErr, ReqFetchContract,
        RequestFetcherErr, invoke_req_fetcher, is_recentry_got, push_result, update_job_status,
        update_recently_got,
    },
    scraper::ScrapeErr,
    thread_handler::ThreadHandler,
};

type Scraper<Output> = dyn Fn(reqwest::Client, String) -> Pin<Box<dyn Future<Output = Output> + Send + Sync + 'static>>
    + Send
    + Sync
    + 'static;

#[derive(thiserror::Error, Debug)]
pub enum EngineErr {
    #[error("{0}")]
    RequestFetcherErr(#[from] RequestFetcherErr),

    #[error("{0}")]
    SerdeJsonErr(#[from] serde_json::Error),

    #[error("{0}")]
    RedisLibErr(#[from] RedisLibErr),
}

pub struct ProcessReqContract {
    pub result_keyword: String,
    pub storage_time: usize,
    pub scraper: Arc<Scraper<Result<String, ScrapeErr>>>,
}

pub async fn create_path<RR>(
    // request part
    redis_client: Arc<redis::Client>,
    redis_client_config: Arc<ClientAcquireConfig>,

    req_fetch_contract: ReqFetchContract,

    // process request part
    pool: Arc<Pool<RedisConnectionManager>>,
    pool_config: Arc<PoolAcquireConfig>,
    client: reqwest::Client,
    process_request_contract: ProcessReqContract,
) -> Result<ThreadHandler, EngineErr>
where
    RR: RedisRequest + serde::de::DeserializeOwned,
{
    let mut set = JoinSet::new();
    let token = CancellationToken::new();

    // invoke thread to fetch request from redis server while set alive and not cancelled
    let mut req_rx = invoke_req_fetcher(
        &mut set,
        token.child_token(),
        redis_client,
        redis_client_config,
        req_fetch_contract,
    )
    .await?;

    // invoke thread to process request from Receiver while set alize and not cancelled
    let child_token = token.child_token();
    set.spawn(async move {
        tokio::select! {
            _ = async move {
                let pool = pool;
                let mut conn = pool_config.acquire_anyway(&pool).await;

                while let Some(received) = req_rx.recv().await {
                    if let Err(e) = process_request_part::<RR>(
                        received,
                        &mut conn,
                        client.clone(),
                        &process_request_contract
                    ).await {
                        tracing::error!("{e}");
                        conn = pool_config.acquire_anyway(&pool).await;
                    };
                }
            } => {
                tracing::info!("processing request thread finished")
            },
            _ = child_token.cancelled() => {
                tracing::warn!("processing request thread cancelled")
            }
        }
    });

    Ok(ThreadHandler { set, token })
}

pub async fn process_request_part<RR>(
    received: String,
    conn: &mut PooledConnection<'_, RedisConnectionManager>,
    client: reqwest::Client,
    process_request_contract: &ProcessReqContract,
) -> Result<(), EngineErr>
where
    RR: RedisRequest + serde::de::DeserializeOwned,
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

    let redis_response = {
        if !is_forced && check_if_recently_got(&url, conn).await? {
            RedisResponse {
                error: Some("not forced and recently got".into()),
                payload: None,
                index: idx,
            }
        } else {
            let mut temp_resp = scrape_to_redis_response_without_idx(
                url.clone(),
                client,
                process_request_contract.scraper.clone(),
            )
            .await;
            temp_resp.index = idx;
            update_recently_status(&url, conn, 300).await?;

            temp_resp
        }
    };

    let redis_result_string = serde_json::to_string(&redis_response).unwrap();
    push_result(
        &process_request_contract.result_keyword,
        &id,
        &redis_result_string,
        conn,
    )
    .await?;
    update_job_status(&job_id, &id, conn).await?;

    Ok(())
}

pub fn create_identifier(url: &String) -> String {
    let mut hasher = sha2::Sha256::new();
    hasher.update(url.as_bytes());

    hex::encode(hasher.finalize())
}

pub async fn check_if_recently_got(
    url: &String,
    conn: &mut PooledConnection<'_, RedisConnectionManager>,
) -> Result<bool, EngineErr> {
    let identifier = create_identifier(url);
    Ok(is_recentry_got(&identifier, conn).await?)
}

pub async fn update_recently_status(
    url: &String,
    conn: &mut PooledConnection<'_, RedisConnectionManager>,
    storage_time: usize,
) -> Result<(), EngineErr> {
    let identifier = create_identifier(url);
    Ok(update_recently_got(&identifier, conn, storage_time).await?)
}

pub async fn scrape_to_redis_response_without_idx(
    url: String,
    client: reqwest::Client,
    scraper: Arc<Scraper<Result<String, ScrapeErr>>>,
) -> RedisResponse {
    match scraper(client, url).await {
        Ok(success_result) => RedisResponse {
            error: None,
            payload: Some(success_result),
            index: -1,
        },
        Err(e) => RedisResponse {
            error: Some(format!("{e}")),
            payload: None,
            index: -1,
        },
    }
}
