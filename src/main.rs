use std::sync::Arc;

use log::{error, info};
use log4rs::{
    append::rolling_file::{
        policy::compound::{
            roll::fixed_window::FixedWindowRoller, trigger::size::SizeTrigger, CompoundPolicy,
        },
        RollingFileAppender,
    },
    config::{Appender, Config, Root},
    encode::pattern::PatternEncoder,
};
use teloxide::dispatching::dialogue::InMemStorage;
use teloxide::prelude::*;
use tokio::{
    sync::Mutex,
    time::{interval, Duration},
};

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
    init_log();

    spawn_background_health_log();

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

/// WORKAROUND: This is an attempt at preventing the OS from killing the bot
/// after a while when it is inactive for too long.
fn spawn_background_health_log() {
    tokio::spawn(async {
        // Create an interval timer that ticks every 3 hours.
        let mut interval = interval(Duration::from_secs(3 * 60 * 60));

        loop {
            // Wait for the next tick.
            interval.tick().await;
            // Log bot health.
            info!("Bot is healthy");
        }
    });
}

fn init_log() {
    // Create a trigger that rolls the log file when it exceeds 10 MB.
    let size_trigger = SizeTrigger::new(10 * 1024 * 1024);

    // Create a roller that keeps up to 2 backup log files with a pattern.
    let fixed_window_roller = FixedWindowRoller::builder()
        .build("log/treasurer.{}.log", 2)
        .expect("[init log] Cannot create fixed window roller");

    // Combine trigger and roller into a compound policy.
    let compound_policy =
        CompoundPolicy::new(Box::new(size_trigger), Box::new(fixed_window_roller));

    // Create a rolling file appender.
    let rolling_file_appender = RollingFileAppender::builder()
        .encoder(Box::new(PatternEncoder::new("{d} - {l} - {m}{n}")))
        .build("log/treasurer.log", Box::new(compound_policy))
        .expect("[init log] Cannot create rolling file appender");

    // Create the configuration.
    let config = Config::builder()
        .appender(Appender::builder().build("rolling_file", Box::new(rolling_file_appender)))
        .build(
            Root::builder()
                .appender("rolling_file")
                .build(log::LevelFilter::Info),
        )
        .expect("[init log] Cannot build config");

    // Initialize log4rs with the configuration
    log4rs::init_config(config).expect("[init log] Cannot init log4rs");
}
