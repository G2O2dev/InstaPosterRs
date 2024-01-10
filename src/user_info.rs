use std::cell::{Ref, RefCell};
use crate::State;

pub struct UserInfo {
    pub chat_id: i64,
    pub state: State,
}

impl UserInfo {
    pub fn new(chat_id: i64) -> Self {
        UserInfo {
            chat_id,
            state: State::Start,
        }
    }

    pub fn set_state(&self, state: State) {
        unsafe {
            let const_ptr = &self.state as *const State;
            let mut_ptr = const_ptr as *mut State;
            *&mut *mut_ptr = state;
        }
    }
}