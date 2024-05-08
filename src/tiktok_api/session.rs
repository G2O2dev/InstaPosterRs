struct TiktokClinet {
    client: reqwest::Client,
}

impl TiktokClinet {
    pub fn new() -> Self {
        let client = reqwest::Client::new();
        let resp = client.get("https://vt.tiktok.com/ZSFF5xXM4");
        Self {
            client: reqwest::Client::new(),
        }
    }
}