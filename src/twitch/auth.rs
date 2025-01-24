use anyhow::bail;
use serde::{Deserialize, Serialize};
use std::fs;

use crate::fs::get_data_directory;

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

pub fn read_auth_token<'a>() -> anyhow::Result<TokenStatus> {
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
