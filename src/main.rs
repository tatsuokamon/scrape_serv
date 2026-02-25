use crate::redis_window::FactoryConfig;

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
        let $operand = std::env::var($keyword)
            .expect(&format!("failed to load: {}", $keyword))
            .parse::<$dest>()
            .expect(&format!("failed to parse {}", $keyword));
    };
}
#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    dotenv::dotenv().expect("failied dotenv::dotenv");

    get_env!(redis_url, "REDIS_URL");
    get_env_with_parsing!(request_retry, "PARSER_REQUEST_RETRY", i32);
    get_env_with_parsing!(pool_size, "POOL_SIZE", u32);
    get_env_with_parsing!(request_channel_buffer, "REQUEST_CHANNEL_BUF", usize);
    get_env_with_parsing!(result_channel_buffer, "RESULT_CHANNEL_BUF", usize);
    get_env_with_parsing!(blocking_time, "BLOCKING_TIME", f64);

    get_env_with_parsing!(semaphore_size, "SEMAPHORE_SIZE", usize);
    get_env!(meta_request_q_keyword, "META_REQUEST_Q_KEYWORD");
    get_env!(detail_request_q_keyword, "DETAIL_REQUEST_Q_KEYWORD");
    get_env!(tag_request_q_keyword, "TAG_REQUEST_Q_KEYWORD");
    get_env!(max_idx_request_q_keyword, "IDX_REQUEST_Q_KEYWORD");
    get_env!(result_keyword, "RESULT_KEYWORD");

    let factory_config = FactoryConfig {
        req_channel_buf: request_channel_buffer,
        result_channel_buf: result_channel_buffer,
        blocking_time
    };

    let engine = engine::EngineBuilder::new()
        .redis_url(redis_url)
        .max_size(pool_size)
        .semaphore_size(semaphore_size)
        .build()
        .await
        .expect("failed to build engine");

    let meta_finder = scrape_window::ffi_parser_factory(scrape_window::find_meta);
    let meta_scraper = scrape_window::scraper_factory(meta_finder, request_retry);

    let detail_finder = scrape_window::ffi_parser_factory(scrape_window::find_detail);
    let detail_scraper = scrape_window::scraper_factory(detail_finder, request_retry);

    let tag_updater = scrape_window::ffi_parser_factory(scrape_window::update_tag);
    let tag_scraper = scrape_window::scraper_factory(tag_updater, request_retry);

    let max_idx_scraper =
        scrape_window::scraper_factory(scrape_window::max_idx_finder, request_retry);

    let meta_handler = engine
        .create_path::<redis_communication::BasicRedisReq>(
            meta_request_q_keyword,
            result_keyword.clone(),
            &factory_config,
            meta_scraper,
        )
        .await
        .expect("failed to create path: meta_scraper");
    let detail_handler = engine
        .create_path::<redis_communication::BasicRedisReq>(
            detail_request_q_keyword,
            result_keyword.clone(),
            &factory_config,
            detail_scraper,
        )
        .await
        .expect("failed to create path: detail_scraper");
    let tag_updater_handler = engine
        .create_path::<redis_communication::BasicRedisReq>(
            tag_request_q_keyword,
            result_keyword.clone(),
            &factory_config,
            tag_scraper,
        )
        .await
        .expect("failed to create path: tag_updater");
    let max_idx_finder_handler = engine
        .create_path::<redis_communication::BasicRedisReq>(
            max_idx_request_q_keyword,
            result_keyword.clone(),
            &factory_config,
            max_idx_scraper,
        )
        .await
        .expect("failed to create path: max_idx_finder");

    meta_handler.join().await;
    detail_handler.join().await;
    tag_updater_handler.join().await;
    max_idx_finder_handler.join().await;
}
