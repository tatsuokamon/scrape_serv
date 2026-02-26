use std::{pin::Pin, sync::Arc};
use tokio_util::io::simplex::new;

use crate::{parser::ParserErr, redis_communication::RedisResponse};

type Scraper<Output> = dyn Fn(reqwest::Client, String) -> Pin<Box<dyn Future<Output = Output> + Send + Sync + 'static>>
    + Send
    + Sync
    + 'static;

#[derive(thiserror::Error, Debug)]
pub enum ScrapeErr {
    #[error("{0}")]
    ParserErr(#[from] ParserErr),

    #[error("{0}")]
    ReqwestErr(#[from] reqwest::Error),

    #[error("")]
    OverRetry,
}

pub fn generate_scraper(
    parser: impl Fn(&str) -> Result<String, ParserErr> + Send + Sync + 'static,
    retry: i32,
) -> Arc<Scraper<Result<String, ScrapeErr>>> {
    let parser = Arc::new(parser);

    Arc::new(move |client: reqwest::Client, url: String| {
        let moved_parser = parser.clone();

        Box::pin(async move {
            let mut tempt = 0;
            while tempt < retry {
                let req = client.get(&url);
                match req.send().await {
                    Ok(result) => {
                        let text = result.text().await.unwrap();
                        return Ok(moved_parser(&text)?);
                    }
                    Err(e) => {
                        tracing::error!("{e}");
                        tempt += 1;
                    }
                }
            }
            Err(ScrapeErr::OverRetry)
        })
    })
}
