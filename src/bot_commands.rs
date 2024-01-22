//! Definition of Telegram bot commands and handlers.

use std::{collections::HashMap, sync::Arc};

use log::{debug, info};
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

use crate::{bot_logic::compute_exchanges, error::BotError};
use crate::{
    formatter::{format_balance, format_list_expenses, format_simple_list},
    memory::sqlite::SqliteMemory,
};
use crate::{memory::Memory, validator::validate_participants_exist};
use crate::{
    parser::{parse_expense, parse_group_and_members, parse_participants},
    validator::validate_group_exists,
};
use crate::{
    types::ParsedExpense,
    validator::{validate_and_resolve_groups, validate_expense},
};

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
    #[command(
        description = "adds a new expense; format: participant1 34.4 participant2 participant3"
    )]
    Expense(String),
    #[command(description = "shortcut for the /expense command")]
    E(String),
    #[command(description = "prints the current balance.")]
    Balance,
    #[command(description = "marks all expenses as settled.")]
    Reset,
    #[command(
        description = "/list n shows the last n expenses; without argument, it shows the last one."
    )]
    List(String),
    #[command(
        description = "/delete <id> deletes the expense with the given ID; to find the ID, use /list."
    )]
    Delete(String),
    #[command(
        description = "/addparticipants participant1 participant2 adds participants that can be \
                       used as creditors or debtors in expenses."
    )]
    AddParticipants(String),
    #[command(
        description = "/removeparticipants participant1 participant2 removes participants that should \
                       not appear in expenses anymore (they are not removed from older expenses)."
    )]
    RemoveParticipants(String),
    #[command(
        description = "returns the list of all registered participants (only registered participants can \
                       appear in expenses)."
    )]
    ListParticipants,
    #[command(
        description = "/addgroup group_name member1 member2 creates a group with two members."
    )]
    AddGroup(String),
    #[command(description = "/deletegroup group_name deletes a group, no questions asked.")]
    DeleteGroup(String),
    #[command(
        description = "/addgroupmembers group_name member1 member2 adds two members to a group if not already present."
    )]
    AddGroupMembers(String),
    #[command(
        description = "/removegroupmembers group_name member1 member2 removes two members from a group if present."
    )]
    RemoveGroupMembers(String),
    #[command(description = "returns the list of all existing groups.")]
    ListGroups,
    #[command(description = "returns the list of all members of the given group.")]
    ListGroupMembers(String),
}

type HandlerResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;

// We would like to take this as parameter of dialogue_handler, but probably in Rust you
// cannot pass a type as a parameter. So we define it as a type alias instead.
// If the correct type of memory is not provided, the thread will panic at runtime during message
// handling.
type MemoryInUse = Arc<Mutex<SqliteMemory>>;

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
                    Command::AddParticipants(s) => handle_add_participants(&msg, &memory, &s).await,
                    Command::RemoveParticipants(s) => {
                        handle_remove_participants(&msg, &memory, &s).await
                    }
                    Command::ListParticipants => {
                        handle_list_participants(&bot, &msg, &memory).await
                    }
                    Command::AddGroup(s) => handle_add_group(&msg, &memory, &s).await,
                    Command::DeleteGroup(group_name) => {
                        handle_delete_group(&msg, &memory, &group_name).await
                    }
                    Command::AddGroupMembers(s) => {
                        handle_add_group_members(&msg, &memory, &s).await
                    }
                    Command::RemoveGroupMembers(s) => {
                        handle_remove_group_members(&msg, &memory, &s).await
                    }
                    Command::ListGroups => handle_list_groups(&bot, &msg, &memory).await,
                    Command::ListGroupMembers(group_name) => {
                        handle_list_group_members(&bot, &msg, &memory, &group_name).await
                    }
                };

                if result.is_err() {
                    let e = result.as_ref().expect_err("just checked this is an error!");
                    bot.send_message(msg.chat.id, format!("{e}"))
                        .await
                        .map_err(|e| BotError::telegram("cannot send error message", e))?;
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
        .await
        .map_err(|e| BotError::telegram("cannot send help", e))?;
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
        BotError::nom_parse(&format!("cannot parse input message '{}'", message,), e)
    })?;
    let expense = expense.1;
    let expense = validate_and_resolve_groups(expense, chat_id, memory).await?;
    validate_expense(&expense, chat_id, memory).await?;

    let expense = normalize_participants(expense);

    memory
        .lock()
        .await
        .save_expense_with_message(chat_id, expense, message_ts)
        .map_err(|e| BotError::database("cannot save expense", e))?;

    Ok(())
}

/// Make sure that each participant appears at most once as debtor and
/// at most once as creditor (so at most twice in total).
///
/// If a participant has a custom amount, make sure to preserve it.
fn normalize_participants(expense: ParsedExpense) -> ParsedExpense {
    let mut participants = HashMap::new();

    for participant in expense.participants {
        if participants.contains_key(&(participant.name.clone(), participant.is_creditor())) {
            if participant.amount.is_some() {
                // If a participant has a custom amount, it supersedes any mention without.
                // Note that this can only happen once because we have already validated the expense.
                participants.insert(
                    (participant.name.clone(), participant.is_creditor()),
                    participant,
                );
            }
        } else {
            // We insert the participant if not already present.
            participants.insert(
                (participant.name.clone(), participant.is_creditor()),
                participant,
            );
        }
    }

    ParsedExpense::new(
        participants.into_iter().map(|(_, p)| p).collect(),
        expense.amount,
        expense.message,
    )
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
        .map_err(|e| BotError::database("cannot get active expenses", e))?;
    let exchanges = compute_exchanges(active_expenses);
    let formatted_balance = format_balance(&exchanges);

    bot.send_message(msg.chat.id, formatted_balance)
        .parse_mode(ParseMode::MarkdownV2)
        .await
        .map_err(|e| BotError::telegram("cannot send balance", e))?;
    Ok(())
}

async fn handle_reset<M: Memory>(msg: &Message, memory: &Arc<Mutex<M>>) -> HandlerResult {
    let chat_id = msg.chat.id.0;

    memory
        .lock()
        .await
        .mark_all_as_settled(chat_id)
        .map_err(|e| BotError::database("cannot mark all as settled", e))?;
    Ok(())
}

async fn handle_list<M: Memory>(
    bot: &Bot,
    msg: &Message,
    memory: &Arc<Mutex<M>>,
    limit: &str,
) -> HandlerResult {
    let chat_id = msg.chat.id.0;
    let limit = if limit.is_empty() {
        1
    } else {
        limit.parse().map_err(|e| {
            BotError::new(
                format!("cannot parse limit '{}': {}", limit, e),
                "cannot parse integer".to_string(),
            )
        })?
    };
    debug!("Producing the list of expenses with limit {}", limit);

    // Considering how we run the query, it is not easy to use a LIMIT, so
    // we ask for everything and just slice the result.
    let mut active_expenses = memory
        .lock()
        .await
        .get_active_expenses(chat_id)
        .map_err(|e| BotError::database("cannot get active expenses", e))?;

    active_expenses.sort_by(|e1, e2| {
        e2.id
            .partial_cmp(&e1.id)
            .expect("cannot sort active expenses")
    });
    let limit = std::cmp::min(limit, active_expenses.len());
    let result = format_list_expenses(&active_expenses[0..limit]);

    bot.send_message(msg.chat.id, result)
        .parse_mode(ParseMode::MarkdownV2)
        .await
        .map_err(|e| BotError::telegram("cannot send expense list", e))?;

    Ok(())
}

async fn handle_delete<M: Memory>(
    msg: &Message,
    memory: &Arc<Mutex<M>>,
    expense_id: &str,
) -> HandlerResult {
    let chat_id = msg.chat.id.0;
    let expense_id = expense_id.parse().map_err(|e| {
        BotError::new(
            format!("cannot parse expense ID '{}': {}", expense_id, e),
            "cannot parse integer".to_string(),
        )
    })?;

    memory
        .lock()
        .await
        .delete_expense(chat_id, expense_id)
        .map_err(|e| BotError::database("cannot delete expense", e))?;
    Ok(())
}

async fn handle_add_participants<M: Memory>(
    msg: &Message,
    memory: &Arc<Mutex<M>>,
    payload: &str,
) -> HandlerResult {
    let chat_id = msg.chat.id.0;
    let participants = parse_participants(payload)?;
    debug!("Adding participants: {:#?}", participants);
    memory
        .lock()
        .await
        .add_participants_if_not_exist(chat_id, &participants)
        .map_err(|e| BotError::database("cannot add participants", e))?;
    Ok(())
}

async fn handle_remove_participants<M: Memory>(
    msg: &Message,
    memory: &Arc<Mutex<M>>,
    payload: &str,
) -> HandlerResult {
    let chat_id = msg.chat.id.0;
    let participants = parse_participants(payload)?;
    debug!("Removing participants: {:#?}", participants);
    memory
        .lock()
        .await
        .remove_participants_if_exist(chat_id, &participants)
        .map_err(|e| BotError::database("cannot remove participants", e))?;
    Ok(())
}

async fn handle_list_participants<M: Memory>(
    bot: &Bot,
    msg: &Message,
    memory: &Arc<Mutex<M>>,
) -> HandlerResult {
    let chat_id = msg.chat.id.0;
    let participants = memory
        .lock()
        .await
        .get_participants(chat_id)
        .map_err(|e| BotError::database("cannot get participants", e))?;

    let result = format_simple_list(&participants);

    bot.send_message(msg.chat.id, result)
        .await
        .map_err(|e| BotError::telegram("cannot send participant list", e))?;

    Ok(())
}

async fn handle_add_group<M: Memory>(
    msg: &Message,
    memory: &Arc<Mutex<M>>,
    payload: &str,
) -> HandlerResult {
    let chat_id = msg.chat.id.0;
    let (group_name, members) = parse_group_and_members(payload)?;
    debug!(
        "Creating group named {group_name} with members: {:#?}",
        members
    );

    validate_participants_exist(&members, chat_id, memory).await?;

    memory
        .lock()
        .await
        .create_group_if_not_exists(chat_id, &group_name)
        .map_err(|e| BotError::database("cannot create group", e))?;

    if !members.is_empty() {
        memory
            .lock()
            .await
            .add_group_members_if_not_exist(chat_id, &group_name, &members)
            .map_err(|e| BotError::database("cannot add group members", e))?;
    }
    Ok(())
}

async fn handle_delete_group<M: Memory>(
    msg: &Message,
    memory: &Arc<Mutex<M>>,
    group_name: &str,
) -> HandlerResult {
    let chat_id = msg.chat.id.0;
    info!("Deleting group named {group_name}");

    validate_group_exists(group_name, chat_id, memory).await?;

    memory
        .lock()
        .await
        .delete_group_if_exists(chat_id, group_name)
        .map_err(|e| BotError::database("cannot delete group", e))?;

    Ok(())
}

async fn handle_add_group_members<M: Memory>(
    msg: &Message,
    memory: &Arc<Mutex<M>>,
    payload: &str,
) -> HandlerResult {
    let chat_id = msg.chat.id.0;
    let (group_name, members) = parse_group_and_members(payload)?;
    debug!(
        "Adding group members to group named {group_name}. Members: {:#?}",
        members
    );

    validate_group_exists(&group_name, chat_id, memory).await?;
    validate_participants_exist(&members, chat_id, memory).await?;

    memory
        .lock()
        .await
        .add_group_members_if_not_exist(chat_id, &group_name, &members)
        .map_err(|e| BotError::database("cannot add group members", e))?;

    Ok(())
}

async fn handle_remove_group_members<M: Memory>(
    msg: &Message,
    memory: &Arc<Mutex<M>>,
    payload: &str,
) -> HandlerResult {
    let chat_id = msg.chat.id.0;
    let (group_name, members) = parse_group_and_members(payload)?;

    validate_group_exists(&group_name, chat_id, memory).await?;
    validate_participants_exist(&members, chat_id, memory).await?;

    memory
        .lock()
        .await
        .remove_group_members_if_exist(chat_id, &group_name, &members)
        .map_err(|e| BotError::database("cannot remove group members", e))?;

    Ok(())
}

async fn handle_list_groups<M: Memory>(
    bot: &Bot,
    msg: &Message,
    memory: &Arc<Mutex<M>>,
) -> HandlerResult {
    let chat_id = msg.chat.id.0;
    let groups = memory
        .lock()
        .await
        .get_groups(chat_id)
        .map_err(|e| BotError::database("cannot get groups", e))?;

    let result = format_simple_list(&groups);

    bot.send_message(msg.chat.id, result)
        .await
        .map_err(|e| BotError::telegram("cannot send group list", e))?;

    Ok(())
}

async fn handle_list_group_members<M: Memory>(
    bot: &Bot,
    msg: &Message,
    memory: &Arc<Mutex<M>>,
    group_name: &str,
) -> HandlerResult {
    let chat_id = msg.chat.id.0;

    validate_group_exists(group_name, chat_id, memory).await?;

    let members = memory
        .lock()
        .await
        .get_group_members(chat_id, group_name)
        .map_err(|e| BotError::database("cannot get group members", e))?;

    let result = format_simple_list(&members);
    bot.send_message(msg.chat.id, result)
        .await
        .map_err(|e| BotError::telegram("cannot send group member list", e))?;

    Ok(())
}
