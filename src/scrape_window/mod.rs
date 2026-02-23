mod err;
mod model;
mod parser_ffi;
mod scrape_window;

pub use err::ScrapeErr;
pub use parser_ffi::{
    ffi_parser_factory, find_detail, find_meta, max_idx_finder, scraper_factory, update_tag,
};
pub use scrape_window::ScrapeWindow;
