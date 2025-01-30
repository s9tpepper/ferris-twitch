use std::sync::mpsc::Sender;

use log::{error, info};

use crate::{
    channel::ChannelMessages,
    twitch::eventsub::{
        deserialization::{NotificationEvent, NotificationPayload},
        notifications::send_to_channels,
    },
};

pub fn channel_chat_message(
    payload: &NotificationPayload,
    tui_tx: &Sender<ChannelMessages>,
    websocket_tx: &Sender<ChannelMessages>,
) {
    info!("channel_chat_message()");
    info!("{payload:?}");

    // TODO: Figure out which fields are needed and send only those via
    // ChannelMessages::ChatMessage instead of sending the entire enum variant
    let NotificationEvent::ChannelNotification { .. } = &*payload.event else {
        error!("Error trying to destructure NotificationEvent::ChannelNotification");
        return;
    };

    let channel_message = ChannelMessages::ChatMessage {
        message: &payload.event,
    };

    send_to_channels(channel_message, tui_tx, websocket_tx);
}
