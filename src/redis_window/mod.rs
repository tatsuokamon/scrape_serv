mod err;
mod library; // definded used functions
mod redis_window; // redis_window, redis_window_builder defined // defined RedisWindowErr
//
pub use err::RedisWindowErr;
pub use redis_window::{RedisWindow, RedisWindowBuilder};
