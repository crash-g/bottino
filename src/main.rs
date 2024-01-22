use std::sync::Arc;

use log::{error, info};
use teloxide::dispatching::dialogue::InMemStorage;
use teloxide::prelude::*;
use tokio::sync::Mutex;

mod bot_commands;
mod bot_logic;
mod formatter;
mod memory;
mod parser;
mod types;
mod validator;

use crate::bot_commands::{dialogue_handler, State};
use crate::memory::sqlite::SqliteMemory;

#[tokio::main]
async fn main() {
    pretty_env_logger::init_timed();

    info!("Initiliazing database...");
    let memory = SqliteMemory::new()
        .map_err(|e| error!("Cannot initialize database: {}", e))
        .expect("Cannot initialize database");

    let memory = Arc::new(Mutex::new(memory));

    info!("Starting command bot...");

    let bot = Bot::from_env();

    Dispatcher::builder(bot, dialogue_handler())
        .dependencies(dptree::deps![InMemStorage::<State>::new(), memory])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;
}
