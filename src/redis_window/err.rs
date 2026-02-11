#[derive(thiserror::Error, Debug)]
pub enum RedisWindowErr {
    #[error("redis_window_err: redis_err {0}")]
    RedisError(#[from] redis::RedisError),

    #[error("redis_window_err: redis_err {0}")]
    RunErr(#[from] bb8::RunError<redis::RedisError>),
}
