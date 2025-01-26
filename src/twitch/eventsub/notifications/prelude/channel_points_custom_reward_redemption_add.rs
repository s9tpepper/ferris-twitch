use std::{
    process::{self, Command},
    sync::{mpsc::Sender, Arc},
};

use log::{error, info};

use crate::{
    channel::ChannelMessages,
    chat_commands::get_reward,
    twitch::eventsub::deserialization::{NotificationEvent, NotificationPayload, Reward},
};

use super::send_to_channels;

pub fn channel_points_custom_reward_redemption_add(
    payload: &NotificationPayload,
    tui_tx: &Sender<ChannelMessages>,
    websocket_tx: &Sender<ChannelMessages>,
    oauth_token: &Arc<String>,
    client_id: &Arc<String>,
) {
    let NotificationEvent::ChannelPointsCustomRewardRedemptionAdd {
        id,
        broadcaster_user_id,
        user_name,
        reward,
        user_input,
        status,
        ..
    } = &*payload.event
    else {
        return;
    };

    let Ok(cmd_mapping) = get_reward(&reward.title) else {
        return;
    };

    let (command_name, sub_command) = cmd_mapping.split_once(':').unwrap_or((&cmd_mapping, ""));

    let mut command = Command::new(command_name);
    if !sub_command.is_empty() {
        command.arg(sub_command);
        info!("Command with subcommand: {command:?}");
    }

    command.arg(user_name);

    if !user_input.is_empty() {
        command.arg(user_input);

        info!("Command with subcommand and user input: {command:?}");
    }

    let command_result = command
        .stdout(process::Stdio::piped())
        .stderr(process::Stdio::piped())
        .output();

    match command_result {
        Ok(command_result) => match command_result.status.success() {
            true => {
                // NOTE: values: unknown, unfulfilled, fulfilled, and canceled.
                let reward_status = status.to_lowercase();
                if reward_status == "unfulfilled" {
                    reward_fulfilled(id, reward, broadcaster_user_id, oauth_token, client_id);
                }
            }
            false => {
                refund_points(
                    id,
                    user_name,
                    broadcaster_user_id,
                    reward,
                    tui_tx,
                    websocket_tx,
                    oauth_token,
                    client_id,
                    command_result,
                );
            }
        },
        Err(ref command_error) => {
            error!("Error running reward command: {command_error}, command: {command:?}");

            if status.to_lowercase() == "unfulfilled" {
                refund_points(
                    id,
                    user_name,
                    broadcaster_user_id,
                    reward,
                    tui_tx,
                    websocket_tx,
                    oauth_token,
                    client_id,
                    command_result.expect("Command results should be available"),
                );
            }
        }
    }
}

fn reward_fulfilled(
    id: &str,
    reward: &Reward,
    broadcaster_user_id: &str,
    oauth_token: &Arc<String>,
    client_id: &Arc<String>,
) {
    let api_url = "https://api.twitch.tv/helix/channel_points/custom_rewards/redemptions";
    let reward_id = &reward.id;
    let response = ureq::patch(api_url)
        .set(
            "Authorization",
            &format!("Bearer {}", oauth_token.replace("oauth:", "")),
        )
        .set("Client-Id", client_id)
        .query_pairs(vec![
            ("id", id),
            ("broadcaster_id", broadcaster_user_id),
            ("reward_id", reward_id),
            ("status", "FULFILLED"),
        ])
        .call();

    if response.is_err() {
        error!("Fulfill Error {response:?}");
    }
}

#[allow(clippy::too_many_arguments)]
fn refund_points(
    id: &str,
    user_name: &str,
    broadcaster_user_id: &str,
    reward: &Reward,
    tui_tx: &Sender<ChannelMessages>,
    websocket_tx: &Sender<ChannelMessages>,
    oauth_token: &Arc<String>,
    client_id: &Arc<String>,
    command_result: process::Output,
) {
    let api_url = "https://api.twitch.tv/helix/channel_points/custom_rewards/redemptions";

    let response = ureq::patch(api_url)
        .set(
            "Authorization",
            &format!("Bearer {}", oauth_token.replace("oauth:", "")),
        )
        .set("Client-Id", client_id)
        .query_pairs(vec![
            ("id", id),
            ("broadcaster_id", broadcaster_user_id),
            ("reward_id", &reward.id),
            ("status", "CANCELED"),
        ])
        .call();

    let success = response.is_ok();
    if !success {
        error!("Refund Error: {response:?}");
    }

    let points = reward.cost;
    let result = if success { "were" } else { "could not be" };
    let message = format!("{points} points {result} refunded to {user_name}");
    let command_output = String::from_utf8(command_result.stdout)
        .expect("Invalid UTF-8")
        .to_string();

    let channel_message = ChannelMessages::RedeemRefund {
        message,
        command_output,
    };

    send_to_channels(channel_message, tui_tx, websocket_tx);
}
