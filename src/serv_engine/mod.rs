mod post_process;
mod prior_process;
mod scrape_process;

use std::{pin::Pin, sync::Arc};

use bb8::Pool;
use bb8_redis::RedisConnectionManager;
use sha2::Digest;
use tokio::{sync::Semaphore, task::JoinSet};
use tokio_util::sync::CancellationToken;

use crate::{
    redis_communication::RedisRequest,
    redis_lib::{
        ClientAcquireConfig, PoolAcquireConfig, RedisLibErr, ReqFetchContract, RequestFetcherErr,
        invoke_req_fetcher,
    },
    scraper::ScrapeErr,
    serv_engine::{
        post_process::{PostProcessErr, invoke_post_process},
        prior_process::{PriorProcessErr, invoke_prior_process},
        scrape_process::{ScrapeProcessErr, invoke_scrape_process},
    },
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

    #[error("{0}")]
    ScrapeErr(#[from] ScrapeErr),

    #[error("{0}")]
    InvokePriorProcess(#[from] PriorProcessErr),

    #[error("{0}")]
    InvokeScrapeProcess(#[from] ScrapeProcessErr),

    #[error("{0}")]
    InvokePostProcess(#[from] PostProcessErr),
}

pub struct ProcessReqContract {
    pub result_keyword: String,
    pub storage_time: usize,
    pub scraper: Arc<Scraper<Result<String, ScrapeErr>>>,
    pub inner_buf: usize,
    pub semaphore: Arc<Semaphore>,
}

pub async fn create_path<RR>(
    // request part
    redis_client: Arc<redis::Client>,
    redis_client_config: Arc<ClientAcquireConfig>,

    req_fetch_contract: ReqFetchContract,

    // process request part
    pool: Arc<Pool<RedisConnectionManager>>,
    pool_config: Arc<PoolAcquireConfig>,

    process_request_contract: ProcessReqContract,
) -> Result<ThreadHandler, EngineErr>
where
    RR: RedisRequest + serde::de::DeserializeOwned,
{
    let mut set = JoinSet::new();
    let token = CancellationToken::new();

    // invoke thread to fetch request from redis server while set alive and not cancelled
    let req_rx = invoke_req_fetcher(
        &mut set,
        token.child_token(),
        redis_client,
        redis_client_config,
        req_fetch_contract,
    )
    .await?;
    let (tx_of_process_info, rx_of_process_info) =
        tokio::sync::mpsc::channel(process_request_contract.inner_buf);
    let (tx_of_scrape_result, rx_of_scrape_result) =
        tokio::sync::mpsc::channel(process_request_contract.inner_buf);
    //
    // invoke thread to process request
    // prior process
    invoke_prior_process::<RR>(
        &mut set,
        token.child_token(),
        pool.clone(),
        pool_config.clone(),
        req_rx,
        tx_of_process_info,
    )
    .await?;

    // scrape process
    invoke_scrape_process(
        &mut set,
        token.child_token(),
        process_request_contract.semaphore.clone(),
        rx_of_process_info,
        tx_of_scrape_result,
        process_request_contract.scraper.clone(),
    )
    .await?;

    // post process
    invoke_post_process(
        &mut set,
        token.child_token(),
        pool.clone(),
        pool_config.clone(),
        rx_of_scrape_result,
        process_request_contract.result_keyword,
        process_request_contract.storage_time,
    )
    .await?;

    Ok(ThreadHandler { set, token })
}

pub struct ProcessItem {
    pub id: String,
    pub job_id: String,
    pub idx: i32,

    pub url: String,
    pub need_request: bool,
}

pub fn create_identifier(url: &str) -> String {
    let mut hasher = sha2::Sha256::new();
    hasher.update(url.as_bytes());

    hex::encode(hasher.finalize())
}
