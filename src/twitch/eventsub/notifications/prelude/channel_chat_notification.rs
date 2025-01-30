use std::sync::mpsc::Sender;

use crate::{
    channel::ChannelMessages,
    twitch::eventsub::deserialization::{NotificationEvent, NotificationPayload},
};

// use super::send_to_channels;

pub fn channel_chat_notification(
    payload: &NotificationPayload,
    _tui_tx: &Sender<ChannelMessages>,
    _websocket_tx: &Sender<ChannelMessages>,
) {
    let NotificationEvent::ChannelNotification { .. } = &*payload.event else {
        return;
    };

    // let channel_message = ChannelMessages::ClearMessagesByUser {
    //     target_user_name: target_user_name.clone(),
    // };
    //
    // send_to_channels(channel_message, tui_tx, websocket_tx, "channel_chat_notification");
}
