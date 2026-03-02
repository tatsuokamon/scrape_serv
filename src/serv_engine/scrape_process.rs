use std::{pin::Pin, sync::Arc};
use tokio::{
    sync::{
        OwnedSemaphorePermit, Semaphore,
        mpsc::{Receiver, Sender, error::SendError},
    },
    task::JoinSet,
};
use tokio_util::sync::CancellationToken;

use crate::{
    redis_communication::RedisResponse, redis_lib::RedisLibErr, scraper::ScrapeErr,
    serv_engine::ProcessItem,
};

type Scraper<Output> = dyn Fn(reqwest::Client, String) -> Pin<Box<dyn Future<Output = Output> + Send + Sync + 'static>>
    + Send
    + Sync
    + 'static;
type ProcessResult<T> = Result<T, ScrapeProcessErr>;

#[derive(thiserror::Error, Debug)]
pub enum ScrapeProcessErr {
    #[error("{0}")]
    RedisLib(#[from] RedisLibErr),

    #[error("{0}")]
    SendErr(#[from] SendError<ScrapeResultItem>),

    #[error("{0}")]
    SerdeJsonErr(#[from] serde_json::Error),
}

async fn get_response(
    scraper: &Arc<Scraper<Result<String, ScrapeErr>>>,
    item: &ProcessItem,
    http_client: reqwest::Client,
) -> ProcessResult<RedisResponse> {
    Ok(match item.need_request {
        true => match (scraper)(http_client, item.url.clone()).await {
            Ok(payload) => RedisResponse {
                error: None,
                index: item.idx,
                payload: Some(payload),
            },
            Err(e) => RedisResponse {
                error: Some(format!("{e}")),
                payload: None,
                index: item.idx,
            },
        },
        false => RedisResponse {
            error: Some("not forced and ".to_string()),
            payload: None,
            index: item.idx,
        },
    })
}

pub struct ScrapeResultItem {
    pub id: String,
    pub job_id: String,
    pub status_update_url: Option<String>,
    pub send_content: String,
}

// assumed to be used in JoinSet
// in other way needs to have independent lifetime
async fn scrape_process(
    scraper: Arc<Scraper<Result<String, ScrapeErr>>>,
    item: ProcessItem,
    http_client: reqwest::Client,
    scraped_result_tx: Sender<ScrapeResultItem>,
    _guard: OwnedSemaphorePermit,
) -> ProcessResult<()> {
    let resp = get_response(&scraper, &item, http_client).await?;

    scraped_result_tx
        .send(ScrapeResultItem {
            id: item.id,
            job_id: item.job_id,
            status_update_url: if item.need_request {
                Some(item.url)
            } else {
                None
            },
            send_content: serde_json::to_string(&resp)?,
        })
        .await?;

    Ok(())
}

pub async fn invoke_scrape_process(
    set: &mut JoinSet<()>,
    token: CancellationToken,

    semaphore: Arc<Semaphore>,
    mut process_item_rx: Receiver<ProcessItem>,
    scraped_result_tx: Sender<ScrapeResultItem>,

    scraper: Arc<Scraper<Result<String, ScrapeErr>>>,
) -> ProcessResult<()> {
    set.spawn(async move {
        let mut inner_set = JoinSet::new();
        let client = reqwest::Client::new();

        loop {
            tokio::select! {
                item = process_item_rx.recv() => {
                    if let Some(item) = item {
                        let guard = semaphore.clone().acquire_owned().await.unwrap();
                        let move_scraper = scraper.clone();
                        let move_client = client.clone();
                        let moved_tx = scraped_result_tx.clone();

                        inner_set.spawn(async move {
                            if let Err(e) = scrape_process(
                                move_scraper,
                                item,
                                move_client,
                                moved_tx,
                                guard
                            ).await {
                                tracing::error!("{e}");
                            }
                        });
                    }
                },

                _ = token.cancelled() => {
                    while inner_set.join_next().await.is_some() {};
                    break;
                }
            }
        }
    });

    Ok(())
}
