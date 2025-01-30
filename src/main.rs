use anyhow::bail;
use cli::command;

mod announcements;
mod channel;
mod chat_commands;
mod cli;
mod fs;
mod image_protocols;
mod twitch;

fn main() -> anyhow::Result<()> {
    match command() {
        Ok(response) => {
            println!("command() returned?... {response:?}");
            Ok(())
        }
        Err(error) => {
            bail!("Mistakes were made... {error}")
        }
    }
}
