#[derive(serde::Serialize, serde::Deserialize)]
pub struct TagSrc {
    name: String,
    url: String,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct TimeTableSrc {
    index: i32,
    title: String,
    time: String,
}

// Serialized this struct will be returned by find_meta
#[derive(serde::Serialize, serde::Deserialize)]
pub struct MetaSrc {
    id: String,
    title: String,
    url: String,
    img_src: Option<String>,
    time: i64,

    cv: Vec<TagSrc>,
    genre: Vec<TagSrc>,
    illust: Vec<TagSrc>,
    circle: Vec<TagSrc>,
    series: Vec<TagSrc>,
    time_table: Vec<TimeTableSrc>,
}

// Vec<Tag> will be returned by update_tag
// Vec<String> will be returned by find_detail
