use std::sync::Arc;

use crate::twitch::{
    assets::get_badges,
    auth::{get_credentials, refresh_token, validate, TokenStatus},
};

pub fn start_chat(
    twitch_name: Option<&str>,
    oauth_token: Option<&str>,
    client_id: Option<&str>,
    _skip_announcements: bool,
) -> anyhow::Result<()> {
    let (_name, token, id, refresh) = get_credentials(twitch_name, oauth_token, client_id)?;

    let validate_token_response = validate(&token);
    let mut token_status: Option<TokenStatus> = None;
    if validate_token_response.is_err() {
        let token_status_result = refresh_token(&refresh);
        if token_status_result.is_err() {
            panic!("Token refresh failed, unable to validate Twitch API access. Please login again.");
        }

        token_status = Some(token_status_result.unwrap());
    }

    let (oauth_token, client_id) = match token_status {
        Some(token_status) => (
            Arc::new(token_status.token.unwrap_or(token)),
            Arc::new(token_status.client_id.unwrap_or(id)),
        ),
        None => (Arc::new(token), Arc::new(id)),
    };

    get_badges(&oauth_token, &client_id)?;

    // NOTE: Take a look at what is happening in ChannelMessages and remove unused/uneeded things
    // let (pubsub_tx, rx) = channel::<ChannelMessages>();
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

    // NOTE: Keep this
    // let (socket_tx, socket_rx) = channel::<ChannelMessages>();
    // thread::spawn(|| {
    //     start_websocket(socket_rx);
    // });

    // NOTE: Keep this
    // let id = client_id.clone();
    // let token = oauth_token.clone();
    // let eventsub_socket_tx = socket_tx.clone();
    // thread::spawn(|| {
    //     start_eventsub(token, id, eventsub_tx, eventsub_socket_tx);
    // });

    // NOTE: Ratatui stuff, this should go away for Anathema
    // install_hooks()?;
    // App::new(&twitch_name).run(rx, socket_tx.clone())?;
    // restore()?;

    Ok(())
}
