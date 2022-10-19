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
use crate::memory::sqlite::SqlLiteMemory;

#[tokio::main]
async fn main() {
    pretty_env_logger::init_timed();

    info!("Initiliazing database...");
    SqlLiteMemory::new()
        .map_err(|e| error!("Cannot initialize database: {}", e))
        .expect("Cannot initialize database");

    info!("Creating database connection...");
    let connection = SqlLiteMemory::connection()
        .map_err(|e| error!("Cannot create database connection: {}", e))
        .expect("Cannot create database connection");
    let connection = Arc::new(Mutex::new(connection));

    info!("Starting command bot...");

    let bot = Bot::from_env();

    Dispatcher::builder(bot, dialogue_handler())
        .dependencies(dptree::deps![InMemStorage::<State>::new(), connection])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;
}
