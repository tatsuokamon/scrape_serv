use std::sync::Arc;

use bb8::{Pool, PooledConnection};
use bb8_redis::RedisConnectionManager;
use tokio::{sync::mpsc::Receiver, task::JoinSet};
use tokio_util::sync::CancellationToken;

use crate::{
    redis_lib::{
        AcquireConfigTrait, PoolAcquireConfig, RedisLibErr, push_result, update_job_status,
        update_recently_got,
    },
    serv_engine::{create_identifier, scrape_process::ScrapeResultItem},
};

#[derive(thiserror::Error, Debug)]
pub enum PostProcessErr {
    #[error("{0}")]
    RedisLib(#[from] RedisLibErr),
}

type ProcessResult<T> = Result<T, PostProcessErr>;

async fn post_process_inner(
    conn: &mut PooledConnection<'_, RedisConnectionManager>,
    scraped_result: ScrapeResultItem,
    storage_time: usize,
    result_keyword: &str,
) -> ProcessResult<()> {
    let url_op = scraped_result.status_update_url;
    if let Some(url) = url_op {
        push_result(
            result_keyword,
            &scraped_result.id,
            &scraped_result.send_content,
            conn,
        )
        .await?;
        update_job_status(&scraped_result.job_id, &scraped_result.id, conn).await?;
        let identifier = create_identifier(&url);
        update_recently_got(&identifier, conn, storage_time).await?;
    };

    Ok::<_, PostProcessErr>(())
}

pub async fn invoke_post_process(
    set: &mut JoinSet<()>,
    token: CancellationToken,

    pool: Arc<Pool<RedisConnectionManager>>,
    pool_config: Arc<PoolAcquireConfig>,
    mut scraped_result_rx: Receiver<ScrapeResultItem>,

    result_keyword: String,
    storage_time: usize,
) -> ProcessResult<()> {
    set.spawn(async move {
        tokio::select! {
            _ = async move {
            } => {
                let mut conn = pool_config.acquire_anyway(&pool).await;
                while let Some(item) = scraped_result_rx.recv().await {
                    if let Err (e)  = post_process_inner(&mut conn, item, storage_time, &result_keyword).await {
                        tracing::error!("{e}");
                        conn = pool_config.acquire_anyway(&pool).await;
                    }
                }
            },
            _ = token.cancelled() => {}
        }
    });

    Ok(())
}
