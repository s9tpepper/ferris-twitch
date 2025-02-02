use crate::commands::get_action;
use crate::scrollview::scroll_view::ScrollView;
use crate::scrollview::state::ScrollViewState;
use crate::twitch::irc::TwitchIrcClient;

use color_eyre::config::HookBuilder;
use color_eyre::eyre;

use ratatui::prelude::*;

use std::io::{self, stdout, Stdout};
use std::process::{self, Command};
use std::str::FromStr;
use std::sync::mpsc::{Receiver, Sender};
use std::{error::Error, fs};
use std::{panic, time};

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use crossterm::{execute, terminal::*};
use ratatui::backend::CrosstermBackend;

use color_eyre::{eyre::Result, eyre::WrapErr};

use crate::tui;
use crate::twitch::parse::Text;
use crate::twitch::parse::{Emote, RedeemMessage};
use crate::twitch::pubsub::{send_to_error_log, SubMessage};
use crate::twitch::ChannelMessages;
use crate::{
    twitch::{
        irc::TwitchIRC,
        parse::{ChatMessage, TwitchMessage},
    },
    utils::get_data_directory,
};

pub mod test_messages;

/// A type alias for the terminal type used in this application
pub type Tui = Terminal<CrosstermBackend<Stdout>>;

#[derive(Debug, Default)]
pub struct App {
    twitch_name: String,
    scroll_view_state: ScrollViewState,
    chat_log: Vec<ChannelMessages>,
    exit: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Symbol {
    Text(Text),
    Emote(Emote),
}

#[derive(Debug, PartialEq)]
pub enum MessageParts {
    Text(Vec<Symbol>),
    Emote(Emote),
}

impl Clone for MessageParts {
    fn clone(&self) -> Self {
        match self {
            MessageParts::Text(symbols) => MessageParts::Text(symbols.clone()),
            MessageParts::Emote(emote) => MessageParts::Emote(emote.clone()),
        }
    }
}

impl App {
    pub fn new(twitch_name: &str) -> Self {
        let scroll_view_state = ScrollViewState::new();

        App {
            twitch_name: twitch_name.to_string(),
            scroll_view_state,
            chat_log: vec![],
            exit: false,
        }
    }

    pub fn handle_events(&mut self, terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
        let available = event::poll(time::Duration::from_millis(16))?;
        if available {
            match event::read()? {
                // NOTE: it's important to check that the event is a key press event as
                // crossterm also emits key release and repeat events on Windows.
                Event::Key(key_event) if key_event.kind == KeyEventKind::Press => {
                    send_to_error_log(
                        format!(">>> DEBUG: Handling this key event: {:?}", key_event.code),
                        "".to_string(),
                    );

                    self.handle_key_event(key_event, terminal).wrap_err_with(|| {
                        send_to_error_log(
                            format!(">>> ERROR: Couldnt do this key event: {:?}", key_event.code),
                            "".to_string(),
                        );

                        format!("Failed to handle key event: {:?}", key_event.code)
                    })
                }

                _ => Ok(()),
            }
        } else {
            Ok(())
        }
    }

    pub fn handle_key_event(
        &mut self,
        key_event: KeyEvent,
        terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    ) -> Result<()> {
        match key_event.code {
            KeyCode::Char('q') => self.exit(),
            KeyCode::Char('j') => self.scroll_view_state.scroll_down(),
            KeyCode::Char('k') => self.scroll_view_state.scroll_up(),
            KeyCode::Char('f') => self.scroll_view_state.scroll_page_down(),
            KeyCode::Char('b') => self.scroll_view_state.scroll_page_up(),
            KeyCode::Char('g') => self.scroll_view_state.scroll_to_top(),
            KeyCode::Char('G') => self.scroll_view_state.scroll_to_bottom(),
            _ => {}
        }

        terminal.draw(|frame| self.render(frame))?;

        Ok(())
    }

    pub fn run(&mut self, rx: Receiver<ChannelMessages>, socket_tx: Sender<ChannelMessages>) -> Result<()> {
        let mut terminal = Terminal::new(CrosstermBackend::new(stdout())).expect("No TUI");

        execute!(stdout(), EnterAlternateScreen)?;

        enable_raw_mode()?;

        let _ = self.restore_chat_log();

        // NOTE: Test messages can go here for now
        // let test_raid_message = get_raid_message();
        // self.chat_log.insert(0, test_raid_message);

        terminal.draw(|frame| {
            self.render(frame);
            self.scroll_view_state.scroll_to_bottom();
        })?;

        while !self.exit {
            let _ = self.handle_events(&mut terminal);

            if let Ok(message) = rx.try_recv() {
                let _ = terminal.backend_mut().clear_region(backend::ClearType::All);

                match &message {
                    ChannelMessages::TwitchMessage(message) => {
                        match message {
                            TwitchMessage::ClearMessage { message } => {
                                self.chat_log.retain(|msg| match msg {
                                    ChannelMessages::TwitchMessage(TwitchMessage::PrivMessage { message: msg }) => {
                                        msg.id != message.message_id
                                    }
                                    _ => true,
                                });
                            }

                            // NOTE: This message comes from EventSub channel
                            TwitchMessage::ClearMessageByUser { message } => {
                                self.chat_log.retain(|msg| match msg {
                                    ChannelMessages::TwitchMessage(TwitchMessage::PrivMessage { message: msg }) => {
                                        msg.nickname.to_lowercase() != message.display_name.to_lowercase()
                                    }
                                    _ => true,
                                });
                            }

                            _ => {
                                // This line removes the artifacts from behind emotes, but is now causing
                                // the chat to flicker when a new message is received
                                let _ = terminal.backend_mut().clear_region(backend::ClearType::All);

                                let mut display_new_msg = true;
                                if let TwitchMessage::PrivMessage { message } = message {
                                    let msg = message.message.clone();
                                    let repeated = self.chat_log.iter().any(|item| match item {
                                        ChannelMessages::TwitchMessage(tm) => {
                                            if let TwitchMessage::PrivMessage { message: pm } = tm {
                                                return pm.message == msg && pm.is_bot;
                                            }

                                            false
                                        }
                                        _ => false,
                                    });

                                    if message.nickname == self.twitch_name // is streamer
                                        && !message.is_bot // message is not from bot
                                        && repeated
                                    {
                                        display_new_msg = false;
                                    }
                                };

                                if display_new_msg {
                                    self.chat_log.insert(0, ChannelMessages::TwitchMessage(message.clone()));
                                    self.truncate();

                                    terminal.draw(|frame| self.render(frame))?;
                                }
                            }
                        }
                    }

                    ChannelMessages::MessageData(message) => match &message.data {
                        SubMessage::Points(points_message) => {
                            let message = format!(
                                "{} redeemed {} for {}",
                                points_message.redemption.user.display_name,
                                points_message.redemption.reward.title,
                                points_message.redemption.reward.cost
                            );

                            let rm = RedeemMessage {
                                message,
                                area: None,
                                color: None,
                            };
                            let redeem_message = TwitchMessage::RedeemMessage { message: rm };
                            self.chat_log.insert(0, ChannelMessages::TwitchMessage(redeem_message));

                            terminal.draw(|frame| self.render(frame))?;
                        }

                        // TODO: Implement Sub and Bits messages
                        SubMessage::Sub(_) => {}
                        SubMessage::Bits(_) => {}
                    },

                    // noop here
                    ChannelMessages::Announcement(_) => {}

                    ChannelMessages::Notifications(subscription_event) => {
                        if let Some(notice_type) = &subscription_event.notice_type {
                            #[allow(clippy::single_match)]
                            match notice_type.as_str() {
                                "announcement" => {
                                    let mut color = String::from_str("grey").unwrap();
                                    if let Some(announcement) = &subscription_event.announcement {
                                        color = announcement.color.to_string();
                                    }
                                    let rgb = color_name::Color::val().by_string(color);
                                    let message_color = rgb.unwrap_or((128, 128, 128).into());

                                    if let Some(message) = &subscription_event.message {
                                        let redeem_message = RedeemMessage {
                                            message: message.text.clone(),
                                            area: None,
                                            color: Some(message_color.into()),
                                        };

                                        let redeem_message = TwitchMessage::RedeemMessage {
                                            message: redeem_message,
                                        };
                                        self.chat_log.insert(0, ChannelMessages::TwitchMessage(redeem_message));

                                        terminal.draw(|frame| self.render(frame))?;
                                    }
                                }

                                &_ => {}
                            }
                        }
                    }
                }

                // NOTE: Comment out for local dev
                let _ = self.persist_chat_log();

                let socket_tx_send_result = socket_tx.send(message);
                if let Err(send_error) = socket_tx_send_result {
                    send_to_error_log(
                        "websocket: Could not transmit message to channel".to_string(),
                        send_error.to_string(),
                    )
                } else {
                    send_to_error_log(
                        "websocket: Successfully transmitted message to channel".to_string(),
                        "Success".to_string(),
                    );
                }

                // terminal.draw(|frame| self.render(frame))?;
            }
        }

        Ok(())
    }

    fn render(&mut self, frame: &mut Frame) {
        let mut state = self.scroll_view_state;
        frame.render_stateful_widget(self, frame.area(), &mut state);
    }

    fn exit(&mut self) {
        self.exit = true;
    }

    fn restore_chat_log(&mut self) -> Result<(), Box<dyn Error>> {
        let target_dir = "chat_log";
        let mut chat_log_path = get_data_directory(Some(target_dir))?;
        chat_log_path.push("log.txt");

        if chat_log_path.is_file() {
            let chat_log_str = fs::read_to_string(&chat_log_path).expect("");
            if let Ok(chat_log) = serde_json::from_str(&chat_log_str) {
                self.chat_log = chat_log;
            }
        }

        Ok(())
    }

    fn persist_chat_log(&self) -> Result<(), Box<dyn Error>> {
        let json_string = serde_json::to_string(&self.chat_log).unwrap_or_default();

        let target_dir = "chat_log";
        let mut chat_log_path = get_data_directory(Some(target_dir))?;

        if !chat_log_path.exists() {
            fs::create_dir_all(&chat_log_path)?;
        }

        if !json_string.is_empty() {
            chat_log_path.push("log.txt");
            fs::write(chat_log_path, json_string)?;
        }

        Ok(())
    }

    fn truncate(&mut self) {
        if self.chat_log.len() > 100 {
            self.chat_log.remove(self.chat_log.len() - 1);
        }
    }
}

impl StatefulWidget for &mut App {
    type State = ScrollViewState;

    fn render(self, area: Rect, buf: &mut Buffer, _state: &mut Self::State) {
        buf.reset();

        let content_size = layout::Size {
            // Subtract one to avoid getting horizontal scrollbar from tui-scrollview
            width: area.width.saturating_sub(1),
            height: area.height * 2,
        };

        let mut scroll_view = ScrollView::new(content_size);

        let mut available_area = area;
        available_area.height = content_size.height;

        if self.chat_log.is_empty() {
            self.scroll_view_state.scroll_to_bottom();
        }

        self.chat_log.iter_mut().for_each(|message| {
            let message_area = match message {
                ChannelMessages::TwitchMessage(message) => match message {
                    TwitchMessage::PrivMessage { message } => {
                        message.render(available_area, scroll_view.buf_mut());

                        message.area
                    }

                    TwitchMessage::RedeemMessage { message } => {
                        message.render(available_area, scroll_view.buf_mut());

                        message.area
                    }

                    TwitchMessage::RaidMessage { message } => {
                        message.render(available_area, scroll_view.buf_mut());

                        message.area
                    }

                    _ => Some(Rect::new(0, 0, 0, 0)),
                },

                ChannelMessages::MessageData(message) => match message.data {
                    SubMessage::Sub(ref mut sub_message) => {
                        sub_message.render(available_area, scroll_view.buf_mut());

                        sub_message.area
                    }

                    SubMessage::Bits(ref mut sub_message) => {
                        sub_message.render(available_area, scroll_view.buf_mut());

                        sub_message.area
                    }

                    _ => Some(Rect::new(0, 0, 0, 0)),
                },

                _ => Some(Rect::new(0, 0, 0, 0)),
            };

            if let Some(message_area) = message_area {
                available_area.height = available_area.height.saturating_sub(message_area.height);
            }
        });

        scroll_view.render(buf.area, buf, &mut self.scroll_view_state);
    }
}

/// This replaces the standard color_eyre panic and error hooks with hooks that
/// restore the terminal before printing the panic or error.
pub fn install_hooks() -> color_eyre::Result<()> {
    let (panic_hook, eyre_hook) = HookBuilder::default().into_hooks();

    // convert from a color_eyre PanicHook to a standard panic hook
    let panic_hook = panic_hook.into_panic_hook();
    panic::set_hook(Box::new(move |panic_info| {
        tui::restore().unwrap();
        panic_hook(panic_info);
    }));

    // convert from a color_eyre EyreHook to a eyre ErrorHook
    let eyre_hook = eyre_hook.into_eyre_hook();
    eyre::set_hook(Box::new(move |error: &(dyn std::error::Error + 'static)| {
        tui::restore().unwrap();
        eyre_hook(error)
    }))?;

    Ok(())
}

// NOTE: Use this for testing
// TODO: Figure out how to swap this when debugging or running tests
//
// pub type Tui = Terminal<TestBackend>;

pub fn output(message: TwitchMessage, client: &mut TwitchIRC) {
    if let TwitchMessage::PrivMessage { message } = message {
        print_message(&message, client);
    }
}

pub fn get_list_commands() -> Result<Vec<String>, Box<dyn Error>> {
    get_list("chat_commands")
}

/// Restore the terminal to its original state
pub fn restore() -> io::Result<()> {
    execute!(stdout(), LeaveAlternateScreen)?;
    disable_raw_mode()?;
    Ok(())
}

fn get_list(directory: &str) -> Result<Vec<String>, Box<dyn Error>> {
    let command_path = get_data_directory(Some(directory))?;
    let mut commands = vec![];
    let dir_entries = fs::read_dir(command_path)?;

    for entry in dir_entries {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            let file_name = path.file_name();
            if let Some(file_name) = file_name {
                commands.push(file_name.to_string_lossy().to_string());
            }
        }
    }

    Ok(commands)
}

pub fn check_for_irc_actions(message: &str, display_name: &str, client: &mut impl TwitchIrcClient) {
    let irc_actions_list = get_list("irc_actions");
    if let Ok(list) = &irc_actions_list {
        for item in list {
            let command = format!("!{}", item);
            if message == command {
                let _ = execute_command(item, display_name, client);
            }
        }
    }
}

pub fn execute_command(
    command: &str,
    display_name: &str,
    client: &mut impl TwitchIrcClient,
) -> Result<(), Box<dyn Error>> {
    let irc_action = get_action(command);

    let found_command = if irc_action.is_ok() {
        irc_action
    } else {
        return Ok(());
    };

    let Ok(cmd_mapping) = found_command else {
        return Ok(());
    };

    let Some((cmd_mapping, command_option)) = cmd_mapping.split_once(' ') else {
        return Ok(());
    };

    let (command_name, sub_command) = cmd_mapping.split_once(':').unwrap_or((cmd_mapping, ""));

    send_to_error_log(
        "irc_action".to_string(),
        format!("command name: {command_name}, sub_command: {sub_command}"),
    );

    let mut command = Command::new(command_name);

    let mut args = vec![];
    if !sub_command.is_empty() {
        args.push(sub_command);
        send_to_error_log("irc_action".to_string(), format!("Added sub_command: {sub_command}"));
    }

    if !display_name.is_empty() {
        args.push(display_name);
        send_to_error_log("irc_action".to_string(), format!("Added display_name: {display_name}"));
    }

    command.args(args.clone());
    send_to_error_log("irc_action".to_string(), format!("Added args: {}", args.join(",")));

    let command_result = command
        .stdout(process::Stdio::piped())
        .stderr(process::Stdio::piped())
        .output()
        .expect("irc action failed");

    let command_success = command_result.status.success();
    if command_success && command_option == "chat" {
        if let Ok(stdout) = String::from_utf8(command_result.stdout.clone()) {
            client.send_privmsg(&stdout);

            // Send message to display since this IRC client is the one posting the message
            // it won't display if its not sent directly to the TUI client for rendering
            client.display_msg(&stdout);
        }
    }

    if !command_success {
        send_to_error_log(
            command_name.to_string(),
            "Error running irc_action command with input: {}".to_string(),
        );

        send_to_error_log(
            format!("{} output: {:?}", command_name, command_result),
            "Error running reward command with input: {}".to_string(),
        );
    }

    Ok(())
}

pub fn check_for_chat_commands(message: &str, client: &mut impl TwitchIrcClient) {
    let commands_list = get_list_commands();
    if message == "!commands" {
        let available_commands = commands_list
            .unwrap()
            .iter()
            .map(|item| format!("!{}", item))
            .collect::<Vec<String>>()
            .join(", ");

        let message = format!("Available commands: {available_commands}");
        client.send_privmsg(message.as_str());

        // Send message to display since this IRC client is the one posting the message
        // it won't display if its not sent directly to the TUI client for rendering
        client.display_msg(&message);

        return;
    }

    if let Ok(list) = &commands_list {
        for item in list {
            let command = format!("!{}", item);
            if message == command {
                let _ = output_chat_command(item, client);
            }
        }
    }
}

fn output_chat_command(command: &str, client: &mut impl TwitchIrcClient) -> Result<(), Box<dyn Error>> {
    let mut data_dir = get_data_directory(Some("chat_commands"))?;
    data_dir.push(command);

    let message = fs::read_to_string(data_dir)?;

    client.send_privmsg(&message);

    // Send message to display since this IRC client is the one posting the message
    // it won't display if its not sent directly to the TUI client for rendering
    client.display_msg(&message);

    Ok(())
}

fn print_message(message: &ChatMessage, client: &mut TwitchIRC) {
    let nickname = &message.nickname;

    let nick = nickname;
    let final_message = format!("{nick}: {}", message.message.trim());

    if message.first_msg {
        let first_time_msg = "✨ First Time Chat:".to_string();
        println!("{}", first_time_msg);
    }

    println!("{final_message}");

    check_for_chat_commands(&message.message, client);
}
