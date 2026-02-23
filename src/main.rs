mod engine;
mod redis_communication;
mod redis_window;
mod scrape_window;
mod thread_handler;

macro_rules! get_env {
    ($operand:ident, $keyword:expr) => {
        let $operand = std::env::var($keyword).expect(&format!("failed to load: {}", $keyword));
    };
}
macro_rules! get_env_with_parsing {
    ($operand:ident, $keyword:expr, $dest:ty) => {
        let $operand = std::env::var($keyword).expect(&format!("failed to load: {}", $keyword)).parse::<$dest>().expect(&format!("failed to parse {}", $keyword));
    };
}
#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    dotenv::dotenv().expect("failied dotenv::dotenv");

    get_env!(RedisURL, "REDIS_URL");
    get_env_with_parsing!(RequestRetry, "REQUEST_RETRY", i32);
    get_env_with_parsing!(PoolSize, "REQUEST_RETRY", u32);
    get_env_with_parsing!(ChannelBuf, "CHANNEL_BUF", usize);
    get_env_with_parsing!(SemaphoreSize, "SEMAPHORE_SIZE", usize);
    get_env!(MetaReqQKeyword, "META_REQUEST_Q_KEYWORD");
    get_env!(MetaResultHashKeyWord, "META_RESULT_HASH_KEYWORD");
    get_env!(DetailReqQKeyword, "DETAIL_REQUEST_Q_KEYWORD");
    get_env!(DetailResultHashKeyword, "DETAIL_RESULT_HASH_KEYWORD");
    get_env!(TagReqQKeyword, "DETAIL_REQUEST_Q_KEYWORD");
    get_env!(TagResultHashKeyword, "DETAIL_RESULT_HASH_KEYWORD");
    get_env!(MaxIdxReqQKeyword, "DETAIL_REQUEST_Q_KEYWORD");
    get_env!(MaxIdxResultHashKeyword, "DETAIL_RESULT_HASH_KEYWORD");

    let engine = engine::EngineBuilder::new()
        .redis_url(RedisURL)
        .max_size(PoolSize)
        .channel_buf(ChannelBuf)
        .semaphore_size(SemaphoreSize)
        .build()
        .await
        .expect("failed to build engine");

    let meta_finder = scrape_window::ffi_parser_factory(scrape_window::find_meta);
    let meta_scraper = scrape_window::scraper_factory(meta_finder, RequestRetry);

    let detail_finder = scrape_window::ffi_parser_factory(scrape_window::find_detail);
    let detail_scraper = scrape_window::scraper_factory(detail_finder, RequestRetry);

    let tag_updater = scrape_window::ffi_parser_factory(scrape_window::update_tag);
    let tag_scraper = scrape_window::scraper_factory(tag_updater, RequestRetry);

    let max_idx_scraper =
        scrape_window::scraper_factory(scrape_window::max_idx_finder, RequestRetry);

    let meta_handler = engine
        .create_path::<redis_communication::BasicRedisReq>(
            MetaReqQKeyword,
            MetaResultHashKeyWord,
            meta_scraper,
        )
        .await
        .expect("failed to create path: meta_scraper");
    let detail_handler = engine
        .create_path::<redis_communication::BasicRedisReq>(
            DetailReqQKeyword,
            DetailResultHashKeyword,
            detail_scraper,
        )
        .await
        .expect("failed to create path: detail_scraper");
    let tag_updater_handler = engine
        .create_path::<redis_communication::BasicRedisReq>(
            TagReqQKeyword,
            TagResultHashKeyword,
            tag_scraper,
        )
        .await
        .expect("failed to create path: tag_updater");
    let max_idx_finder_handler = engine
        .create_path::<redis_communication::BasicRedisReq>(
            MaxIdxReqQKeyword,
            MaxIdxResultHashKeyword,
            max_idx_scraper,
        )
        .await
        .expect("failed to create path: max_idx_finder");

    meta_handler.join().await;
    detail_handler.join().await;
    tag_updater_handler.join().await;
    max_idx_finder_handler.join().await;
}
