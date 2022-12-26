use std::sync::Arc;

use log::{error, info};
use teloxide::dispatching::dialogue::InMemStorage;
use teloxide::prelude::*;
use tokio::sync::Mutex;

mod bot_commands;
mod bot_logic;
mod database;
mod endpoints;
mod error;
mod formatter;
mod parser;
mod types;
mod validator;

use crate::bot_commands::{dialogue_handler, State};
use crate::database::sqlite::SqliteDatabase;

#[tokio::main]
async fn main() {
    pretty_env_logger::init_timed();

    info!("Initiliazing database...");
    let database = SqliteDatabase::new("treasurer.db")
        .map_err(|e| error!("Cannot initialize database: {}", e))
        .expect("Cannot initialize database");

    let database = Arc::new(Mutex::new(database));

    info!("Starting command bot...");

    let bot = Bot::from_env();

    Dispatcher::builder(bot, dialogue_handler())
        .dependencies(dptree::deps![InMemStorage::<State>::new(), database])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;
}
