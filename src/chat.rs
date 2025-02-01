use anathema::{prelude::*, state::List};

use app::{App, AppState};
use log::{error, info};

use std::sync::mpsc::Receiver;

use crate::channel::ChannelMessages;

mod app;

pub fn start_chat_frontend(tui_receiver: Receiver<ChannelMessages>) -> anyhow::Result<()> {
    info!("App::run()");

    let tui = TuiBackend::builder()
        .enable_alt_screen()
        .enable_raw_mode()
        .hide_cursor()
        .finish();

    match tui {
        Ok(tui_backend) => {
            let doc = Document::new("@app");
            let mut runtime_builder = Runtime::builder(doc, tui_backend);

            runtime_builder.register_component(
                "app",
                "src/chat/templates/app.aml",
                App::new(tui_receiver),
                AppState {
                    log: List::from_iter([]),
                    test_field: "This is a string".to_string().into(),
                },
            )?;

            let mut runtime = runtime_builder.finish().unwrap();
            runtime.run();

            // runtime_builder.finish(tui_backend.size(), |runtime| runtime.run(tui_backend))?;
        }

        Err(error) => {
            error!("Error starting TUI {error}");
            panic!("Error starting TUI {error}");
        }
    };

    Ok(())
}
