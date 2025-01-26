use std::{
    net::TcpStream,
    sync::{mpsc::Sender, Arc},
};

use anyhow::bail;
use deserialization::{Condition, Messages, MethodType, Subscription, SubscriptionType, Transport, WelcomePayload};
use log::info;
use notifications::handle_notification;
use tungstenite::{stream::MaybeTlsStream, WebSocket};

use crate::channel::ChannelMessages;

use super::api::{get_user, User};

const EVENT_SUB: &str = "wss://eventsub.wss.twitch.tv:443/ws?keepalive_timeout_seconds=30";
const SUBSCRIPTIONS: &str = "https://api.twitch.tv/helix/eventsub/subscriptions";

mod deserialization;
mod notifications;

pub fn start_eventsub(
    oauth_token: Arc<String>,
    client_id: Arc<String>,
    tx: Sender<ChannelMessages>,
    socket_tx: Sender<ChannelMessages>,
) {
    match tungstenite::connect(EVENT_SUB) {
        Ok((ref mut socket, _)) => {
            info!("eventsub connected");

            listen(socket, oauth_token, client_id, tx, socket_tx);
        }
        Err(_) => todo!(),
    }
}

fn listen(
    socket: &mut WebSocket<MaybeTlsStream<TcpStream>>,
    oauth_token: Arc<String>,
    client_id: Arc<String>,
    tui_tx: Sender<ChannelMessages>,
    websocket_tx: Sender<ChannelMessages>,
) {
    loop {
        info!("running listen loop...");

        if let Ok(message) = socket.read() {
            match message {
                tungstenite::Message::Text(text_message) => {
                    info!("got an eventsub message: {text_message}");

                    match serde_json::from_str::<Messages>(&text_message) {
                        Ok(message) => match &message {
                            Messages::Welcome { payload, .. } => {
                                create_subscriptions(payload, &oauth_token, &client_id)
                            }
                            Messages::KeepAlive { .. } => {
                                info!("It's alive!...");
                            }
                            Messages::Notification { metadata, payload } => {
                                handle_notification(metadata, payload, &tui_tx, &websocket_tx, &oauth_token, &client_id)
                            }
                            Messages::Reconnect { .. } => todo!(),
                            Messages::Revocation { .. } => todo!(),
                        },

                        Err(error) => {
                            info!("{error}");
                            continue;
                        }
                    }

                    // match msg.metadata.message_type {
                    //     MessageTypes::SessionWelcome => {
                    //         info!("Got a session welcome");
                    //
                    //         // create_subscriptions(msg, oauth_token.clone(), client_id.clone());
                    //     }
                    //
                    //     MessageTypes::Notification => {
                    //         if let Some(Subscription { r#type, .. }) = msg.payload.subscription {
                    //             match r#type.as_str() {
                    //                 CHANNEL_AD_BREAK_BEGIN => {
                    //                     if let Some(SubscriptionEvent { duration_seconds, .. }) = msg.payload.event {
                    //                         // channel_ad_break_begin_notification(duration_seconds, tx.clone());
                    //                     }
                    //                 }
                    //
                    //                 CHAT_CLEAR_USER_MESSAGES => {
                    //                     if let Some(SubscriptionEvent { target_user_login, .. }) = msg.payload.event {
                    //                         // chat_clear_user_messages_notification(target_user_login, tx.clone());
                    //                     }
                    //                 }
                    //
                    //                 CHANNEL_CHAT_NOTIFICATION => {
                    //                     if let Some(SubscriptionEvent { .. }) = msg.payload.event {
                    //                         // channel_chat_notification(msg.payload.event, tx.clone(), socket_tx.clone());
                    //                     }
                    //                 }
                    //
                    //                 &_ => {}
                    //             }
                    //         }
                    //     }
                    //
                    //     // TODO: Remove this and handle each enum variant
                    //     _ => {}
                    // }
                }

                tungstenite::Message::Ping(ping_message) => {
                    let _ = socket.send(tungstenite::Message::Pong(ping_message));
                }

                tungstenite::Message::Close(close_message) => {
                    println!("Close message received: {close_message:?}");
                }

                _ => {}
            }
        }
    }
}

fn create_subscriptions(payload: &WelcomePayload, oauth_token: &Arc<String>, client_id: &Arc<String>) {
    get_eventsub_subscription(
        &payload.session.id,
        SubscriptionType::ChannelAdBreakBegin,
        MethodType::WebSocket,
        oauth_token.clone(),
        client_id.clone(),
    )
    .expect("Channel ad break begin subscription failed");

    get_eventsub_subscription(
        &payload.session.id,
        SubscriptionType::ChannelChatClearUserMessages,
        MethodType::WebSocket,
        oauth_token.clone(),
        client_id.clone(),
    )
    .expect("Channel chat clear user messages subscription failed");

    get_eventsub_subscription(
        &payload.session.id,
        SubscriptionType::ChannelChatNotification,
        MethodType::WebSocket,
        oauth_token.clone(),
        client_id.clone(),
    )
    .expect("Channel chat notification subscription failed");

    get_eventsub_subscription(
        &payload.session.id,
        SubscriptionType::ChannelPointsCustomRewardRedemptionAdd,
        MethodType::WebSocket,
        oauth_token.clone(),
        client_id.clone(),
    )
    .expect("Channel chat notification subscription failed");
}

fn get_eventsub_subscription(
    session_id: &str,
    r#type: SubscriptionType,
    method: MethodType,
    oauth_token: Arc<String>,
    client_id: Arc<String>,
) -> anyhow::Result<()> {
    match get_user(&oauth_token.clone(), &client_id.clone()) {
        Ok(user) => {
            let sub_type = r#type.clone();
            let condition = get_condition(&user);
            let subscription = get_subscription(r#type, condition, session_id, method);

            match ureq::post(SUBSCRIPTIONS)
                .set(
                    "Authorization",
                    format!("Bearer {}", &oauth_token.replace("oauth:", "")).as_str(),
                )
                .set("Client-Id", &client_id)
                .set("Content-Type", "application/json")
                .send_json(subscription)
            {
                Ok(_) => Ok(()),
                Err(_) => bail!("Could not complete subscription request for {sub_type:?}"),
            }
        }
        Err(user_error) => bail!("Error retrieving user: {user_error}"),
    }
}

fn get_condition(user: &User) -> Condition {
    Condition {
        to_broadcaster_user_id: None,
        broadcaster_user_id: Some(user.id.clone()),
        moderator_user_id: None,
        user_id: Some(user.id.clone()),
        reward_id: None,
        client_id: None,
        organization_id: None,
        category_id: None,
        campaign_id: None,
        extension_client_id: None,
    }
}

fn get_subscription(
    r#type: SubscriptionType,
    condition: Condition,
    session_id: &str,
    method: MethodType,
) -> Subscription {
    Subscription {
        r#type,
        condition,
        // TODO: Update version to come from a mapping for each SubscriptionType
        version: "1".to_string(),
        transport: Transport {
            method,
            session_id: session_id.to_string(),
        },
        status: None,
        created_at: None,
    }
}
