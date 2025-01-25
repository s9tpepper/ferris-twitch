use anyhow::bail;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct TwitchApiResponse<T> {
    pub data: T,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub login: String,
    pub display_name: String,
    pub r#type: String,
    pub broadcaster_type: String,
    pub description: String,
    pub profile_image_url: String,
    pub offline_image_url: String,
    pub created_at: String,
}

pub fn get_user(oauth_token: &str, client_id: &str) -> anyhow::Result<User> {
    let get_users_url = "https://api.twitch.tv/helix/users";
    let response = ureq::get(get_users_url)
        .set(
            "Authorization",
            &format!("Bearer {}", oauth_token.replace("oauth:", "")),
        )
        .set("Client-Id", client_id)
        .call();

    let Ok(response) = response else {
        bail!("Failed to get user data");
    };

    let mut response: TwitchApiResponse<Vec<User>> = serde_json::from_reader(response.into_reader())?;

    let user = response.data.swap_remove(0);

    Ok(user)
}
