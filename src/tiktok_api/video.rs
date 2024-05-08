use anyhow::{Context, Result};
use serde_json::Value;

use crate::CLIENT;

pub struct VideoInfo {
    pub(crate) id: String,
    pub video_link: String,
    pub hd: bool,
}

pub async fn get_video_id(url: &str) -> Result<String> {
    let resp = CLIENT.get(url).send().await?;

    let path = resp.url().path();
    let id = path.split('/').last().unwrap();

    Ok(id.to_string())
}

pub async fn get_video_api16(video_url: &str) -> Result<VideoInfo> {
    let id = get_video_id(video_url).await.unwrap();
    let video_link = get_video_url_api16(&id).await.unwrap();
    Ok(VideoInfo {
        id,
        video_link,
        hd: false,
    })
}

pub async fn get_video_url_api16(video_id: &str) -> Result<String> {
    let resp = CLIENT.get(format!("https://api16-normal-c-useast1a.tiktokv.com/aweme/v1/feed/?aweme_id={video_id}")).send().await?;

    let data: Value = serde_json::from_str(&resp.text().await?)?;

    let url = data["aweme_list"][0]["video"]["play_addr"]["url_list"][0].as_str().context("Parsing json err (api16)")?;

    Ok(url.to_string())
}

pub async fn get_video_tiktokvm(video_url: &str) -> Result<VideoInfo> {
    let url = format!("https://www.tikwm.com/api?url={}&count=12&cursor=0&web=1&hd=1", video_url);
    let req = CLIENT.request(reqwest::Method::POST, url).send().await?;

    let json: Value = serde_json::from_str(&req.text().await?)?;
    let data = json.get("data").context("Parsing json err (tiktokvm)")?;

    let id = data["id"].as_str().context("Parsing json err (tiktokvm)")?.to_owned();
    let mut video_link = "https://www.tikwm.com".to_owned();

    let mut is_hd = false;
    if let Some(hd) = data.get("hdplay") {
        video_link.push_str(hd.as_str().context("Parsing json err (tiktokvm)")?);
        is_hd = true;
    } else {
        video_link.push_str(data["play"].as_str().context("Parsing json err (tiktokvm)")?);
    }

    Ok(VideoInfo {
        id,
        video_link,
        hd: is_hd,
    })
}