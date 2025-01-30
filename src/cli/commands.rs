use std::{
    sync::{mpsc::channel, Arc},
    thread::{self, sleep},
    time::Duration,
};

use log::info;

use crate::{
    channel::ChannelMessages,
    twitch::{
        assets::get_badges,
        auth::{get_credentials, refresh_token, validate},
        eventsub::start_eventsub,
    },
};

pub fn start_chat(
    twitch_name: Option<&str>,
    oauth_token: Option<&str>,
    client_id: Option<&str>,
    _skip_announcements: bool,
) -> anyhow::Result<()> {
    info!("start_chat()");

    let (_name, token, id, refresh) = get_credentials(twitch_name, oauth_token, client_id)?;

    let token_status = match validate(&token) {
        Ok(_) => None,
        Err(_) => match refresh_token(&refresh) {
            Ok(token_status) => Some(token_status),
            Err(_) => panic!("Token refresh failed, unable to validate Twitch API access. Please login again."),
        },
    };
    info!("token validated.");

    let (oauth_token, client_id) = match token_status {
        Some(token_status) => (
            Arc::new(token_status.token.unwrap_or(token)),
            Arc::new(token_status.client_id.unwrap_or(id)),
        ),
        None => (Arc::new(token), Arc::new(id)),
    };
    info!("token status retrieved.");

    get_badges(&oauth_token, &client_id)?;
    info!("badges created");

    // NOTE: Take a look at what is happening in ChannelMessages and remove unused/uneeded things
    let (transmitter, _receiver) = channel::<ChannelMessages>();
    info!("channel message channel created");

    // let announce_tx = pubsub_tx.clone();
    // let chat_tx = pubsub_tx.clone();
    // let eventsub_tx = pubsub_tx.clone();

    // NOTE: This needs to go, whatever is in here should move to EventSub
    // let token = oauth_token.clone();
    // let id = client_id.clone();
    // thread::spawn(|| {
    //     connect_to_pub_sub(token, id, pubsub_tx).unwrap();
    // });

    // NOTE: Keep this
    // let token = oauth_token.clone();
    // let name = twitch_name.clone();
    // let id = client_id.clone();
    // thread::spawn(move || {
    //     let _ = start_announcements(&name, &token, &id, announce_tx, skip_announcements);
    // });

    // NOTE: I think this should move to EventSub
    // let id = client_id.clone();
    // let token = oauth_token.clone();
    // let name = twitch_name.clone();
    // thread::spawn(move || {
    //     let mut twitch_irc = TwitchIRC::new(&name, &token, &id, chat_tx);
    //     twitch_irc.listen();
    // });

    // NOTE: Keep this, this can go last, needs to happen after EventSub
    let (socket_transmitter, _socket_receiver) = channel::<ChannelMessages>();
    info!("websocket channel created");

    // thread::spawn(|| {
    //     start_websocket(socket_rx);
    // });

    // NOTE: Keep this
    let id = client_id.clone();
    let token = oauth_token.clone();
    let eventsub_transmitter = transmitter.clone();
    let eventsub_to_websocket_transmitter = socket_transmitter.clone();
    info!("attack of the clones");

    thread::spawn(|| {
        info!("started eventsub thread");
        start_eventsub(token, id, eventsub_transmitter, eventsub_to_websocket_transmitter);
    });

    info!("thread started");

    loop {
        sleep(Duration::from_secs(10));

        if false {
            break;
        }
    }

    // NOTE: Ratatui stuff, this should go away for Anathema
    // install_hooks()?;
    // App::new(&twitch_name).run(rx, socket_tx.clone())?;
    // restore()?;

    Ok(())
}
