use anyhow::*;
use serde_json::Value;
use crate::CLIENT;

pub mod ig;
pub mod fb;

const APP_ID: &str = "654706123401012";
const SECRET: &str = "3851e1e2ee4785bb038d8587c546fe68";
pub const TOKEN: &str = "EAAJTc6Xq5zQBO4Y3XHDWs2O9Y10DvnRZBS1YwyKb7vyyzV5YoZBaZBP198uFo2fbCnNkYrUswLDE9U4EikfZB6YJhHPxjXhEQh1nA3R9FoUxRZAjl9kmZBtZABfwVIveqILfLpy2h7GZBDwi28ZCgBvVrrrD1wr7VJ0J5CQcZA6ZCJxe3YsaZABZCu2fAtdTFZA7Jh2EgZB";
const GRAPH_URL: &str = "https://graph.facebook.com/v19.0/";


pub async fn get_upload_status(id: &str, page_token: &str) -> Result<Value> {
    let req = CLIENT.get(format!("{}{}", GRAPH_URL, id))
            .query(&vec![("fields", "status_code,status"), ("access_token", page_token)]).send().await?;

    let json: Value = serde_json::from_str(&req.text().await?)?;

    Ok(json)
}