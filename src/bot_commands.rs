use std::sync::Arc;

use anyhow::{anyhow, Context};
use log::debug;
use teloxide::{
    dispatching::{
        dialogue::{self, InMemStorage},
        UpdateHandler,
    },
    prelude::*,
    types::ParseMode,
    utils::command::BotCommands,
};
use tokio::sync::Mutex;

use crate::bot_logic::compute_exchanges;
use crate::formatter::{format_balance, format_list_expenses};
use crate::memory::sqlite::SqlLiteMemory;
use crate::memory::Memory;
use crate::parser::parse_expense;
use crate::validator::validate_expense;

#[derive(Clone, Default)]
pub enum State {
    #[default]
    Normal,
}

#[derive(BotCommands, Clone)]
#[command(
    rename_rule = "lowercase",
    description = "This bot keeps track of debts and credits in a group. Supported commands:"
)]
enum Command {
    #[command(description = "shows this message.")]
    Help,
    #[command(description = "add a new expense; format: aa 34.4 bb cc")]
    Expense(String),
    #[command(description = "shortcut for the /expense command")]
    E(String),
    #[command(description = "print the current balance.")]
    Balance,
    #[command(description = "mark all expenses as settled.")]
    Reset,
    #[command(
        description = "/list n shows the last n expenses; without argument, it shows the last one."
    )]
    List(String),
    #[command(
        description = "/delete <id> deletes the expense with the given ID; to find the ID, use /list."
    )]
    Delete(String),
}

type HandlerResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;

// We would like to take this as parameter of dialogue_handler, but probably in Rust you
// cannot pass a type as a parameter. So we define it as a type alias instead.
// If the correct type of memory is not provided, the thread will panic at runtime during message
// handling.
type MemoryInUse = Arc<Mutex<SqlLiteMemory>>;

pub fn dialogue_handler() -> UpdateHandler<Box<dyn std::error::Error + Send + Sync + 'static>> {
    use dptree::case;

    let command_handler =
        teloxide::filter_command::<Command, _>().branch(case![State::Normal].endpoint(
            |msg: Message, bot: Bot, cmd: Command, memory: MemoryInUse| async move {
                let result = match cmd {
                    Command::Help => handle_help(&bot, &msg).await,
                    Command::Expense(e) => handle_expense(&msg, &memory, &e).await,
                    Command::E(e) => handle_expense(&msg, &memory, &e).await,
                    Command::Balance => handle_balance(&bot, &msg, &memory).await,
                    Command::Reset => handle_reset(&msg, &memory).await,
                    Command::List(limit) => handle_list(&bot, &msg, &memory, &limit).await,
                    Command::Delete(id) => handle_delete(&msg, &memory, &id).await,
                };

                if result.is_err() {
                    bot.send_message(msg.chat.id, "An error occurred!").await?;
                }

                // teloxide default error handler will take care of logging the result if it is an error.
                result
            },
        ));

    let message_handler = Update::filter_message().branch(command_handler);

    dialogue::enter::<Update, InMemStorage<State>, State, _>().branch(message_handler)
}

async fn handle_help(bot: &Bot, msg: &Message) -> HandlerResult {
    bot.send_message(msg.chat.id, Command::descriptions().to_string())
        .await?;
    Ok(())
}

async fn handle_expense<M: Memory>(
    msg: &Message,
    memory: &Arc<Mutex<M>>,
    message: &str,
) -> HandlerResult {
    let chat_id = msg.chat.id.0;
    let message_ts = msg.date;

    let expense = parse_expense(message).map_err(|e| {
        anyhow!(
            "Cannot parse input message '{}'. Error: {}",
            message.to_string(),
            e.to_string()
        )
    })?;
    let expense = expense.1;
    validate_expense(&expense)?;

    memory
        .lock()
        .await
        .save_expense_with_message(chat_id, expense, message_ts)?;

    Ok(())
}

async fn handle_balance<M: Memory>(
    bot: &Bot,
    msg: &Message,
    memory: &Arc<Mutex<M>>,
) -> HandlerResult {
    let chat_id = msg.chat.id.0;

    let active_expenses = memory
        .lock()
        .await
        .get_active_expenses(chat_id)
        .with_context(|| "Could not get active expenses")?;
    let exchanges = compute_exchanges(active_expenses);
    let formatted_balance = format_balance(&exchanges);

    bot.send_message(msg.chat.id, formatted_balance)
        .parse_mode(ParseMode::MarkdownV2)
        .await?;
    Ok(())
}

async fn handle_reset<M: Memory>(msg: &Message, memory: &Arc<Mutex<M>>) -> HandlerResult {
    let chat_id = msg.chat.id.0;

    memory
        .lock()
        .await
        .mark_all_as_settled(chat_id)
        .with_context(|| "Could not mark all as settled")?;
    Ok(())
}

async fn handle_list<M: Memory>(
    bot: &Bot,
    msg: &Message,
    memory: &Arc<Mutex<M>>,
    limit: &str,
) -> HandlerResult {
    let chat_id = msg.chat.id.0;
    let limit = if limit.is_empty() { 1 } else { limit.parse()? };
    debug!("Producing the list of expenses with limit {}", limit);

    // Considering how we run the query, it is not easy to use a LIMIT, so
    // we ask for everything and just slice the result.
    let mut active_expenses = memory
        .lock()
        .await
        .get_active_expenses(chat_id)
        .with_context(|| "Could not get active expenses")?;

    active_expenses.sort_by(|e1, e2| e2.id.partial_cmp(&e1.id).unwrap());
    let limit = std::cmp::min(limit, active_expenses.len());
    let result = format_list_expenses(&active_expenses[0..limit]);

    bot.send_message(msg.chat.id, result)
        .parse_mode(ParseMode::MarkdownV2)
        .await?;

    Ok(())
}

async fn handle_delete<M: Memory>(
    msg: &Message,
    memory: &Arc<Mutex<M>>,
    expense_id: &str,
) -> HandlerResult {
    let chat_id = msg.chat.id.0;
    let expense_id = expense_id.parse()?;

    memory.lock().await.delete_expense(chat_id, expense_id)?;
    Ok(())
}
