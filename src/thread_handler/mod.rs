use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

pub struct ThreadHandler {
    pub set: JoinSet<()>,
    pub token: CancellationToken,
}

impl ThreadHandler {
    pub async fn stop(self) -> () {
        self.token.cancel();
        self.join().await;
    }

    pub async fn join(mut self) -> () {
        while let Some(res) = self.set.join_next().await {
            if let Err(e) = res {
                tracing::error!("task panic: {}", e);
            }
        }
    }
}

impl Drop for ThreadHandler {
    fn drop(&mut self) {
        self.token.cancel();
    }
}
