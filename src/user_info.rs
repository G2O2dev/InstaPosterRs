use frankenstein::{DeleteMessageParams, EditMessageResponse, EditMessageTextParams, Error, Message, MethodResponse, SendMessageParams, TelegramApi};

use crate::{API, State};

#[derive(PartialEq)]
pub enum UserType{
    Admin,
    Normal
}

pub struct UserInfo {
    pub chat_id: i64,
    pub state: State,
    pub user_type: UserType,
}

impl UserInfo {
    pub fn new(chat_id: i64, user_type: UserType) -> Self {
        UserInfo {
            chat_id,
            state: State::Start,
            user_type,
        }
    }

    pub fn edit_msg(&self, msg_id: i32, text: impl Into<String>) -> Result<EditMessageResponse, Error> {
        API.edit_message_text(&EditMessageTextParams::builder().chat_id(self.chat_id).message_id(msg_id).text(text).build())
    }

    pub fn send_msg(&self, text: impl Into<String>) -> Result<MethodResponse<Message>, Error> {
        API.send_message(&SendMessageParams::builder().chat_id(self.chat_id).text(text).build())
    }

    pub fn delete_msg(&self, msg_id: i32) -> Result<MethodResponse<bool>, Error> {
        API.delete_message(&DeleteMessageParams::builder().chat_id(self.chat_id).message_id(msg_id).build())
    }
}