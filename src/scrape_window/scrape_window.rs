use std::{pin::Pin, sync::Arc};

use reqwest::Client;
use tokio::{
    sync::{
        Semaphore,
        mpsc::{Receiver, Sender},
    },
    task::JoinSet,
};
use tokio_util::sync::CancellationToken;

use crate::{
    redis_communication::{RedisRequest, RedisResponse},
    redis_window::RedisWindowErr,
};

pub struct ScrapeWindow {
    client: Client,
    semaphore: Arc<Semaphore>,
}

fn spawn_scraper<RR, Scraper, JobPusher>(
    client: Client,
    sem: Arc<Semaphore>,
    set: &mut JoinSet<()>,
    token: CancellationToken,
    scraper: Scraper,
    mut from_redis_req_rx: Receiver<String>,
    to_redis_string_tx: Sender<(String, String)>,
    job_pusher: JobPusher,
) where
    RR: RedisRequest + serde::de::DeserializeOwned,
    JobPusher: Fn(String, String) -> Pin<Box<dyn Future<Output = Result<(), RedisWindowErr>> + Send>>
        + 'static
        + Send
        + Sync,
    Scraper: Fn(reqwest::Client, String) -> Pin<Box<dyn Future<Output = RedisResponse> + Send>>
        //                                                              (id,     job_id, result)
        + 'static
        + Send
        + Sync,
{
    let result_tx = to_redis_string_tx;

    set.spawn(async move {
        let scraper = Arc::new(scraper);
        let job_pusher = Arc::new(job_pusher);
        let mut inner_set = JoinSet::new();

        loop {
            tokio::select! {
                received_result = from_redis_req_rx.recv() => {
                    match received_result {
                        Some(received) => {
                            let guard_result = sem.clone().acquire_owned().await;
                            let guard;
                            match guard_result {
                                Ok(g) => {
                                    guard = g;
                                },
                                Err(e) => {
                                    tracing::error!("{}", e);
                                    continue;
                                }
                            }

                            let client = client.clone();
                            let child = token.child_token();
                            let scraper = scraper.clone();
                            let job_pusher = job_pusher.clone();
                            let result_tx = result_tx.clone();
                            let redis_req;
                            match serde_json::from_str::<RR>(&received) {
                                Ok(req) => {
                                    redis_req = req;
                                },
                                Err(e) => {
                                    tracing::error!("{}", e);
                                    continue;
                                }
                            };

                            let id = redis_req.get_id();
                            let url = redis_req.get_url();
                            let job_id = redis_req.get_job_id();
                            let index = redis_req.index();

                            inner_set.spawn(async move {
                                let _guard = guard;
                                tokio::select! {
                                    mut red_response = scraper(client, url) => {
                                        red_response.index = index;
                                        let parsed = serde_json::to_string(&red_response).unwrap();
                                        match result_tx.send((id.clone(), parsed)).await {
                                            Ok(_) => {
                                                if let Err(e) = job_pusher(job_id, id).await {
                                                    tracing::error!("{}", e)
                                                };
                                            },
                                            Err(e) => {
                                                tracing::error!("{}", e);
                                            }
                                        };
                                    },
                                    _ = child.cancelled() => {}
                                };
                            });
                        },
                        None => {
                            break;
                        }
                    }
                },
                _ = token.cancelled() => {
                    break;
                }
            }
        } // loop out

        while inner_set.join_next().await.is_some() {}
    });
}

impl ScrapeWindow {
    pub fn new(semaphore_size: usize) -> Self {
        Self {
            semaphore: Arc::new(Semaphore::new(semaphore_size)),
            client: Client::new(),
        }
    }

    pub fn spawn_scraper<RR, Scraper, JobPusher>(
        &self,
        set: &mut JoinSet<()>,
        token: CancellationToken,
        scraper: Scraper,
        from_redis_req_rx: Receiver<String>,
        to_redis_string_tx: Sender<(String, String)>,
        job_pusher: JobPusher,
    ) where
        RR: RedisRequest + serde::de::DeserializeOwned,
        JobPusher: Fn(String, String) -> Pin<Box<dyn Future<Output = Result<(), RedisWindowErr>> + Send>>
            + 'static
            + Send
            + Sync,
        Scraper: Fn(reqwest::Client, String) -> Pin<Box<dyn Future<Output = RedisResponse> + Send>>
            + 'static
            + Send
            + Sync,
    {
        let client = self.client.clone();
        let sem = self.semaphore.clone();
        spawn_scraper::<RR, Scraper, JobPusher>(
            client,
            sem,
            set,
            token,
            scraper,
            from_redis_req_rx,
            to_redis_string_tx,
            job_pusher,
        );
    }
}
