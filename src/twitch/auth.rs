use anyhow::bail;
use base64::{prelude::BASE64_STANDARD, Engine};
use serde::{Deserialize, Serialize};
use std::{fs, thread::sleep, time::Duration};

use crate::fs::get_data_directory;

const TEN_SECONDS: Duration = Duration::from_secs(10);
const MAX_RETRIES: u8 = 18;

const TWITCH_SCOPES: [&str; 17] = [
    "channel:read:subscriptions",
    "chat:read",
    "chat:edit",
    "channel:moderate",
    "channel:read:redemptions",
    "channel:manage:redemptions",
    "channel:bot",
    "user:write:chat",
    "moderator:manage:shoutouts",
    "user_read",
    "chat_login",
    "bits:read",
    "channel:moderate",
    "channel:read:ads",
    "user:read:chat",
    "user:bot",
    "channel:bot",
];

const TWITCH_CREATE_TOKEN: &str = "https://twitchtokengenerator.com/api/create/[APP_NAME]/[SCOPES]";
const TWITCH_TOKEN_STATUS: &str = "https://twitchtokengenerator.com/api/status/[ID]";

#[derive(Serialize, Deserialize, Debug)]
pub struct TokenResponse {
    success: bool,
    id: String,
    message: String,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct TokenStatus {
    pub success: bool,
    pub id: String,

    // Success field
    pub scopes: Option<Vec<String>>,
    pub token: Option<String>,
    pub refresh: Option<String>,
    pub username: Option<String>,
    pub client_id: Option<String>,

    // Error fields
    pub message: Option<String>,
    pub error: Option<usize>,
}

pub fn read_auth_token() -> anyhow::Result<TokenStatus> {
    let error_message =
        "You need to provide credentials via positional args, env vars, or by running the login command";
    let mut data_dir = get_data_directory(Some("token")).expect(error_message);
    data_dir.push("oath_token.txt");

    let token_file = fs::read_to_string(data_dir)?;

    Ok(serde_json::from_str::<TokenStatus>(&token_file)?)
}

pub fn get_credentials<'a>(
    twitch_name: Option<&'a str>,
    oauth_token: Option<&'a str>,
    client_id: Option<&'a str>,
) -> anyhow::Result<(String, String, String, String)> {
    match (twitch_name, oauth_token, client_id) {
        (Some(twitch_name), Some(oauth_token), Some(client_id)) => Ok((
            twitch_name.to_string(),
            oauth_token.to_string(),
            client_id.to_string(),
            "".to_string(),
        )),

        _ => {
            let error_message =
                "You need to provide credentials via positional args, env vars, or by running the login command";
            let token_status: TokenStatus = read_auth_token()?;

            if token_status.success {
                Ok((
                    token_status.username.unwrap(),
                    format!("oauth:{}", token_status.token.unwrap()),
                    token_status.client_id.unwrap(),
                    token_status.refresh.unwrap(),
                ))
            } else {
                panic!("{}", error_message);
            }
        }
    }
}

pub fn validate(oauth_token: &str) -> anyhow::Result<()> {
    let url = "https://id.twitch.tv/oauth2/validate";
    let token = oauth_token.replace("oauth:", "");
    let response = ureq::get(url).set("Authorization", &format!("OAuth {}", token)).call();

    let is_ok = response.is_ok();
    let status = response.unwrap().status();

    if is_ok && status == 200 {
        Ok(())
    } else {
        bail!(format!("Failed to validate token: {status}"))
    }
}

pub fn refresh_token(refresh_token: &str) -> anyhow::Result<TokenStatus> {
    let url = format!("https://twitchtokengenerator.com/api/refresh/{refresh_token}");
    let response = ureq::get(&url).call()?;

    let token_status = serde_json::from_str::<TokenStatus>(&response.into_string()?)?;
    if token_status.success {
        store_token(&token_status)?;
    }

    Ok(token_status)
}

pub fn store_token(token_status: &TokenStatus) -> anyhow::Result<()> {
    let mut token_dir = get_data_directory(Some("token"))?;
    token_dir.push("oath_token.txt");

    fs::write(token_dir, serde_json::to_string(&token_status)?)?;

    Ok(())
}

pub fn authenticate_with_twitch() -> anyhow::Result<()> {
    let app_name = BASE64_STANDARD.encode(clap::crate_name!());
    let url = TWITCH_CREATE_TOKEN
        .replace("[APP_NAME]", &app_name)
        .replace("[SCOPES]", &TWITCH_SCOPES.join("+"));

    let token_response = match ureq::get(&url).call() {
        Ok(response) => response,
        Err(_) => bail!("Failed to get token response"),
    };

    let token_response = serde_json::from_str::<TokenResponse>(&token_response.into_string()?)?;
    println!("Navigate to this url to grant a token: {}", token_response.message);

    let mut retries = 0;

    let status_id = token_response.id;
    let status_url = TWITCH_TOKEN_STATUS.replace("[ID]", &status_id);
    loop {
        if retries == MAX_RETRIES {
            println!("You took too long, please try again");
            break;
        }

        let token_status_response = match ureq::get(&status_url).call() {
            Ok(response) => response,
            Err(_) => {
                println!("Failed to get token status");
                bail!("token status response was bad");
            }
        };

        let token_status = serde_json::from_str::<TokenStatus>(&token_status_response.into_string()?)?;
        match token_status.success {
            true => {
                store_token(&token_status)?;
                break;
            }
            false => {
                sleep(TEN_SECONDS);
                retries += 1;
            }
        }
    }

    println!("Token has been successfully generated.");
    Ok(())
}
