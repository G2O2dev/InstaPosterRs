use std::{env, fs, result};
use std::ops::Add;
use std::sync::atomic::Ordering;
use std::sync::atomic::Ordering::SeqCst;

use anyhow::Result;
use chrono::{Datelike, DateTime, Duration, Timelike, Utc};
use rand::Rng;
use rusqlite::Error;

use crate::{DB, PostType, REELS_COUNT, STORY_COUNT};
use crate::contol_panel::UserControl;
use crate::graph_api::fb;
use crate::graph_api::ig;
use crate::PostType::{Reels, Story};
use crate::user_info::{UserInfo, UserType};
use crate::util::{get_post_count, upload_file};

pub struct PageInfo {
    pub name: String,
    pub inst_id: String,
    pub page_id: String,
    pub page_token: String,

    pub reels_schedule: [u32; 7],
    pub story_schedule: [u32; 7],
    pub post_schedule: [u32; 7],

    pub time_to_reels: DateTime<Utc>,
    pub time_to_story: DateTime<Utc>,
    pub time_to_post: DateTime<Utc>,
}

impl PageInfo {
    pub fn new(name: String, page_id: String, reels_schedule: [u32; 7], story_schedule: [u32; 7], post_schedule: [u32; 7]) -> Self {
        PageInfo {
            name,
            inst_id: "".to_string(),
            page_id,
            page_token: "".to_string(),

            reels_schedule,
            story_schedule,
            post_schedule,

            time_to_reels: PageInfo::get_closest_time(&reels_schedule),
            time_to_story: PageInfo::get_closest_time(&story_schedule),
            time_to_post: PageInfo::get_closest_time(&post_schedule),
        }
    }

    pub async fn need_post_something(&self) -> Option<PostType> {
        let now = Utc::now();

        if self.time_to_reels <= now {
            let resp: u32 = DB.lock().await.query_row("SELECT EXISTS(SELECT 1 UsedBy FROM Reels WHERE UsedBy IS NOT (?1))", [&self.page_id], |r| r.get(0)).unwrap();

            if resp != 0 {
                return Some(Reels);
            }
        }
        if self.time_to_story <= now {
            let resp: u32 = DB.lock().await.query_row("SELECT EXISTS(SELECT 1 UsedBy FROM Story WHERE UsedBy IS NOT (?1))", [&self.page_id], |r| r.get(0)).unwrap();

            if resp != 0 {
                return Some(Story);
            }
        }

        None
    }

    pub async fn post_reels(&mut self, uc: &UserControl) -> Result<()> {
        let mut used_by: String = "".to_string();
        let uid: String = DB.lock().await.query_row("SELECT UniqueId,UsedBy FROM Reels WHERE UsedBy IS NOT (?1)", [&self.page_id], |r| {
            used_by = r.get(1).or_else(|_| Ok::<String, Error>("".to_string()))?;
            r.get(0)
        })?;


        let mut file_path = env::current_dir()?;
        file_path.push("Reels");
        file_path.push(format!("{uid}.mp4"));

        let content_url = upload_file(&file_path, "reels.mp4").await.expect("Error while uploading file");

        fb::upload_reel(&self.page_id, &self.page_token, "#happy #funny #smile", fb::Src::Url(&content_url)).await?;

        ig::upload_reel(&self.inst_id, "#happy #funny #smile", &content_url, &uc).await?;


        if used_by != "" && rand::thread_rng().gen_range(0..10) <= 2 {
            DB.lock().await.execute(&format!("UPDATE Reels SET UsedBy = {} WHERE UniqueId = '{}'", self.page_id, uid), ())?;
        } else {
            fs::remove_file(file_path)?;
            DB.lock().await.execute(&format!("DELETE FROM Reels WHERE UniqueId=\"{}\"", uid), ())?;
            REELS_COUNT.fetch_sub(1, SeqCst);
        }

        uc.send_msg(&format!("Загрузил рилс в {}, рилсов осталось {}\n\n{}", self.name, REELS_COUNT.load(SeqCst), content_url), UserType::Admin);
        self.time_to_reels = PageInfo::get_closest_time(&self.reels_schedule);
        Ok(())
    }
    pub async fn post_story(&mut self, uc: &UserControl) -> Result<()> {
        let mut used_by: String = "".to_string();
        let uid: String = DB.lock().await.query_row("SELECT UniqueId,UsedBy FROM Story WHERE UsedBy IS NOT (?1)", [&self.page_id], |r| {
            used_by = r.get(1).or_else(|_| Ok::<String, Error>("".to_string()))?;
            r.get(0)
        })?;

        let mut file_path = env::current_dir()?;
        file_path.push("Story");
        file_path.push(format!("{uid}.mp4"));

        let content_url = upload_file(&file_path, "story.mp4").await?;

        fb::upload_story(&self.page_id, &self.page_token, fb::Src::Url(&content_url)).await?;

        ig::upload_story(&self.inst_id, &content_url, uc).await?;

        if used_by != "" && rand::thread_rng().gen_range(0..10) <= 3 {
            DB.lock().await.execute(&format!("UPDATE Story SET UsedBy = {} WHERE UniqueId = '{}'", self.page_id, uid), ())?;
        } else {
            fs::remove_file(file_path)?;
            DB.lock().await.execute(&format!("DELETE FROM Story WHERE UniqueId=\"{}\"", uid), ())?;
            STORY_COUNT.fetch_sub(1, SeqCst);
        }

        uc.send_msg(&format!("Загрузил сторис в {}, историй осталось {}\n\n{}", self.name, STORY_COUNT.load(SeqCst), content_url), UserType::Admin);
        self.time_to_story = PageInfo::get_closest_time(&self.story_schedule);
        Ok(())
    }
    pub async fn post_post(&self, uc: &UserControl) -> Result<()> {
        Ok(())
    }

    fn get_closest_time(schedule: &[u32; 7]) -> DateTime<Utc> {
        unsafe {
            let now = Utc::now();
            const start_posting: u32 = 8;
            const end_posting: u32 = 22;

            let hour = now.hour();
            if hour > end_posting || (hour == end_posting && now.minute() > 0) {
                return now.add(Duration::days(1)).with_hour(start_posting).unwrap().with_minute(0).unwrap();
            } else if hour < start_posting {
                return now.with_hour(start_posting).unwrap().with_minute(0).unwrap();
            }

            let day_of_week = now.weekday().num_days_from_monday();
            let interval = Duration::hours((end_posting - start_posting) as i64) / schedule[day_of_week as usize] as i32;


            let mut new_time = now.with_hour(start_posting).unwrap().with_minute(0).unwrap().with_second(0).unwrap();

            while new_time <= now {
                new_time += interval;

                let hour = new_time.hour();
                if hour > end_posting || (hour == end_posting && new_time.minute() > 0) {
                    return now.add(Duration::days(1)).with_hour(start_posting).unwrap().with_minute(0).unwrap();
                } else if hour < start_posting {
                    return now.with_hour(start_posting).unwrap().with_minute(0).unwrap();
                }
            }

            new_time
        }
    }
}