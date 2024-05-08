use std::time::Duration;

use anyhow::*;
use reqwest::Response;
use serde_json::Value;

use crate::{CLIENT};
use crate::contol_panel::UserControl;
use crate::graph_api::{get_upload_status, GRAPH_URL, TOKEN};
use crate::user_info::{UserInfo, UserType};

pub async fn upload_reel(ig_id: &str, description: &str, video_url: &str, uc: &UserControl) -> Result<()> {
    let resp = CLIENT.post(format!("{}{}/media", GRAPH_URL, ig_id))
        .query(&vec![("media_type", "REELS"), ("share_to_feed", "true"), ("video_url", video_url), ("caption", description), ("access_token", TOKEN)]).send().await?;

    finish_uploading(resp, ig_id, uc).await
}

pub async fn upload_story(ig_id: &str, video_url: &str, uc: &UserControl) -> Result<()> {
    let resp = CLIENT.post(format!("{}{}/media", GRAPH_URL, ig_id))
        .query(&vec![("media_type", "STORIES"), ("video_url", video_url), ("access_token", TOKEN)]).send().await?;

    finish_uploading(resp, ig_id, uc).await
}

async fn finish_uploading(resp: Response, ig_id: &str, err_receiver: &UserControl) -> Result<()> {
    let data: Value = serde_json::from_str(&resp.text().await?)?;
    let id = data["id"].as_str().context("Upload story, error getting Id")?;

    let mut resp: Value = Value::Null;
    let mut status = "IN_PROGRESS";
    while status == "IN_PROGRESS" {
        tokio::time::sleep(Duration::from_secs(12)).await;
        resp = get_upload_status(id, TOKEN).await?;
        status = resp["status_code"].as_str().context(format!("Bad status code: {resp}"))?;
    }

    if resp["status_code"].as_str().unwrap() == "ERROR" {
        err_receiver.send_msg(&format!("Произошла ошибка при загрузке контейнера: {resp}"), UserType::Admin);
    }

    CLIENT.post(format!("{}{}/media_publish", GRAPH_URL, ig_id)).query(&vec![("creation_id", id), ("access_token", TOKEN)]).send().await?;

    Ok(())
}