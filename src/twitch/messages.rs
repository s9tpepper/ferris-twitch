use crate::utils::get_data_directory;
use hex_rgb::*;
use serde::{Deserialize, Serialize};

use std::error::Error;
use std::fs;
use std::io::Cursor;
use std::path::PathBuf;

use base64::prelude::*;
use irc::client::prelude::Message;
use irc::proto::message::Tag;
use irc::proto::Command;

const ESCAPE: &str = "\x1b";
const BELL: &str = "\x07";

type AsyncResult<T> = Result<T, Box<dyn Error>>;

#[derive(Debug)]
pub struct Emote {
    // id: String,
    start: usize,
    end: usize,
    url: String,
    name: String,
}

#[derive(Debug)]
pub struct ChatMessage {
    pub badges: Vec<String>,
    pub emotes: Vec<Emote>,
    pub nickname: String,
    pub display_name: String,
    pub first_msg: bool,
    pub returning_chatter: bool,
    pub subscriber: bool,
    pub moderator: bool,
    pub message: String,
    pub color: String,
    pub channel: String,
    pub raid: bool,
    pub raid_notice: String,
}

#[derive(Debug)]
pub enum TwitchMessage {
    RaidMessage {
        user_id: String,
        raid_notice: String,
    },
    PrivMessage {
        message: ChatMessage,
    },
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BadgeVersion {
    // id: String,
    // title: String,
    // description: String,
    // click_action: String,
    // click_url: String,
    image_url_1x: String,
    // image_url_2x: String,
    // image_url_4x: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BadgeItem {
    set_id: String,
    versions: Vec<BadgeVersion>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct TwitchApiResponse<T> {
    pub data: T,
}

pub async fn get_badges(token: &str, client_id: &String) -> AsyncResult<Vec<BadgeItem>> {
    // Global badges: https://api.twitch.tv/helix/chat/badges/global
    // oauth:141241241241241
    //
    // scopes:
    // chat:read+chat:edit+channel:moderate+channel:read:redemptions+channel:bot+user:write:chat
    // base64: encoded app title
    // https://twitchtokengenerator.com/api/create
    //
    let mut response = reqwest::Client::new()
        .get("https://api.twitch.tv/helix/chat/badges/global")
        .header(
            "Authorization",
            format!("Bearer {}", token.replace("oauth:", "")),
        )
        .header("Client-Id", client_id)
        .send()
        .await?
        .json::<TwitchApiResponse<Vec<BadgeItem>>>()
        .await?;

    let data_dir = get_data_directory(None)?;

    for badge_item in response.data.iter_mut() {
        let file_name = format!("{}.txt", badge_item.set_id);
        let Some(ref version) = badge_item.versions.pop() else {
            continue;
        };

        let badge_path = data_dir.join(file_name);

        if !badge_path.exists() {
            generate_badge_file(badge_path, version).await?;
        }
    }

    Ok(response.data)
}

async fn get_encoded_image(url: &str) -> Result<String, Box<dyn Error>> {
    let file_bytes: Vec<u8> = reqwest::get(url).await?.bytes().await?.to_vec();
    let img_data = image::load_from_memory(&file_bytes)?;

    let mut buffer: Vec<u8> = Vec::new();
    img_data.write_to(&mut Cursor::new(&mut buffer), image::ImageOutputFormat::Png)?;
    let base64_emote = BASE64_STANDARD.encode(&buffer);

    Ok(base64_emote)
}

async fn generate_badge_file(
    badge_path: PathBuf,
    version: &BadgeVersion,
) -> Result<(), Box<dyn Error>> {
    if let Ok(encoded_image) = get_encoded_image(&version.image_url_1x).await {
        fs::write(badge_path, encoded_image)?;
    }

    Ok(())
}

impl ChatMessage {
    pub fn get_nickname_color(&self) -> (u8, u8, u8) {
        let color = Color::new(&self.color).unwrap();

        (color.red, color.green, color.blue)
    }
}

fn get_bool(value: &str) -> bool {
    value != "0"
}

async fn parse_privmsg(irc_message: Message) -> Result<TwitchMessage, Box<dyn Error>> {
    let nickname: String = irc_message.source_nickname().unwrap_or("").to_owned();

    let mut badges: Vec<String> = vec![];
    let mut color = "#FF9912".to_string();
    let mut display_name = "".to_string();
    let mut first_msg = false;
    let mut subscriber = false;
    let mut returning_chatter = false;
    let mut moderator = false;
    let mut emotes: Vec<Emote> = vec![];
    let raid = false;
    let raid_notice = "".to_string();

    if let Some(tags) = irc_message.tags {
        for Tag(tag, value) in tags {
            match tag.as_str() {
                "badges" => set_badges(value, &mut badges),
                "color" => {
                    if let Some(value) = value {
                        if !value.is_empty() {
                            color = value;
                        }
                    }
                }
                "display-name" => {
                    if let Some(value) = value {
                        display_name = value.trim().to_string();
                    }
                }
                "first-msg" => {
                    if let Some(value) = value {
                        first_msg = get_bool(&value);
                    }
                }
                "subscriber" => {
                    if let Some(value) = value {
                        subscriber = get_bool(&value);
                    }
                }
                "returning-chatter" => {
                    if let Some(value) = value {
                        returning_chatter = get_bool(&value);
                    }
                }
                "mod" => {
                    if let Some(value) = value {
                        moderator = get_bool(&value);
                    }
                }
                "emotes" => process_emotes(value, &mut emotes),
                _other => {}
            }
        }
    }

    let Command::PRIVMSG(ref msg_sender, ref msg) = irc_message.command else {
        return Err("This shoulnt happen".into());
    };

    let channel = msg_sender.to_string();
    let mut message = msg.to_string();

    add_emotes(&mut message, &mut emotes).await?;
    let encoded_badges = add_badges(&badges).await?;

    display_name = format!("{} {}", encoded_badges, display_name);

    let twitch_message = TwitchMessage::PrivMessage {
        message: ChatMessage {
            badges,
            emotes,
            nickname,
            display_name,
            first_msg,
            returning_chatter,
            subscriber,
            message,
            moderator,
            color,
            channel,
            raid,
            raid_notice,
        },
    };

    Ok(twitch_message)
}

async fn parse_raw(irc_message: Message) -> Result<TwitchMessage, Box<dyn Error>> {
    if irc_message.to_string().contains("USERNOTICE") {
        let mut system_msg = String::new();
        let mut is_raid = false;
        let mut user_id = String::new();

        if let Some(tags) = irc_message.tags {
            for Tag(tag, value) in tags {
                if let Some(value) = &value {
                    if value == "raid" {
                        is_raid = true;
                    }

                    if tag == "system-msg" {
                        system_msg = value.to_string();
                    }

                    if tag == "user-id" {
                        user_id = value.to_string();
                    }
                }
                // let value = value.unwrap_or("".to_string());
            }

            if is_raid && !system_msg.is_empty() {
                return Ok(TwitchMessage::RaidMessage {
                    raid_notice: system_msg,
                    user_id,
                });
            }
        }
    }

    Err("oops".into())
}

pub async fn parse(irc_message: Message) -> Result<TwitchMessage, Box<dyn Error>> {
    let twitch_message = match irc_message.command {
        Command::PRIVMSG(ref _msg_sender, ref _msg) => parse_privmsg(irc_message).await?,
        Command::Raw(ref _raw_string, ref _vec) => parse_raw(irc_message).await?,
        _other => return Err("Unhandled Command".into()),
    };

    Ok(twitch_message)
}

fn get_iterm_encoded_image(base64: String) -> String {
    format!(
        "{}]1337;File=inline=1;height=20px;preserveAspectRatio=1:{}{}",
        ESCAPE,
        base64.as_str(),
        BELL
    )
}

async fn add_badges(badges: &[String]) -> Result<String, Box<dyn Error>> {
    let mut badges_list = String::new();
    let data_dir = get_data_directory(None)?;
    for badge in badges.iter() {
        let badge_path = data_dir.join(format!("{}.txt", badge));
        let base64 = fs::read_to_string(badge_path)?;
        let encoded_badge = get_iterm_encoded_image(base64);
        // format!("{} {}", encoded_badge.as_str(), twitch_message.display_name.as_ref().unwrap()));
        badges_list.push_str(&encoded_badge);
    }

    Ok(badges_list)
}

/// Tmux sucks.
fn is_tmux() -> bool {
    let term = std::env::var("TERM").unwrap();
    term.contains("tmux") || term.contains("screen")
}

fn get_emote_prefix() -> String {
    if is_tmux() {
        return format!("{0}Ptmux;{0}{0}]", ESCAPE);
    }

    format!("{ESCAPE}]")
}

fn get_emote_suffix() -> String {
    if is_tmux() {
        return format!("{}{}\\.", BELL, ESCAPE);
    }

    BELL.to_string()
}

async fn add_emotes(message: &mut String, emotes: &mut [Emote]) -> Result<(), Box<dyn Error>> {
    for emote in emotes.iter_mut() {
        let range = emote.start..=emote.end;
        let temp_msg = message.clone();
        let emote_name = temp_msg.get(range);
        emote.name = emote_name.unwrap_or("").to_string();
    }

    for emote in emotes.iter() {
        let file_bytes: Vec<u8> = reqwest::get(&emote.url).await?.bytes().await?.to_vec();

        let img_data = image::load_from_memory(&file_bytes)?;

        let mut buffer: Vec<u8> = Vec::new();
        img_data.write_to(&mut Cursor::new(&mut buffer), image::ImageOutputFormat::Png)?;
        let base64_emote = BASE64_STANDARD.encode(&buffer);

        let encoded_image = format!(
            "{}1337;File=inline=1;height=20px;width=20px;preserveAspectRatio=1:{}{}",
            get_emote_prefix(),
            base64_emote.as_str(),
            get_emote_suffix()
        );

        *message = message.replace(&emote.name, encoded_image.as_str());
    }

    Ok(())
}

// 303147449:0-13
// id: text-position-for-emote
// https://static-cdn.jtvnw.net/emoticons/v2/303147449/default/dark/1.0
fn process_emotes(tag_value: Option<String>, emotes: &mut Vec<Emote>) {
    if let Some(value) = tag_value {
        for emote_data in value.split('/') {
            let mut emote_parts = emote_data.split(':');
            let emote_id = emote_parts.next();
            let Some(emote_id) = emote_id else {
                continue;
            };

            let positions = emote_parts.next();
            let Some(mut emote_position_data) = positions else {
                continue;
            };

            if let Some((a, _)) = emote_position_data.split_once(',') {
                emote_position_data = a;
            }

            let (s, e) = emote_position_data.split_once('-').unwrap();
            let start = s.to_string().parse::<usize>().unwrap();
            let end = e.to_string().parse::<usize>().unwrap();

            let url = format!(
                "https://static-cdn.jtvnw.net/emoticons/v2/{}/default/dark/1.0",
                emote_id
            );

            let name = "".to_string();

            let emote = Emote {
                // id: emote_id.to_owned(),
                start,
                end,
                url,
                name,
            };

            emotes.push(emote);
        }
    }
}

fn set_badges(tag_value: Option<String>, valid_badges: &mut Vec<String>) {
    if let Some(value) = tag_value {
        for badge in value.split(',') {
            let mut badge_parts = badge.split('/');
            if let Some(key) = badge_parts.next() {
                let value = badge_parts.next().unwrap_or("0");
                if value == "1" {
                    valid_badges.push(key.to_string());
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use irc::proto::message::Tag;
    use irc::proto::Message;

    use crate::twitch::fixtures::TEST_MESSAGE_WITH_EMOTES;

    use super::parse;

    use std::error::Error;

    #[tokio::test]
    async fn test_parse_raid_message() -> Result<(), Box<dyn Error>> {
        let tag = Tag(
            "emotes".to_string(),
            Some("303147449:0-13/emotesv2_a388006c5b8c4826906248a22b50d0a3:15-28".to_string()),
        );
        let tag2 = Tag("msg-id".to_string(), Some("raid".to_string()));
        let tag3 = Tag(
            "system-msg".to_string(),
            Some("system-msg=1\\sraiders\\sfrom\\svei_bean\\shave\\sjoined!".to_string()),
        );

        let tags = vec![tag, tag2, tag3];
        let msg = Message::with_tags(
            Some(tags),
            Some("rayslash!rayslash@rayslash.tmi.twitch.tv"),
            "USERNOTICE",
            vec!["#s9tpepper_"],
        )
        .unwrap();

        println!("{:?}", msg.prefix);
        println!("{:?}", msg.command);

        let twitch_message = parse(msg).await?;

        match twitch_message {
            crate::twitch::messages::TwitchMessage::RaidMessage { raid_notice } => {
                assert_eq!(
                    "system-msg=1\\sraiders\\sfrom\\svei_bean\\shave\\sjoined!",
                    raid_notice
                );
            }
            _ => {}
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_parse_emotes_attaching() -> Result<(), Box<dyn Error>> {
        let tag = Tag(
            "emotes".to_string(),
            Some("303147449:0-13/emotesv2_a388006c5b8c4826906248a22b50d0a3:15-28".to_string()),
        );
        let tags = vec![tag];
        let msg = Message::with_tags(
            Some(tags),
            Some("rayslash!rayslash@rayslash.tmi.twitch.tv"),
            "PRIVMSG",
            vec!["#s9tpepper_", "This is a message from twitch"],
        );

        let twitch_message = parse(msg.unwrap()).await?;
        match twitch_message {
            crate::twitch::messages::TwitchMessage::PrivMessage { message } => {
                assert_eq!(TEST_MESSAGE_WITH_EMOTES, message.message);
            }
            _ => {}
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_parse_emotes_length() -> Result<(), Box<dyn Error>> {
        let tag = Tag(
            "emotes".to_string(),
            Some("303147449:0-13/emotesv2_a388006c5b8c4826906248a22b50d0a3:15-28".to_string()),
        );
        let tags = vec![tag];
        let msg = Message::with_tags(
            Some(tags),
            Some("rayslash!rayslash@rayslash.tmi.twitch.tv"),
            "PRIVMSG",
            vec!["#s9tpepper_", "This is a message from twitch"],
        );

        let twitch_message = parse(msg.unwrap()).await?;
        match twitch_message {
            crate::twitch::messages::TwitchMessage::PrivMessage { message } => {
                assert_eq!(2, message.emotes.len());
            }
            _ => {}
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_parse_emotes_url() -> Result<(), Box<dyn Error>> {
        let tag = Tag(
            "emotes".to_string(),
            Some("303147449:0-13/emotesv2_a388006c5b8c4826906248a22b50d0a3:15-28".to_string()),
        );
        let tags = vec![tag];
        let msg = Message::with_tags(
            Some(tags),
            Some("rayslash!rayslash@rayslash.tmi.twitch.tv"),
            "PRIVMSG",
            vec!["#s9tpepper_", "This is a message from twitch"],
        );

        let twitch_message = parse(msg.unwrap()).await?;
        match twitch_message {
            crate::twitch::messages::TwitchMessage::PrivMessage { message } => {
                assert_eq!(
                    "https://static-cdn.jtvnw.net/emoticons/v2/303147449/default/dark/1.0",
                    message.emotes[0].url
                );
            }
            _ => {}
        }

        Ok(())
    }

    // #[tokio::test]
    // async fn test_parse_emotes_id() -> Result<(), Box<dyn Error>> {
    //     let tag = Tag(
    //         "emotes".to_string(),
    //         Some("303147449:0-13/emotesv2_a388006c5b8c4826906248a22b50d0a3:15-28".to_string()),
    //     );
    //     let tags = vec![tag];
    //     let msg = Message::with_tags(
    //         Some(tags),
    //         Some("rayslash!rayslash@rayslash.tmi.twitch.tv"),
    //         "PRIVMSG",
    //         vec!["#s9tpepper_", "This is a message from twitch"],
    //     );
    //
    //     let twitch_message = parse(msg.unwrap()).await?;
    //
    //     assert_eq!("303147449", twitch_message.emotes[0].id);
    //
    //     Ok(())
    // }

    #[tokio::test]
    async fn test_parse_emotes_position() -> Result<(), Box<dyn Error>> {
        let tag = Tag(
            "emotes".to_string(),
            Some("303147449:0-13/emotesv2_a388006c5b8c4826906248a22b50d0a3:15-28".to_string()),
        );
        let tags = vec![tag];
        let msg = Message::with_tags(
            Some(tags),
            Some("rayslash!rayslash@rayslash.tmi.twitch.tv"),
            "PRIVMSG",
            vec!["#s9tpepper_", "This is a message from twitch"],
        );

        let twitch_message = parse(msg.unwrap()).await?;
        match twitch_message {
            crate::twitch::messages::TwitchMessage::PrivMessage { message } => {
                assert_eq!(0, message.emotes[0].start);
                assert_eq!(13, message.emotes[0].end);
            }
            _ => {}
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_parse_message() -> Result<(), Box<dyn Error>> {
        let tag = Tag(
            "badges".to_string(),
            Some("broadcaster/1,premium/1".to_string()),
        );
        let tags = vec![tag];
        let msg = Message::with_tags(
            Some(tags),
            Some("rayslash!rayslash@rayslash.tmi.twitch.tv"),
            "PRIVMSG",
            vec!["#s9tpepper_", "This is a message from twitch"],
        );

        let twitch_message = parse(msg.unwrap()).await?;
        match twitch_message {
            crate::twitch::messages::TwitchMessage::PrivMessage { message } => {
                assert_eq!("This is a message from twitch", message.message);
            }
            _ => {}
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_parse_nickname() -> Result<(), Box<dyn Error>> {
        let tag = Tag(
            "badges".to_string(),
            Some("broadcaster/1,premium/1".to_string()),
        );
        let tags = vec![tag];
        let msg = Message::with_tags(
            Some(tags),
            Some("rayslash!rayslash@rayslash.tmi.twitch.tv"),
            "PRIVMSG",
            vec!["#s9tpepper_", "This is a message from twitch"],
        );

        let twitch_message = parse(msg.unwrap()).await?;
        match twitch_message {
            crate::twitch::messages::TwitchMessage::PrivMessage { message } => {
                assert_eq!("rayslash", message.nickname);
            }
            _ => {}
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_parse_display_name() -> Result<(), Box<dyn Error>> {
        let tag = Tag("display-name".to_string(), Some("s9tpepper_".to_string()));
        let tags = vec![tag];
        let msg = Message::with_tags(
            Some(tags),
            Some(""),
            "PRIVMSG",
            vec!["#s9tpepper_", "This is a message from twitch"],
        );

        let twitch_message = parse(msg.unwrap()).await?;
        match twitch_message {
            crate::twitch::messages::TwitchMessage::PrivMessage { message } => {
                assert_eq!(" s9tpepper_", message.display_name);
            }
            _ => {}
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_parse_color() -> Result<(), Box<dyn Error>> {
        let tag = Tag("color".to_string(), Some("#8A2BE2".to_string()));
        let tags = vec![tag];
        let msg = Message::with_tags(
            Some(tags),
            Some(""),
            "PRIVMSG",
            vec!["#s9tpepper_", "This is a message from twitch"],
        );

        let twitch_message = parse(msg.unwrap()).await?;
        match twitch_message {
            crate::twitch::messages::TwitchMessage::PrivMessage { message } => {
                assert_eq!("#8A2BE2", message.color);
            }
            _ => {}
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_parse_returning_chatter_is_true() -> Result<(), Box<dyn Error>> {
        let tag = Tag("returning-chatter".to_string(), Some("1".to_string()));
        let tags = vec![tag];
        let msg = Message::with_tags(
            Some(tags),
            Some(""),
            "PRIVMSG",
            vec!["#s9tpepper_", "This is a message from twitch"],
        );

        let twitch_message = parse(msg.unwrap()).await?;
        match twitch_message {
            crate::twitch::messages::TwitchMessage::PrivMessage { message } => {
                assert_eq!(true, message.returning_chatter);
            }
            _ => {}
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_parse_returning_chatter_is_false() -> Result<(), Box<dyn Error>> {
        let tag = Tag("returning-chatter".to_string(), Some("0".to_string()));
        let tags = vec![tag];
        let msg = Message::with_tags(
            Some(tags),
            Some(""),
            "PRIVMSG",
            vec!["#s9tpepper_", "This is a message from twitch"],
        );

        let twitch_message = parse(msg.unwrap()).await?;
        match twitch_message {
            crate::twitch::messages::TwitchMessage::PrivMessage { message } => {
                assert_eq!(false, message.returning_chatter);
            }
            _ => {}
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_parse_subscriber_is_true() -> Result<(), Box<dyn Error>> {
        let tag = Tag("subscriber".to_string(), Some("1".to_string()));
        let tags = vec![tag];
        let msg = Message::with_tags(
            Some(tags),
            Some(""),
            "PRIVMSG",
            vec!["#s9tpepper_", "This is a message from twitch"],
        );

        let twitch_message = parse(msg.unwrap()).await?;
        match twitch_message {
            crate::twitch::messages::TwitchMessage::PrivMessage { message } => {
                assert_eq!(true, message.subscriber);
            }
            _ => {}
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_parse_subscriber_is_false() -> Result<(), Box<dyn Error>> {
        let tag = Tag("subscriber".to_string(), Some("0".to_string()));
        let tags = vec![tag];
        let msg = Message::with_tags(
            Some(tags),
            Some(""),
            "PRIVMSG",
            vec!["#s9tpepper_", "This is a message from twitch"],
        );

        let twitch_message = parse(msg.unwrap()).await?;
        match twitch_message {
            crate::twitch::messages::TwitchMessage::PrivMessage { message } => {
                assert_eq!(false, message.subscriber);
            }
            _ => {}
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_parse_moderator_is_true() -> Result<(), Box<dyn Error>> {
        let tag = Tag("mod".to_string(), Some("1".to_string()));
        let tags = vec![tag];
        let msg = Message::with_tags(
            Some(tags),
            Some(""),
            "PRIVMSG",
            vec!["#s9tpepper_", "This is a message from twitch"],
        );

        let twitch_message = parse(msg.unwrap()).await?;
        match twitch_message {
            crate::twitch::messages::TwitchMessage::PrivMessage { message } => {
                assert_eq!(true, message.moderator);
            }
            _ => {}
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_parse_moderator_is_false() -> Result<(), Box<dyn Error>> {
        let tag = Tag("mod".to_string(), Some("0".to_string()));
        let tags = vec![tag];
        let msg = Message::with_tags(
            Some(tags),
            Some(""),
            "PRIVMSG",
            vec!["#s9tpepper_", "This is a message from twitch"],
        );

        let twitch_message = parse(msg.unwrap()).await?;
        match twitch_message {
            crate::twitch::messages::TwitchMessage::PrivMessage { message } => {
                assert_eq!(false, message.moderator);
            }
            _ => {}
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_parse_first_msg_is_true() -> Result<(), Box<dyn Error>> {
        let tag = Tag("first-msg".to_string(), Some("1".to_string()));
        let tags = vec![tag];
        let msg = Message::with_tags(
            Some(tags),
            Some(""),
            "PRIVMSG",
            vec!["#s9tpepper_", "This is a message from twitch"],
        );

        let twitch_message = parse(msg.unwrap()).await?;
        match twitch_message {
            crate::twitch::messages::TwitchMessage::PrivMessage { message } => {
                assert_eq!(true, message.first_msg);
            }
            _ => {}
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_parse_first_msg_is_false() -> Result<(), Box<dyn Error>> {
        let tag = Tag("first-msg".to_string(), Some("0".to_string()));
        let tags = vec![tag];
        let msg = Message::with_tags(
            Some(tags),
            Some(""),
            "PRIVMSG",
            vec!["#s9tpepper_", "This is a message from twitch"],
        );

        let twitch_message = parse(msg.unwrap()).await?;
        match twitch_message {
            crate::twitch::messages::TwitchMessage::PrivMessage { message } => {
                assert_eq!(false, message.first_msg);
            }
            _ => {}
        }

        Ok(())
    }
}
