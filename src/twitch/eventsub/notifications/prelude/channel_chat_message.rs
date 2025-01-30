use std::sync::mpsc::Sender;

use log::info;

use crate::{channel::ChannelMessages, twitch::eventsub::deserialization::NotificationPayload};

pub fn channel_chat_message(
    payload: &NotificationPayload,
    _tui_tx: &Sender<ChannelMessages>,
    _websocket_tx: &Sender<ChannelMessages>,
) {
    info!("channel_chat_message()");
    info!("{payload:?}");
}
