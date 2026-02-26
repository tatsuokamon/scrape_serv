use std::{sync::Arc, time::Duration};

use bb8::Pool;
use bb8_redis::RedisConnectionManager;
use redis::AsyncConnectionConfig;

use crate::{
    parser::{ffi_parser_factory, find_detail, find_meta, max_idx_finder, update_tag},
    redis_communication::BasicRedisReq,
    redis_lib::{ClientAcquireConfig, PoolAcquireConfig, ReqFetchContract},
    scraper::generate_scraper,
    serv_engine::{ProcessReqContract, create_path},
};

mod parser;
mod redis_communication;
mod redis_lib;
mod scraper;
mod serv_engine;
mod thread_handler;

macro_rules! get_env {
    ($keyword:expr) => {
        std::env::var($keyword).expect(&format!("failed to load: {}", $keyword))
    };
}
macro_rules! get_env_with_parsing {
    ($keyword:expr, $dest:ty) => {
        std::env::var($keyword)
            .expect(&format!("failed to load: {}", $keyword))
            .parse::<$dest>()
            .expect(&format!("failed to parse {}", $keyword))
    };
}

fn temp_backoff_next(current: Duration) -> Duration {
    current.mul_f64(1.5)
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    dotenv::dotenv().expect("failied dotenv::dotenv");
    // create request fetch cont;ract

    let channel_buf = get_env_with_parsing!("CHANNEL_BUF", usize);
    let blocking_time = get_env_with_parsing!("BLOCKING_TIME", f64);
    macro_rules! req_fetch_contract_create_macro {
        ($req_q_keyword:expr) => {
            ReqFetchContract {
                channel_buf,
                blocking_time,
                req_q_keyword: $req_q_keyword,
            }
        };
    }
    let req_fetch_contract_for_meta =
        req_fetch_contract_create_macro!(get_env!("META_REQUEST_Q_KEYWORD"));
    let req_fetch_contract_for_detail =
        req_fetch_contract_create_macro!(get_env!("DETAIL_REQUEST_Q_KEYWORD"));
    let req_fetch_contract_for_tag =
        req_fetch_contract_create_macro!(get_env!("TAG_REQUEST_Q_KEYWORD"));
    let req_fetch_contract_for_idx =
        req_fetch_contract_create_macro!(get_env!("IDX_REQUEST_Q_KEYWORD"));

    // ready parser
    let retry = get_env_with_parsing!("NET_REQUEST_RETRY", i32);
    let meta_scraper = generate_scraper(ffi_parser_factory(find_meta), retry);
    let detail_scraper = generate_scraper(ffi_parser_factory(find_detail), retry);
    let tag_update_scraper = generate_scraper(ffi_parser_factory(update_tag), retry);
    let idx_scraper = generate_scraper(max_idx_finder, retry);

    // ready process req contract
    let result_keyword = get_env!("RESULT_KEYWORD");
    let storage_time = get_env_with_parsing!("STORAGE_TIME", usize);

    macro_rules! generate_req_contract {
        ($scraper:expr) => {
            ProcessReqContract {
                result_keyword: result_keyword.clone(),
                storage_time,
                scraper: $scraper,
            }
        };
    }

    let process_contract_for_meta = generate_req_contract!(meta_scraper);
    let process_contract_for_detail = generate_req_contract!(detail_scraper);
    let process_contract_for_tag = generate_req_contract!(tag_update_scraper);
    let process_contract_for_idx = generate_req_contract!(idx_scraper);

    let redis_client =
        Arc::new(redis::Client::open(get_env!("REDIS_URL")).expect("failed to open redis client"));
    let redis_client_config = Arc::new(ClientAcquireConfig {
        async_config: AsyncConnectionConfig::new().set_response_timeout(None),
        init_backoff: Duration::from_secs(get_env_with_parsing!("INIT_BACKOFF", u64)),
        backoff_next: Arc::new(temp_backoff_next),
    });

    let manager =
        RedisConnectionManager::new(get_env!("REDIS_URL")).expect("failed to create redis manager");
    let pool = Arc::new(
        Pool::builder()
            .max_size(get_env_with_parsing!("MAX_POOL_SIZE", u32))
            .connection_timeout(Duration::from_secs(get_env_with_parsing!(
                "CENNECTION_TIMEOUT",
                u64
            )))
            .build(manager)
            .await
            .expect("failed build pool"),
    );

    let pool_config = Arc::new(PoolAcquireConfig {
        init_backoff: Duration::from_secs(get_env_with_parsing!("INIT_BACKOFF", u64)),
        backoff_next: Arc::new(temp_backoff_next),
    });

    let net_client = reqwest::Client::new();
    macro_rules! create_path_macro {
        ($req_fetch_contract:expr, $process_req_contract:expr) => {
            serv_engine::create_path::<BasicRedisReq>(
                redis_client.clone(),
                redis_client_config.clone(),
                $req_fetch_contract,
                pool.clone(),
                pool_config.clone(),
                net_client.clone(),
                $process_req_contract,
            )
            .await
        };
    }

    let meta_handler = create_path_macro!(req_fetch_contract_for_meta, process_contract_for_meta)
        .expect("failed create path : meta");
    let detail_handler =
        create_path_macro!(req_fetch_contract_for_detail, process_contract_for_detail)
            .expect("failed create path : detail");
    let tag_updater_handler =
        create_path_macro!(req_fetch_contract_for_tag, process_contract_for_tag)
            .expect("failed create path : tag");
    let idx_handler = create_path_macro!(req_fetch_contract_for_idx, process_contract_for_idx)
        .expect("failed create path : idx");

    meta_handler.join().await;
    detail_handler.join().await;
    tag_updater_handler.join().await;
    idx_handler.join().await;
}
