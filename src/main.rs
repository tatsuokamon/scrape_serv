mod engine;
mod redis_communication;
mod redis_window;
mod scrape_window;
mod thread_handler;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let engine = engine::EngineBuilder::new()
        .redis_url("".into())
        .max_size(2)
        .channel_buf(1)
        .semaphore_size(3)
        .build()
        .await
        .expect("failed to build engine");
    let meta_finder = scrape_window::ffi_parser_factory(scrape_window::find_meta);
    let meta_scraper = scrape_window::scraper_factory(meta_finder, 3);
    let handler = engine
        .create_path::<redis_communication::BasicRedisReq>("".into(), "".into(), meta_scraper)
        .await
        .expect("failed to create path");
    handler.join().await;
}
