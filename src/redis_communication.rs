use serde::Serialize;

pub trait RedisRequest {
    fn get_url(&self) -> String;
    fn get_id(&self) -> String;
    fn get_job_id(&self) -> String;
    fn index(&self) -> i32;
    fn is_forced(&self) -> bool;
}

#[derive(serde::Deserialize)]
pub struct BasicRedisReq {
    url: String,
    id: String,
    job_id: String,
    index: i32,
    force: Option<bool>,
}

#[derive(Serialize)]
pub struct RedisResponse {
    pub error: Option<String>,
    pub payload: Option<String>,
    pub index: i32,
}

impl RedisRequest for BasicRedisReq {
    fn get_url(&self) -> String {
        self.url.clone()
    }
    fn get_id(&self) -> String {
        self.id.clone()
    }
    fn get_job_id(&self) -> String {
        self.job_id.clone()
    }

    fn index(&self) -> i32 {
        self.index
    }

    fn is_forced(&self) -> bool {
        self.force.unwrap_or(false)
    }
}
