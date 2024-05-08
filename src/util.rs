use std::path::Path;
use std::sync::atomic::Ordering;
use crate::{CLIENT, DB, PostType};
use anyhow::Result;
use reqwest::Body;
use tokio_util::codec::{BytesCodec, FramedRead};

pub async fn upload_file(path: impl AsRef<Path>, name: &str) -> Result<String> {
    let content_file = tokio::fs::File::open(path).await?;
    let mut content_url = CLIENT
        .put(format!("https://transfer.sh/{}", name))
        .header("Max-Days", "1")
        .body(file_to_body(content_file))
        .send().await?
        .text().await?;
    content_url.insert_str(20, "get/");

    Ok(content_url)
}

pub async  fn get_post_count(post_type: PostType) -> u32 {
    DB.lock().await.query_row(&format!("SELECT Count(*) FROM {post_type}"), [], |r| r.get(0)).unwrap()
}

pub fn file_to_body(file: tokio::fs::File) -> Body {
    let stream = FramedRead::new(file, BytesCodec::new());
    let body = Body::wrap_stream(stream);
    body
}