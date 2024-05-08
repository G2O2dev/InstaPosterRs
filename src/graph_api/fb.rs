use std::fs::File;
use std::io::Read;
use std::time::Duration;

use anyhow::{Context, Result};
use serde_json;
use serde_json::Value;

use crate::CLIENT;
use crate::graph_api::*;

pub enum Src<'a> {
    Url(&'a str),
    File(&'a File),
}

pub async fn get_long_lived_access_token(short_token: &str) -> Result<String> {
    let req = CLIENT.post(GRAPH_URL.to_owned() + "oauth/access_token")
        .query(&vec![("grant_type", "fb_exchange_token"),
                     ("client_id", APP_ID),
                     ("client_secret", SECRET),
                     ("fb_exchange_token", short_token)])
        .send().await?;

    let json: Value = serde_json::from_str(&req.text().await?)?;
    let ll_token = json["access_token"].as_str().unwrap();

    Ok(ll_token.to_owned())
}
pub async fn get_pages() -> Result<Vec<(String, String)>> {
    let req = CLIENT.get(GRAPH_URL.to_owned() + "me/accounts")
        .query(&vec![("access_token", TOKEN)]).send().await.unwrap();

    let data: Value = serde_json::from_str(&req.text().await?)?;
    let data = data["data"].as_array().context(format!("Error getting pages: {}", data))?;
    let mut pages: Vec<(String, String)> = Vec::new();
    for page in data {
        pages.push((page["id"].as_str().unwrap().to_owned(), page["access_token"].as_str().unwrap().to_owned()));
    }

    Ok(pages)
}
pub async fn get_video_src(video_id: &str, page_token: &str) -> Result<String> {
    let req = CLIENT.get(format!("{}{}", GRAPH_URL, video_id)).query(&vec![
        ("fields", "status,source"),
        ("access_token", page_token),
    ]);

    let mut status: &str = "";
    let mut data: Value = Value::Null;
    while status != "ready" {
        tokio::time::sleep(Duration::from_secs(15)).await;
        data = serde_json::from_str(&req.try_clone().unwrap().send().await?.text().await?)?;
        status = data["status"]["video_status"].as_str().unwrap();
    }

    Ok(data["source"].as_str().unwrap().to_owned())
}
pub async fn get_ig_account(page_id: &str) -> Result<String> {
    let req = CLIENT.get(format!("{}{}", GRAPH_URL, page_id))
        .query(&vec![("fields", "instagram_business_account"), ("access_token", TOKEN)]).send().await?;

    let json: Value = serde_json::from_str(&req.text().await?)?;
    let id = json["instagram_business_account"]["id"].as_str().unwrap();

    Ok(id.to_owned())
}

pub async fn upload_reel(page_id: &str, page_token: &str, description: &str, src: Src<'_>) -> Result<String> {
    let resp = CLIENT.post(format!("{}{}/video_reels", GRAPH_URL, page_id))
        .query(&vec![("upload_phase", "start"), ("access_token", page_token)]).send().await?;

    let json: Value = serde_json::from_str(&resp.text().await?)?;
    let video_id = json["video_id"].as_str().unwrap();

    upload_src(src, page_token, video_id).await?;

    CLIENT.post(format!("{}{}/video_reels", GRAPH_URL, page_id))
        .query(&vec![("access_token", page_token),
                     ("upload_phase", "finish"),
                     ("video_id", video_id),
                     ("video_state", "PUBLISHED"),
                     ("description", description)]).send().await?;


    Ok(video_id.to_string())
}
pub async fn upload_story(page_id: &str, page_token: &str, src: Src<'_>) -> Result<String> {
    let resp = CLIENT.post(format!("{}{}/video_stories", GRAPH_URL, page_id))
        .query(&vec![("upload_phase", "start"), ("access_token", page_token)]).send().await?;

    let json: Value = serde_json::from_str(&resp.text().await?)?;
    let video_id = json["video_id"].as_str().unwrap();

    upload_src(src, page_token, video_id).await?;

    CLIENT.post(format!("{}{}/video_stories", GRAPH_URL, page_id))
        .query(&vec![
            ("access_token", page_token),
            ("upload_phase", "finish"),
            ("video_id", video_id),
        ]).send().await?;


    Ok(video_id.to_string())
}
async fn upload_src(src: Src<'_>, page_token: &str, video_id: &str) -> Result<()> {
    let req = CLIENT.post(format!("https://rupload.facebook.com/video-upload/v19.0/{}", video_id)).header("Authorization", format!("OAuth {}", page_token).as_str());
    if let Src::Url(url) = src {
        req.header("file_url", url).send().await?;
    } else if let Src::File(file) = src {
        req.header("file_size", file.metadata()?.len().to_string()).body(file.bytes().collect::<Result<Vec<u8>, _>>()?).send().await?;
    }

    Ok(())
}