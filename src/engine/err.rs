use crate::{redis_window::RedisWindowErr, scrape_window::ScrapeErr};

#[derive(thiserror::Error, Debug)]
pub enum EngineErr {
    #[error("{0}")]
    ScrapeErr(#[from] ScrapeErr),

    #[error("{0}")]
    RedisErr(#[from] RedisWindowErr),
}
