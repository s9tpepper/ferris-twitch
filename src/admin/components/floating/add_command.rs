use std::collections::HashMap;

use anathema::{
    component::{Component, ComponentId, KeyCode},
    prelude::TuiBackend,
    runtime::{Error, RuntimeBuilder},
    state::{CommonVal, State, Value},
};

use crate::{
    admin::{
        components::{
            app::{AppMessageHandler, FloatingWindow},
            MessageSender,
        },
        messages::{CommandsViewReload, ComponentMessages},
        templates::ADD_COMMAND_TEMPLATE,
        AppComponent,
    },
    commands::add_chat_command,
};

#[derive(Default)]
pub struct AddCommand;

impl AddCommand {
    pub fn register(
        builder: &mut RuntimeBuilder<TuiBackend, ()>,
        component_ids: &mut HashMap<String, ComponentId<String>>,
    ) {
        <crate::admin::components::floating::add_command::AddCommand as AppComponent>::register_component(
            builder,
            "add_command_window",
            ADD_COMMAND_TEMPLATE,
            AddCommand,
            AddCommandState::new(),
            component_ids,
        )
    }
}

impl AppComponent for AddCommand {}

impl AppMessageHandler for AddCommand {
    fn handle_message<F>(
        value: CommonVal<'_>,
        ident: impl Into<String>,
        state: &mut crate::admin::components::app::AppState,
        context: anathema::prelude::Context<'_, crate::admin::components::app::AppState>,
        component_ids: &HashMap<String, ComponentId<String>>,
        fun: F,
    ) where
        F: Fn(
            &mut crate::admin::components::app::AppState,
            anathema::prelude::Context<'_, crate::admin::components::app::AppState>,
        ),
    {
        let event: String = ident.into();
        match event.as_str() {
            "add_command__cancel" => {
                fun(state, context);
            }

            "add_command__submit" => {
                let command: Command = value.into();

                match add_chat_command(&command.name.to_ref(), &command.output.to_ref(), None) {
                    Ok(_) => {
                        if let Some(id) = component_ids.get("commands_view") {
                            let _ = MessageSender::send_message(
                                *id,
                                ComponentMessages::CommandsViewReload(CommandsViewReload {}),
                                context.emitter.clone(),
                            );
                        }
                    }

                    Err(_) => {
                        // TODO: bring up a message window with an error message
                    }
                };

                fun(state, context);
            }

            _ => {}
        }
    }
}

#[derive(Default, State)]
pub struct AddCommandState {
    command: Value<Command>,
}

#[derive(Default, Debug)]
pub struct Command {
    pub name: Value<String>,
    pub output: Value<String>,
    pub common: Value<String>,
}

impl State for Command {
    fn to_common(&self) -> Option<CommonVal<'_>> {
        let str = self.common.to_ref().to_string().clone().into_boxed_str();

        Some(CommonVal::Str(Box::leak(str)))
    }
}

impl From<CommonVal<'_>> for Command {
    fn from(value: CommonVal) -> Self {
        if let Some((name, output)) = value.to_string().split_once("::::") {
            return Command {
                name: name.to_string().into(),
                output: output.to_string().into(),
                common: format!("{}::::{}", name, output).into(),
            };
        }

        Command {
            name: String::from("").into(),
            output: String::from("").into(),
            common: String::from("::::").into(),
        }
    }
}

impl AddCommandState {
    pub fn new() -> Self {
        AddCommandState {
            command: Command {
                name: String::from("").into(),
                output: String::from("").into(),
                common: String::from("::::").into(),
            }
            .into(),
        }
    }
}

impl Component for AddCommand {
    type State = AddCommandState;
    type Message = String;

    fn accept_focus(&self) -> bool {
        true
    }

    fn receive(
        &mut self,
        ident: &str,
        value: anathema::state::CommonVal<'_>,
        state: &mut Self::State,
        _: anathema::widgets::Elements<'_, '_>,
        mut context: anathema::prelude::Context<'_, Self::State>,
    ) {
        match ident {
            "name_update" => {
                state.command.to_mut().name.set(value.to_string());

                let common = format!("{}::::{}", value, *state.command.to_ref().output.to_ref());
                state.command.to_mut().common.set(common);
            }

            "output_update" => {
                state.command.to_mut().output.set(value.to_string());

                let common = format!("{}::::{}", *state.command.to_ref().name.to_ref(), value);
                state.command.to_mut().common.set(common);
            }

            "name_focus_change" => {
                context.set_focus("id", "add_command_window");
            }

            "output_focus_change" => {
                context.set_focus("id", "add_command_window");
            }

            _ => {}
        }
    }

    fn on_key(
        &mut self,
        key: anathema::component::KeyEvent,
        _: &mut Self::State,
        _: anathema::widgets::Elements<'_, '_>,
        mut context: anathema::prelude::Context<'_, Self::State>,
    ) {
        match key.code {
            KeyCode::Char(char) => match char {
                's' => {
                    context.publish("submit_add_command", |state| &state.command);
                }

                'c' => context.publish("cancel_add_command", |state| &state.command),

                'n' => context.set_focus("id", "command_name_input"),

                'o' => context.set_focus("id", "command_output_input"),

                _ => {}
            },

            KeyCode::Esc => context.publish("cancel_add_command", |state| &state.command),

            _ => {}
        }
    }
}
