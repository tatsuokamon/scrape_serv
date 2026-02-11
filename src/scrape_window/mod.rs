mod err;
mod parser_ffi;
mod scrape_window;

pub use err::ScrapeErr;
pub use parser_ffi::{
    ffi_parser_factory, find_detail, find_max_idx, find_meta, scraper_factory, update_tag,
};
pub use scrape_window::ScrapeWindow;
