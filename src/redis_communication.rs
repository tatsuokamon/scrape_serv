use serde::Serialize;

pub trait RedisRequest {
    fn get_url(&self) -> String;
    fn get_id(&self) -> String;
    fn get_job_id(&self) -> String;
}

#[derive(serde::Deserialize)]
pub struct BasicRedisReq {
    url: String,
    id: String,
    job_id: String,
}

#[derive(Serialize)]
pub struct RedisResponse {
    pub error: Option<String>,
    pub payload: Option<String>,
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
}
