use std::{fs, path::PathBuf};

use serde::{Deserialize, Serialize};

use crate::{fs::get_data_directory, image_protocols::get_iterm_image_encoding};

use super::api::TwitchApiResponse;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct BadgeVersion {
    id: String,
    // title: String,
    // description: String,
    // click_action: String,
    // click_url: String,
    image_url_1x: String,
    // image_url_2x: String,
    // image_url_4x: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BadgeItem {
    set_id: String,
    versions: Vec<BadgeVersion>,
}

pub fn get_badges(token: &str, client_id: &str) -> anyhow::Result<Vec<BadgeItem>> {
    // Global badges: https://api.twitch.tv/helix/chat/badges/global
    // oauth:141241241241241
    //
    // scopes:
    // chat:read+chat:edit+channel:moderate+channel:read:redemptions+channel:bot+user:write:chat
    // base64: encoded app title
    // https://twitchtokengenerator.com/api/create
    //
    let response = ureq::get("https://api.twitch.tv/helix/chat/badges/global")
        .set("Authorization", &format!("Bearer {}", token.replace("oauth:", "")))
        .set("Client-Id", client_id)
        .call()?;

    let mut response: TwitchApiResponse<Vec<BadgeItem>> = serde_json::from_reader(response.into_reader())?;

    let data_dir = get_data_directory(Some("badges"))?;

    for badge_item in response.data.iter_mut() {
        for version in badge_item.versions.iter_mut() {
            let file_name = format!("{}_{}.txt", badge_item.set_id, version.id);
            let badge_path = data_dir.join(file_name);
            if !badge_path.exists() {
                generate_badge_file(badge_path, version)?;
            }
        }
    }

    Ok(response.data)
}

fn generate_badge_file(badge_path: PathBuf, version: &BadgeVersion) -> anyhow::Result<()> {
    let encoded_image = get_iterm_image_encoding(&version.image_url_1x)?;
    fs::write(badge_path, encoded_image)?;

    Ok(())
}
