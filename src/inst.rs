use std::fs::File;
use std::io::Read;
use reqwest::{*};
use rustc_serialize::json::Json;

const app_id: &str = "654706123401012";
const secret: &str = "3851e1e2ee4785bb038d8587c546fe68";
pub const token: &str = "EAAJTc6Xq5zQBO4Y3XHDWs2O9Y10DvnRZBS1YwyKb7vyyzV5YoZBaZBP198uFo2fbCnNkYrUswLDE9U4EikfZB6YJhHPxjXhEQh1nA3R9FoUxRZAjl9kmZBtZABfwVIveqILfLpy2h7GZBDwi28ZCgBvVrrrD1wr7VJ0J5CQcZA6ZCJxe3YsaZABZCu2fAtdTFZA7Jh2EgZB";
const graph_url: &str = "https://graph.facebook.com/v18.0/";


pub fn get_long_lived_access_token(short_token: &str) -> String {
    let client = blocking::Client::new();
    let req = client.post(graph_url.to_owned() + "oauth/access_token")
        .query(&vec![("grant_type", "fb_exchange_token"),
                     ("client_id", app_id),
                     ("client_secret", secret),
                     ("fb_exchange_token", short_token)])
        .send().unwrap();

    let json: Json = Json::from_str(&req.text().unwrap()).unwrap();
    let ll_token = json.find("access_token").unwrap().to_string();
    (&ll_token[1..ll_token.len() - 1]).to_owned()
}

pub fn get_pages() -> Vec<(String, String)> {
    let client = blocking::Client::new();

    let req = client.get(graph_url.to_owned() + "me/accounts")
        .query(&vec![("access_token", token)])
        .send().unwrap();

    let data = Json::from_str(&req.text().unwrap()).unwrap();
    let data = data.find("data").unwrap().as_array().unwrap();
    let mut pages: Vec<(String, String)> = Vec::new();
    for page in data {
        pages.push((page["id"].as_string().unwrap().to_owned(), page["access_token"].as_string().unwrap().to_owned()));
    }

    pages
}

pub fn get_instagram_ids(pages: Vec<String>) -> Vec<String> {
    let client = blocking::Client::new();

    let mut inst_ids: Vec<String> = Vec::new();

    for page in pages {
        let req = client.get(format!("{}{}", graph_url, page))
            .query(&vec![("fields", "instagram_business_account"),
                         ("access_token", token)]).send().unwrap();

        let json: Json = Json::from_str(&req.text().unwrap()).unwrap();
        let id = json["instagram_business_account"]["id"].as_string().unwrap();

        inst_ids.push(id.to_owned());
    }

    inst_ids
}

pub fn upload_reel(page: &str, page_token: &str, description: &str, file_path: &str) {
    let client = blocking::Client::new();

    let req = client.post(format!("{}{}/video_reels", graph_url, page))
        .query(&vec![("access_token", page_token), ("upload_phase", "start")]).send().unwrap();

    let json: Json = Json::from_str(&req.text().unwrap()).unwrap();
    println!("{}", json);
    let video_id = json["video_id"].as_string().unwrap();

    let file = File::open(file_path).unwrap();
    let req = client.post(format!("https://rupload.facebook.com/video-upload/v18.0/{}", video_id))
        .header("Authorization", format!("OAuth {}", page_token).as_str()).header("offset", "0").header("file_size", &file.metadata().unwrap().len().to_string())
        .body(file).send().unwrap();

    println!("{}", req.text().unwrap());

    let req = client.post(format!("{}{}/video_reels", graph_url, page))
        .query(&vec![("fields", "instagram_business_account"),
                     ("access_token", page_token),
                     ("upload_phase", "finish"),
                     ("video_id", video_id),
                     ("video_state", "PUBLISHED"),
                     ("description", description)]).send().unwrap();
    println!("{}", req.text().unwrap());
}

pub fn upload_post() {}

pub fn upload_story() {}