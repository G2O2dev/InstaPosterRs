use std::*;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::atomic::{AtomicU32, Ordering};

use anyhow::Result;
use frankenstein::*;
use futures_util::StreamExt;
use lazy_static::lazy_static;
use reqwest;
use reqwest::Client;
use rusqlite::*;
use tokio::sync::Mutex;

use crate::contol_panel::UserControl;
use crate::graph_api::fb::*;
use crate::instagrapi::user::IgUser;
use crate::page_info::PageInfo;
use crate::PostType::{Post, Reels, Story};
use crate::State::{AllToReels, AllToStory};
use crate::tiktok_api::video::{get_video_api16, get_video_tiktokvm, VideoInfo};
use crate::user_info::{UserInfo, UserType};
use crate::util::get_post_count;
use crate::VideoWrapper::{Telegram, TikTok};

mod user_info;
mod page_info;
mod util;
mod graph_api;
mod msg_handler;
mod contol_panel;
mod tiktok_api;
mod instagrapi;

//#region Fields

lazy_static! {
    pub static ref API: Api = Api::new(TOKEN);
    pub static ref CLIENT: Client = Client::new();
    pub static ref DB: Mutex<Connection> = Mutex::new(Connection::open(&[env::current_dir().unwrap(), "Posts.db".into()].iter().collect::<PathBuf>()).expect("Can't connect to database"));
    pub static ref REELS_COUNT: AtomicU32 = AtomicU32::new(0);
    pub static ref STORY_COUNT: AtomicU32 = AtomicU32::new(0);
    pub static ref POST_COUNT: AtomicU32 = AtomicU32::new(0);
}


pub const TOKEN: &str = "токен бота";


#[derive(PartialEq)]
pub enum State {
    Start,
    ReelsWaiting,
    StoryWaiting,
    PostWaiting,
    AllToWaiting(i32),
    ChoiceWaiting(i32),
    Something(VideoWrapper, i32),
    AllToReels,
    AllToStory,
    AllToPost,
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

//#region Telegram API hellpers

fn make_markup(buttons: Vec<(&str, &str)>) -> InlineKeyboardMarkup {
    let mut keyboard: Vec<Vec<InlineKeyboardButton>> = Vec::new();
    let mut row: Vec<InlineKeyboardButton> = Vec::new();

    for (text, callback) in buttons {
        let btn = InlineKeyboardButton::builder().text(text).callback_data(callback).build();
        row.push(btn);
    }
    keyboard.push(row);
    InlineKeyboardMarkup { inline_keyboard: keyboard }
}


//#endregion
pub fn handle_main_err<T>(res: Result<T, anyhow::Error>, uc: &UserControl, comment: &str) {
    if let Err(err) = res {
        let report = format!("An error has occurred\n{}\n{}\n{}", comment, err, err.backtrace());
        println!("{}", &report);
        uc.send_msg(&report, UserType::Admin);
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let mut update_params = GetUpdatesParams::builder().allowed_updates(vec![AllowedUpdate::Message, AllowedUpdate::CallbackQuery]).build();

    REELS_COUNT.store(get_post_count(Reels).await, Ordering::SeqCst);
    STORY_COUNT.store(get_post_count(Story).await, Ordering::SeqCst);
    POST_COUNT.store(get_post_count(Post).await, Ordering::SeqCst);

    let mut users: UserControl = UserControl::new(vec![UserInfo::new(1737227326, UserType::Admin), UserInfo::new(6916374574, UserType::Admin)]);
    let mut pages: Vec<PageInfo> = vec![
        PageInfo::new("Magic 4ish".to_string(), "188202787709983".to_string(), [3, 3, 3, 3, 3, 4, 4], [7, 7, 7, 7, 7, 10, 10], [3, 3, 3, 3, 3, 3, 3]),
        PageInfo::new("G2Mem".to_string(), "195652260295422".to_string(), [3, 2, 2, 2, 3, 4, 4], [5, 5, 5, 5, 5, 8, 8], [1, 1, 1, 1, 2, 2, 2]),
    ];
    handle_main_err(fill_page_info(&mut pages).await, &users, "Loading pages");

    loop {
        let result = API.get_updates(&update_params);

        match result {
            Ok(resp) => {
                for update in resp.result {
                    if let UpdateContent::Message(message) = update.content {
                        handle_main_err(handle_message(message, &mut users, &pages).await, &users, "Message handling");

                        update_params.offset = Some(update.update_id as i64 + 1);
                    } else if let UpdateContent::CallbackQuery(callback) = update.content {
                        if let Err(_) = API.answer_callback_query(&AnswerCallbackQueryParams::builder().callback_query_id(&callback.id).build()) {
                            continue;
                        }

                        handle_main_err(handle_callback(callback, &mut users).await, &users, "Callback handling");
                        update_params.offset = Some(update.update_id as i64 + 1);
                    }
                }
            }

            Err(e) => println!("{:#?}", e)
        }

        for page in &mut *pages {
            if let Some(post_type) = page.need_post_something().await {
                match post_type {
                    PostType::Reels => {
                        if REELS_COUNT.load(Ordering::SeqCst) > 0 {
                            handle_main_err(page.post_reels(&users).await, &users, "Reels uploading");
                        }
                    }
                    PostType::Story => {
                        if STORY_COUNT.load(Ordering::SeqCst) > 0 {
                            handle_main_err(page.post_story(&users).await, &users, "Story uploading");
                        }
                    }
                    PostType::Post => {
                        // if POST_COUNT.load(Ordering::SeqCst) > 0 {
                        //     page.post_post(&api, &users[0], &DB).await.unwrap();
                        //     POST_COUNT.fetch_sub(1, Ordering::SeqCst);
                        // }
                    }
                }
            }
        }
    }
}

async fn fill_page_info(pages: &mut Vec<PageInfo>) -> Result<()> {
    for page in &mut *pages {
        page.inst_id = get_ig_account(&page.page_id).await?;
    }

    let pages_id_token = get_pages().await?;
    for id_token in pages_id_token {
        for page in &mut *pages {
            if page.page_id == id_token.0 {
                page.page_token = id_token.1;
                break;
            }
        }
    }


    Ok(())
}

async fn handle_message(msg: Message, users: &mut UserControl, pages: &Vec<PageInfo>) -> Result<()> {
    let chat_id = ChatId::Integer(msg.chat.id);
    let mut user = users.try_add_user(msg.chat.id);

    let send_msg = |txt: &str| -> MethodResponse<Message> {
        API.send_message(&SendMessageParams::builder().chat_id(chat_id.clone()).text(txt).reply_markup(ReplyMarkup::InlineKeyboardMarkup(make_markup(vec!(("Рилс", "Reels"), ("История", "Story"), ("Отмена", "Cancel"))))).build()).expect("error sending message")
    };

    if let Some(video) = msg.video {
        match user.state {
            State::ReelsWaiting => {
                add_video(&Telegram(video), &user, PostType::Reels).await?;

                user.state = State::Start;
            }
            State::StoryWaiting => {
                add_video(&Telegram(video), &user, PostType::Story).await?;

                user.state = State::Start;
            }
            State::PostWaiting => {}
            State::AllToReels => {
                add_video(&Telegram(video), &user, PostType::Reels).await?;
            }
            State::AllToStory => {
                add_video(&Telegram(video), &user, PostType::Story).await?;
            }
            State::AllToPost => {}
            _ => {
                if let State::Something(_, msg_id) = user.state {
                    user.delete_msg(msg_id.clone())?;
                }
                let resp = send_msg("Что это?");

                user.state = State::Something(Telegram(video), resp.result.message_id);
            }
        }
    } else if let Some(text) = msg.text {
        match text.as_ref() {
            "/count" => {
                user.send_msg(format!("У нас в копилке:\n\n{} - Рилсов\n{} - Сторис\n{} - Постов", REELS_COUNT.load(Ordering::SeqCst), STORY_COUNT.load(Ordering::SeqCst), POST_COUNT.load(Ordering::SeqCst)))?;
                return Ok(());
            }
            "/all-to" => {
                let resp = API.send_message(&SendMessageParams::builder().chat_id(chat_id.clone()).text("Режим All to\n\nВыбери в какую категорию мне загружать все видео которые ты будешь отпраавлять").reply_markup(ReplyMarkup::InlineKeyboardMarkup(make_markup(vec![("Reels", "OnlyReels"), ("Story", "OnlyStory")]))).build())?;
                user.state = State::AllToWaiting(resp.result.message_id);
                return Ok(());
            }
            "/post-time" => {
                let mut resp = "Время ближайших постов:".to_string();

                for page in pages {
                    resp.push_str(&format!("\n\n{}:\nРилс - {}\nСторис - {}\nПост - {}", page.name, page.time_to_reels.format("%H:%M"), page.time_to_story.format("%H:%M"), page.time_to_post.format("%H:%M")));
                }

                user.send_msg(resp)?;
                return Ok(());
            }
            _ => {}
        }

        let valid_link = text.contains("tiktok.com");

        match user.state {
            State::Start => {
                if valid_link {
                    let resp = send_msg("Что это?");

                    user.state = State::Something(TikTok(text), resp.result.message_id);
                } else {
                    let resp = send_msg("Что ты хочешь добавить?");

                    user.state = State::ChoiceWaiting(resp.result.message_id);
                }
            }
            State::ReelsWaiting => {
                if valid_link {
                    add_video(&TikTok(text), &user, PostType::Reels).await?;
                    user.state = State::Start;
                }
            }
            State::StoryWaiting => {
                if valid_link {
                    add_video(&TikTok(text), &user, PostType::Story).await?;
                    user.state = State::Start;
                }
            }
            State::PostWaiting => {}
            State::Something(_, msg_id) => {
                if valid_link {
                    user.delete_msg(msg_id)?;

                    let resp = send_msg("Что это?");

                    user.state = State::Something(TikTok(text.clone()), resp.result.message_id);
                }
            }
            State::ChoiceWaiting(msg_id) => {
                if valid_link {
                    user.delete_msg(msg_id)?;

                    let resp = send_msg("Что это?");

                    user.state = State::Something(TikTok(text.clone()), resp.result.message_id);
                }
            }
            State::AllToWaiting(msg_id) => {
                user.delete_msg(msg_id)?;
                if valid_link {
                    let resp = send_msg("Что это?");

                    user.state = State::Something(TikTok(text), resp.result.message_id);
                } else {
                    let resp = send_msg("Что ты хочешь добавить?");

                    user.state = State::ChoiceWaiting(resp.result.message_id);
                }
            }
            State::AllToReels => {
                if valid_link {
                    add_video(&TikTok(text), &user, PostType::Reels).await?;
                }
            }
            State::AllToStory => {
                if valid_link {
                    add_video(&TikTok(text), &user, PostType::Story).await?;
                }
            }
            State::AllToPost => {}
        }
    }

    Ok(())
}

async fn handle_callback(callback: CallbackQuery, users: &mut UserControl) -> Result<()> {
    let chat_id = match callback.message.unwrap() {
        MaybeInaccessibleMessage::Message(m) => m.chat.id,
        MaybeInaccessibleMessage::InaccessibleMessage(m) => m.chat.id
    };
    let mut user = users.try_add_user(chat_id);

    match callback.data.unwrap().as_ref() {
        "Reels" => {
            if let State::Something(video, msg_id) = &user.state {
                API.delete_message(&DeleteMessageParams::builder().chat_id(chat_id).message_id(msg_id.clone()).build());
                add_video(video, &user, PostType::Reels).await?;
                user.state = State::Start;
            } else if let State::ChoiceWaiting(choice_message_id) = user.state {
                API.edit_message_text(&EditMessageTextParams::builder().chat_id(chat_id).message_id(choice_message_id).text("Хорошо, жду рилс, ты можешь отправить видео файл или ссылку на тикток.").build()).unwrap();
                user.state = State::ReelsWaiting;
            }
        }
        "Story" => {
            if let State::Something(video, msg_id) = &user.state {
                API.delete_message(&DeleteMessageParams::builder().chat_id(chat_id).message_id(msg_id.clone()).build())?;
                add_video(video, &user, PostType::Story).await?;
                user.state = State::Start;
            } else if let State::ChoiceWaiting(choice_message_id) = user.state {
                API.edit_message_text(&EditMessageTextParams::builder().chat_id(chat_id).message_id(choice_message_id).text("Хорошо, жду историю, ты можешь отправить видео файл или ссылку на тикток.").build()).unwrap();
                user.state = State::StoryWaiting;
            }
        }
        "Post" => {
            if let State::Something(video, msg_id) = &user.state {
                API.delete_message(&DeleteMessageParams::builder().chat_id(chat_id).message_id(msg_id.clone()).build())?;
                add_video(video, &user, PostType::Post).await?;
                user.state = State::Start;
            } else if let State::ChoiceWaiting(choice_message_id) = user.state {
                API.edit_message_text(&EditMessageTextParams::builder().chat_id(chat_id).message_id(choice_message_id).text("Хорошо, жду пост, ты можешь отправить видео/фото файл или ссылку на тикток.").build()).unwrap();
                user.state = State::PostWaiting;
            }
        }
        "Cancel" => {
            if let State::Something(_, msg_id) = &user.state {
                API.delete_message(&DeleteMessageParams::builder().chat_id(chat_id).message_id(*msg_id).build());
            } else if let State::ChoiceWaiting(msg_id) = &user.state {
                API.delete_message(&DeleteMessageParams::builder().chat_id(chat_id).message_id(*msg_id).build());
            }
            user.state = State::Start;
        }
        "OnlyReels" => {
            if let State::AllToWaiting(msg_id) = user.state {
                API.edit_message_text(&EditMessageTextParams::builder().chat_id(chat_id).message_id(msg_id).text("Хорошо, теперь всё что ты мне отправишь будет добавлено в Reels.").reply_markup(make_markup(vec![("Отменить", "Cancel")])).build())?;
                user.state = AllToReels;
            }
        }
        "OnlyStory" => {
            if let State::AllToWaiting(msg_id) = user.state {
                API.edit_message_text(&EditMessageTextParams::builder().chat_id(chat_id).message_id(msg_id).text("Хорошо, теперь всё что ты мне отправишь будет добавлено в Story.").reply_markup(make_markup(vec![("Отменить", "Cancel")])).build())?;
                user.state = AllToStory;
            }
        }
        _ => {}
    }

    Ok(())
}


async fn get_video_link_and_id(video: &VideoWrapper) -> Result<VideoInfo> {
    match video {
        Telegram(video) => {
            let file = API.get_file(&GetFileParams::builder().file_id(&video.file_id).build())?.result;

            return Ok(VideoInfo {
                id: file.file_unique_id,
                video_link: format!("https://api.telegram.org/file/bot{}/{}", TOKEN, file.file_path.unwrap()),
                hd: video.width >= 1080 || video.height >= 1920,
            });
        }
        TikTok(link) => {
            if let Ok(vi) = get_video_tiktokvm(link).await {
                return Ok(vi);
            } else {
                return get_video_api16(link).await;
            }
        }
    }
}

async fn download_file(download_link: &str, dist: &str) -> Result<()> {
    let mut file = fs::File::create(dist)?;

    println!("Start downloading {download_link} {dist}");
    let res = reqwest::get(download_link).await?;

    let mut stream = res.bytes_stream();
    while let Some(item) = stream.next().await {
        let bytes = item?;
        file.write_all(&bytes)?;
    }

    Ok(())
}

fn upscale_video(path: &str, dist: &str) -> Result<()> {
    let mut wm_ath = env::current_dir()?;
    wm_ath.push("Watermark.png");
    process::Command::new("ffmpeg").args(vec![
        "-i", path, "-i", &wm_ath.display().to_string(), "-filter_complex", "[0:v][1:v]overlay=main_w-overlay_w-30:main_h-overlay_h-30", "-s", "1080x1920", "-c:a", "copy", dist, "-y",
    ]).spawn()?.wait();

    Ok(())
}

async fn add_video(video: &VideoWrapper, user: &UserInfo, post_type: PostType) -> Result<()> {
    let msg_id = user.send_msg("Добовляю...")?.result.message_id;

    let right_counter: &AtomicU32 = unsafe {
        match post_type {
            PostType::Reels => &REELS_COUNT,
            PostType::Story => &STORY_COUNT,
            PostType::Post => &POST_COUNT,
        }
    };
    let right_word: String = post_type.to_string();

    user.edit_msg(msg_id, "Получаю данные о видео...");
    let video_info = get_video_link_and_id(video).await?;


    let is_unique: u32 = DB.lock().await.query_row(&format!("SELECT count(Id) FROM {} WHERE UniqueId=\"{}\"", right_word, video_info.id), [], |r| r.get(0)).unwrap();
    if is_unique > 0 {
        user.edit_msg(msg_id, "Это видео уже есть в списке");

        return Ok(());
    }

    user.edit_msg(msg_id, "Скачиваю видео...");
    let mut dist = env::current_dir().unwrap();
    dist.push(&right_word);
    dist.push(format!("{}.mp4", &video_info.id));

    if video_info.hd {
        download_file(&video_info.video_link, &dist.display().to_string()).await?;
    } else {
        let temp_dist = format!("{}tmp", &dist.display().to_string());

        download_file(&video_info.video_link, &temp_dist).await?;

        user.edit_msg(msg_id, "Видео плохого качества, улучшаю...");
        upscale_video(&temp_dist, &dist.display().to_string())?;
        fs::remove_file(&temp_dist);
    }

    right_counter.fetch_add(1, Ordering::AcqRel);

    DB.lock().await.execute(&format!("INSERT INTO {} (UniqueId) VALUES (?1)", &right_word), [&video_info.id])?;

    user.edit_msg(msg_id, &format!("Добавил, в копилке уже {} {}", right_counter.load(Ordering::SeqCst), right_word));

    Ok(())
}