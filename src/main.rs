use cli::command;

mod chat_commands;
mod cli;
mod fs;
mod image_protocols;
mod twitch;

fn main() -> anyhow::Result<()> {
    command()
}
