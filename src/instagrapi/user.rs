use anyhow::Result;

pub struct  IgUser {
    pub id: String
}


impl IgUser {
    pub fn new(login: &str, password: &str) -> Result<IgUser> {

        Ok(IgUser {
            id: login.to_string()
        })
    }
}