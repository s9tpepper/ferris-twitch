use std::fs;

use crate::fs::get_data_directory;

pub fn list_commands() -> anyhow::Result<()> {
    let list = get_list_commands()?;

    if list.is_empty() {
        println!("Currently no chat announcements have been added.");
        return Ok(());
    }

    println!("Available chat commands:");

    list.iter().for_each(|item| println!("- {item}"));

    list_announcements()
}

fn list_announcements() -> anyhow::Result<()> {
    let list = get_list_announcements()?;

    if list.is_empty() {
        println!("Currently no chat announcements have been added.");
        return Ok(());
    }

    println!("Current chat announcements:");
    list.iter().for_each(|item| println!("- {item}"));

    Ok(())
}

pub fn get_list_announcements() -> anyhow::Result<Vec<String>> {
    let command_path = get_data_directory(Some("chat_announcements"))?;
    let mut commands = vec![];
    let dir_entries = fs::read_dir(command_path)?;

    for entry in dir_entries {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            let file_name = path.file_name();
            let Some(file_name) = file_name else {
                continue;
            };

            commands.push(file_name.to_string_lossy().to_string());
        }
    }

    Ok(commands)
}

pub fn get_list_commands() -> anyhow::Result<Vec<String>> {
    get_list("chat_commands")
}

pub fn get_list(directory: &str) -> anyhow::Result<Vec<String>> {
    let command_path = get_data_directory(Some(directory))?;
    let mut commands = vec![];
    let dir_entries = fs::read_dir(command_path)?;

    for entry in dir_entries {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            let file_name = path.file_name();
            let Some(file_name) = file_name else {
                continue;
            };

            commands.push(file_name.to_string_lossy().to_string());
        }
    }

    Ok(commands)
}

pub fn add_command(command_name: &str, message: &str, timing: Option<usize>) -> anyhow::Result<()> {
    let mut target_dir = "chat_commands";

    let file_contents = match timing {
        Some(timing) => {
            target_dir = "chat_announcements";
            format!("{}\n{}", timing, message)
        }

        None => message.to_string(),
    };

    let mut command_path = get_data_directory(Some(target_dir))?;

    if !command_path.exists() {
        std::fs::create_dir_all(&command_path)?;
    }

    command_path.push(command_name);
    fs::write(command_path, file_contents)?;

    Ok(())
}

pub fn remove_command(command_name: &str) -> anyhow::Result<()> {
    let mut command_path = get_data_directory(Some("chat_commands"))?;
    command_path.push(command_name);
    if command_path.exists() {
        return Ok(fs::remove_file(command_path)?);
    }

    let mut command_path = get_data_directory(Some("chat_announcements"))?;
    command_path.push(command_name);
    if command_path.exists() {
        return Ok(fs::remove_file(command_path)?);
    }

    Ok(())
}

pub fn list_actions() -> anyhow::Result<()> {
    list("irc_actions")
}

fn list(list_type: &str) -> anyhow::Result<()> {
    let list = get_list(&list_type)?;
    let human_readable = list_type.replace('_', " ");
    if list.is_empty() {
        println!("Currently no {human_readable} have been added.");
    }

    println!("Available {human_readable}:");
    list.iter().for_each(|item| println!("- {}", item));

    Ok(())
}

pub fn add_action(action_name: &str, cli: &str) -> anyhow::Result<()> {
    add_item(action_name, cli, "irc_actions")
}

pub fn add_item(item_name: &str, cli: &str, item_type: &str) -> anyhow::Result<()> {
    let file_contents = cli.to_string();

    let mut item_path = get_data_directory(Some(item_type))?;

    if !item_path.exists() {
        std::fs::create_dir_all(&item_path)?;
    }

    item_path.push(item_name);

    fs::write(item_path, file_contents)?;

    Ok(())
}

pub fn remove_action(action_name: &str) -> anyhow::Result<()> {
    remove_item(action_name, "irc_actions")
}

pub fn remove_item(item_name: &str, item_type: &str) -> anyhow::Result<()> {
    let mut item_path = get_data_directory(Some(item_type))?;
    item_path.push(item_name);

    if item_path.exists() {
        return Ok(fs::remove_file(item_path)?);
    }

    Ok(())
}

pub fn list_rewards() -> anyhow::Result<()> {
    list("chat_rewards")
}

pub fn add_reward(reward_name: &str, cli: &str) -> anyhow::Result<()> {
    add_item(reward_name, cli, "chat_rewards")
}

pub fn remove_reward(reward_name: &str) -> anyhow::Result<()> {
    remove_item(reward_name, "chat_rewards")
}
