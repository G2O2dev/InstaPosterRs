use crate::user_info::{UserInfo, UserType};
use crate::user_info::UserType::Normal;

pub struct UserControl{
    users: Vec<UserInfo>
}

impl UserControl {
    pub fn new(users: Vec<UserInfo>) -> UserControl {
        UserControl {
            users
        }
    }

    pub fn try_add_user(&mut self, chat_id: i64) -> &mut UserInfo {
        if let Some(pos) = self.users.iter().position(|user| user.chat_id == chat_id) {
            return self.users.get_mut(pos).unwrap()
        } else {
            self.users.push(UserInfo::new(chat_id, Normal));
            let index = self.users.len() - 1;
            return self.users.get_mut(index).unwrap()
        }
    }

    pub fn get_user(&self, chat_id: i64) -> Option<&UserInfo> {
        if let Some(pos) = self.users.iter().position(|user| user.chat_id == chat_id) {
            return Some(self.users.get(pos).unwrap())
        }

        None
    }
    pub fn get_user_mut(&mut self, chat_id: i64) -> Option<&mut UserInfo> {
        if let Some(pos) = self.users.iter().position(|user| user.chat_id == chat_id) {
            return Some(self.users.get_mut(pos).unwrap())
        }

        None
    }

    pub fn add_user(&mut self, user: UserInfo){
        self.users.push(user);
    }
    pub fn send_msg(&self, msg: &str, filter: UserType) {
        for user in &self.users {
            if user.user_type == filter {
                user.send_msg(msg);
            }
        }
    }
}