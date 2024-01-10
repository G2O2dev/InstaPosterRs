use std::*;
use std::cell::{RefCell, RefMut};
use std::fmt::Display;
use std::io::Read;
use std::ops::Add;
use std::rc::Rc;
use std::str::FromStr;
use std::sync::atomic::{AtomicU32, Ordering};

use anyhow::anyhow;
use anyhow::Result;
use chrono::*;
use frankenstein::*;
use frankenstein::AllowedUpdate::*;
use reqwest::blocking::Client;
use rusqlite::*;
use rustc_serialize::json::Json;

use crate::user_info::UserInfo;
use crate::VideoWrapper::{Telegram, TikTok};

mod inst;
// mod user_info;

//#region Fields
const token: &str = "6814456031:AAEubA5vlBdbbUAW35sOGh-YymjWszdq9Sk";

const story_schedule: [u32; 7] = [8, 10, 10, 5, 20, 20, 20];
const reels_schedule: [u32; 7] = [4, 5, 3, 5, 6, 6, 6];
const post_schedule: [u32; 7] = [4, 4, 4, 4, 4, 4, 4];

static mut reels_count: AtomicU32 = AtomicU32::new(0);
static mut story_count: AtomicU32 = AtomicU32::new(0);
static mut post_count: AtomicU32 = AtomicU32::new(0);


#[derive(PartialEq)]
pub enum State {
    Start,
    ReelsWaiting,
    StoryWaiting,
    PostWaiting,
    ChoiceWaiting(i32),
    Something(VideoWrapper, i32),
}

#[derive(PartialEq)]
enum VideoWrapper {
    Telegram(Box<Video>),
    TikTok(String),
}

#[derive(Debug)]
enum PostType {
    Reels,
    Story,
    Post,
}

impl fmt::Display for PostType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

//#endregion

fn main() {
    unsafe {
        let api = Api::new(token);

        let mut update_params = GetUpdatesParams::builder().allowed_updates(vec![AllowedUpdate::Message, AllowedUpdate::CallbackQuery]).build();

        let post_db = Connection::open("E:\\Project\\InstaPoster\\Posts.db").expect("Can't connect to database");

        // let pages = get_pages();

        // upload_reel(&pages[0].0, &pages[0].1, "тест", "C:\\reels\\1.mp4");


        reels_count.store(post_db.query_row("SELECT Count(*) FROM Reels", [], |r| r.get(0)).unwrap(), Ordering::SeqCst);
        story_count.store(post_db.query_row("SELECT Count(*) FROM Story", [], |r| r.get(0)).unwrap(), Ordering::SeqCst);
        post_count.store(post_db.query_row("SELECT Count(*) FROM Post", [], |r| r.get(0)).unwrap(), Ordering::SeqCst);

        let mut users: Vec<UserInfo> = Vec::new();

        loop {
            let result = api.get_updates(&update_params);
            // println!("{:#?}", result);

            match result {
                Ok(resp) => {
                    for update in resp.result {
                        if let UpdateContent::Message(message) = update.content {
                            handle_message(message, &mut users, &api, &post_db).unwrap();
                            update_params.offset = Some(update.update_id as i64 + 1);
                        } else if let UpdateContent::CallbackQuery(callback) = update.content {
                            if let Err(_) = api.answer_callback_query(&AnswerCallbackQueryParams::builder().callback_query_id(&callback.id).build()) {
                                continue;
                            }

                            handle_callback(callback, &mut users, &api, &post_db).unwrap();
                            update_params.offset = Some(update.update_id as i64 + 1);
                        }
                    }
                }

                Err(e) => {
                    println!("{:#?}", e);
                }
            }

            if false {
                let date = get_closest_reels_date(&post_db);
            }
        }
    }
}

fn get_user_index(chat_id: i64, users: &mut Vec<UserInfo>) -> usize {
    return if let Some(u) = users.iter().position(|user| user.chat_id == chat_id) {
        u
    } else {
        users.push(UserInfo::new(chat_id));
        users.len() - 1
    };
}

fn handle_message(msg: frankenstein::Message, users: &mut Vec<UserInfo>, api: &Api, post_db: &Connection) -> Result<()> {
    let chat_id = ChatId::Integer(msg.chat.id);
    let index = get_user_index(msg.chat.id, users);
    let mut user = &users[index];

    let send_msg = |txt: &str| -> MethodResponse<frankenstein::Message> {
        let msg = SendMessageParams::builder().chat_id(chat_id.clone()).text(txt).reply_markup(make_keyboard(vec!(("Рилс", "Reels"), ("История", "Story"), ("Отмена", "Cancel")))).build();
        api.send_message(&msg).expect("error sending message")
    };

    if let Some(video) = msg.video {
        match user.state {
            State::ReelsWaiting => {
                add_video(&Telegram(video), &user, PostType::Story, &post_db, &api).unwrap();

                user.set_state(State::Start);
            }
            State::StoryWaiting => {
                add_video(&Telegram(video), &user, PostType::Story, &post_db, &api).unwrap();

                user.set_state(State::Start);
            }
            State::PostWaiting => {}
            _ => {
                if let State::Something(_, msg_id) = user.state {
                    api.delete_message(&DeleteMessageParams::builder().chat_id(msg.chat.id).message_id(msg_id.clone()).build());
                }
                let resp = send_msg("Что это?");

                user.set_state(State::Something(Telegram(video), resp.result.message_id));
            }
        }
    } else if let Some(text) = msg.text {
        let valid_link = text.contains("tiktok.com");

        match user.state {
            State::Start => {
                if valid_link {
                    let resp = send_msg("Что это?");

                    user.set_state(State::Something(TikTok(text), resp.result.message_id));
                } else {
                    let resp = send_msg("Что ты хочешь добавить?");

                    user.set_state(State::ChoiceWaiting(resp.result.message_id));
                }
            }
            State::ReelsWaiting => {
                if valid_link {
                    add_video(&TikTok(text), &user, PostType::Story, &post_db, &api).unwrap();
                }
            }
            State::StoryWaiting => {
                if valid_link {
                    add_video(&TikTok(text), &user, PostType::Story, &post_db, &api);
                }
            }
            State::PostWaiting => {}
            State::Something(_, msg_id) => {
                if valid_link {
                    api.delete_message(&DeleteMessageParams::builder().chat_id(msg.chat.id).message_id(msg_id).build());

                    let resp = send_msg("Что это?");

                    user.set_state(State::Something(TikTok(text.clone()), resp.result.message_id));
                }
            }
            State::ChoiceWaiting(msg_id) => {
                if valid_link {
                    api.delete_message(&DeleteMessageParams::builder().chat_id(msg.chat.id).message_id(msg_id).build());

                    let resp = send_msg("Что это?");

                    user.set_state(State::Something(TikTok(text.clone()), resp.result.message_id));
                }
            }
        }
    }

    Ok(())
}

fn handle_callback(callback: objects::CallbackQuery, users: &mut Vec<UserInfo>, api: &Api, post_db: &Connection) -> Result<()> {
    let chat_id = callback.message.unwrap().chat.id;
    let index = get_user_index(chat_id, users);
    let mut user = &users[index];

    match callback.data.unwrap().as_ref() {
        "Reels" => {
            if let State::Something(video, msg_id) = &user.state {
                api.delete_message(&DeleteMessageParams::builder().chat_id(chat_id).message_id(msg_id.clone()).build());
                add_video(video, &user, PostType::Reels, post_db, api)?;
            } else if let State::ChoiceWaiting(choice_message_id) = user.state {
                api.edit_message_text(&EditMessageTextParams::builder().chat_id(chat_id).message_id(choice_message_id).text("Хорошо, жду рилс, ты можешь отправить видео файл или ссылку на тикток.").build()).unwrap();
                user.set_state(State::ReelsWaiting);
            }
        }
        "Story" => {
            if let State::Something(video, msg_id) = &user.state {
                api.delete_message(&DeleteMessageParams::builder().chat_id(chat_id).message_id(msg_id.clone()).build());
                add_video(video, &user, PostType::Story, post_db, api)?;
            } else if let State::ChoiceWaiting(choice_message_id) = user.state {
                api.edit_message_text(&EditMessageTextParams::builder().chat_id(chat_id).message_id(choice_message_id).text("Хорошо, жду историю, ты можешь отправить видео файл или ссылку на тикток.").build()).unwrap();
                user.set_state(State::StoryWaiting);
            }
        }
        "Post" => {
            if let State::Something(video, msg_id) = &user.state {
                api.delete_message(&DeleteMessageParams::builder().chat_id(chat_id).message_id(msg_id.clone()).build());
                add_video(video, &user, PostType::Post, post_db, api)?;
            } else if let State::ChoiceWaiting(choice_message_id) = user.state {
                api.edit_message_text(&EditMessageTextParams::builder().chat_id(chat_id).message_id(choice_message_id).text("Хорошо, жду пост, ты можешь отправить видео/фото файл или ссылку на тикток.").build()).unwrap();
                user.set_state(State::PostWaiting);
            }
        }
        "Cancel" => {
            let reset = |msg_id: i32| {
                api.delete_message(&DeleteMessageParams::builder().chat_id(chat_id).message_id(msg_id).build());
                user.set_state(State::Start);
            };

            if let State::Something(_, msg_id) = user.state {
                reset(msg_id);
            } else if let State::ChoiceWaiting(msg_id) = user.state {
                reset(msg_id);
            }
        }
        _ => {}
    }

    Ok(())
}

fn make_keyboard(buttons: Vec<(&str, &str)>) -> ReplyMarkup {
    let mut keyboard: Vec<Vec<InlineKeyboardButton>> = Vec::new();
    let mut row: Vec<InlineKeyboardButton> = Vec::new();

    for (text, callback) in buttons {
        let btn = InlineKeyboardButton::builder().text(text).callback_data(callback).build();
        row.push(btn);
    }

    keyboard.push(row);
    ReplyMarkup::InlineKeyboardMarkup(InlineKeyboardMarkup::builder().inline_keyboard(keyboard).build())
}


// In feature we can remove is_hd param and always process video
fn get_video_link_and_id(video: &VideoWrapper, api: &Api) -> Result<(String, String, bool), > {
    let mut is_hd = false;
    let mut download_link;
    let id;

    match video {
        Telegram(video) => {
            let file = api.get_file(&GetFileParams::builder().file_id(&video.file_id).build())?.result;
            id = file.file_unique_id;
            download_link = format!("https://api.telegram.org/file/bot{}/{}", token, file.file_path.unwrap());

            if video.width >= 1080 || video.height >= 1920 {
                is_hd = true;
            }
        }
        TikTok(link) => {
            let client = Client::new();

            let url = format!("https://www.tikwm.com/api?url={}&count=12&cursor=0&web=1&hd=1", link);
            let req = client.request(reqwest::Method::POST, url).send()?;

            let json = Json::from_str(&req.text()?)?;
            let data = json.find("data").unwrap();

            id = data["id"].as_string().unwrap().to_owned();
            download_link = "https://www.tikwm.com".to_owned();

            if let Some(hd) = data.find("hdplay") {
                download_link.push_str(&hd.as_string().unwrap());
                is_hd = true;
            } else {
                download_link.push_str(&data["play"].as_string().unwrap());
            }
        }
    }

    Ok((download_link, id, is_hd))
}

fn download_video(download_link: &str, dist: &str) -> Result<()> {
    let res = reqwest::blocking::get(download_link)?;
    fs::write(dist, res.bytes()?)?;

    Ok(())
}

fn upscale_video(path: &str, dist: &str) -> Result<()> {
    process::Command::new("E:\\Project\\InstaPosterF\\ffmpeg.exe").args(vec![
        "-i", path, "-i", "E:\\Project\\InstaPosterF\\Watermark.png", "-filter_complex", "[0:v][1:v]overlay=main_w-overlay_w-30:main_h-overlay_h-30", "-s", "1080x1920", "-c:a", "copy", dist, "-y",
    ]).spawn()?.wait();

    Ok(())
}

fn add_video(video: &VideoWrapper, user: &UserInfo, post_type: PostType, post_db: &Connection, api: &Api) -> Result<()> {
    let msg = SendMessageParams::builder().chat_id(user.chat_id).text("Добовляю рилс...").build();
    let resp = api.send_message(&msg)?;


    let mut right_counter: &AtomicU32 = unsafe {
        match post_type {
            PostType::Reels => &reels_count,
            PostType::Story => &story_count,
            PostType::Post => &post_count,
        }
    };
    let right_word: String = post_type.to_string();

    let link_id_hd = get_video_link_and_id(video, &api)?;


    let is_unique: u32 = post_db.query_row(&format!("SELECT count(Id) FROM {} WHERE UniqueId={}", right_word, link_id_hd.1), [], |r| r.get(0)).unwrap();
    if is_unique > 0 {
        api.edit_message_text(&EditMessageTextParams::builder().chat_id(user.chat_id).message_id(resp.result.message_id).text("Это видео уже есть в списке").build())?;

        return Ok(());
    }


    let mut dist = format!("C:\\{}\\{}.mp4", right_word, right_counter.load(Ordering::SeqCst));
    if link_id_hd.2 {
        download_video(&link_id_hd.0, &dist);
    } else {
        let mut temp_dist = dist.to_owned();
        temp_dist.push_str("tmp");

        download_video(&link_id_hd.0, &temp_dist);
        upscale_video(&temp_dist, &dist);
        fs::remove_file(&temp_dist);
    }

    right_counter.fetch_add(1, Ordering::AcqRel);
    user.set_state(State::Start);

    post_db.execute("INSERT INTO Reels (UniqueId) VALUES (?1)", [&link_id_hd.1])?;

    api.edit_message_text(&EditMessageTextParams::builder().chat_id(user.chat_id).message_id(resp.result.message_id).text(format!("Рилс добавлен, в копилке уже {} {}", right_counter.load(Ordering::SeqCst), right_word)).build())?;

    Ok(())
}

fn get_closest_story_date(post_db: &Connection) -> DateTime<Utc> {
    unsafe {
        let now = Utc::now();
        const start_posting: i64 = 8;
        const end_posting: i64 = 22;

        let stories_count: i64 = post_db.query_row("SELECT Count(*) FROM Stories", [], |r| r.get(0)).unwrap();

        if stories_count == 0 {
            let interval = Duration::hours(end_posting - start_posting) / story_schedule[now.weekday().num_days_from_monday() as usize] as i32;

            let mut new_time = now.with_hour(start_posting as u32).unwrap().with_minute(0).unwrap().with_second(0).unwrap();

            while new_time <= now {
                new_time += interval;

                let hour = new_time.hour();
                if hour > end_posting as u32 || hour < start_posting as u32 || (hour == end_posting as u32 && new_time.minute() > 0) {
                    return now.add(Duration::days(1)).with_hour(start_posting as u32).unwrap().with_minute(0).unwrap().with_second(0).unwrap();
                }
            }

            new_time
        } else {
            let timestamp: i64 = post_db.query_row("SELECT PostTime FROM Stories ORDER BY Id DESC LIMIT 1", [], |r| r.get(0)).unwrap();
            let last_story_time = DateTime::from_naive_utc_and_offset(NaiveDateTime::from_timestamp_opt(timestamp, 0).unwrap(), Utc);

            let interval = Duration::hours(end_posting - start_posting) / story_schedule[last_story_time.weekday().num_days_from_monday() as usize] as i32;

            let new_time = last_story_time + interval;
            let hour = new_time.hour();
            if hour > end_posting as u32 || hour < start_posting as u32 || (hour == end_posting as u32 && new_time.minute() > 0) {
                return last_story_time.add(Duration::days(1)).with_hour(start_posting as u32).unwrap().with_minute(0).unwrap();
            }

            last_story_time + interval
        }
    }
}

fn get_closest_reels_date(post_db: &Connection) -> DateTime<Utc> {
    unsafe {
        let now = Utc::now();
        const start_posting: i64 = 8;
        const end_posting: i64 = 22;

        if reels_count.load(Ordering::SeqCst) == 0 {
            let interval = Duration::hours(end_posting - start_posting) / reels_schedule[now.weekday().num_days_from_monday() as usize] as i32;

            let mut new_time = now.with_hour(start_posting as u32).unwrap().with_minute(0).unwrap().with_second(0).unwrap();

            while new_time <= now {
                new_time += interval;

                let hour = new_time.hour();
                if hour > end_posting as u32 || hour < start_posting as u32 || (hour == end_posting as u32 && new_time.minute() > 0) {
                    return now.add(Duration::days(1)).with_hour(start_posting as u32).unwrap().with_minute(0).unwrap();
                }
            }

            new_time
        } else {
            let timestamp: i64 = post_db.query_row("SELECT PostTime FROM Reels ORDER BY Id DESC LIMIT 1", [], |r| r.get(0)).unwrap();
            let last_reels_time = DateTime::from_naive_utc_and_offset(NaiveDateTime::from_timestamp_opt(timestamp, 0).unwrap(), Utc);

            let interval = Duration::hours(end_posting - start_posting) / reels_schedule[last_reels_time.weekday().num_days_from_monday() as usize] as i32;

            let new_time = last_reels_time + interval;
            let hour = new_time.hour();
            if hour > end_posting as u32 || hour < start_posting as u32 || (hour == end_posting as u32 && new_time.minute() > 20) {
                return last_reels_time.add(Duration::days(1)).with_hour(start_posting as u32).unwrap().with_minute(0).unwrap().with_second(0).unwrap();
            }

            last_reels_time + interval
        }
    }
}