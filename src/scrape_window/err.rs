use std::ffi::NulError;

#[derive(thiserror::Error, Debug)]
pub enum ScrapeErr {
    #[error("{0}")]
    NulError(#[from] NulError),

    #[error("NonNulIsNoneErr")]
    NonNulIsNoneErr,

    #[error("ReqwestErr: {0}")]
    ReqwetErr(#[from] reqwest::Error),

    #[error("RequestErr LoopOut")]
    LoopOutErr,
}
