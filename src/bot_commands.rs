//! Definition of Telegram bot commands and handlers.

use std::{
    collections::{hash_map::Entry, HashMap},
    sync::Arc,
};

use log::{debug, error};
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

use crate::{
    bot_logic::compute_exchanges,
    error::{DatabaseError, InputError, TelegramError},
    validator::{validate_group_name, validate_participant_names},
};
use crate::{
    database::sqlite::SqliteDatabase,
    formatter::{format_balance, format_list_expenses, format_simple_list},
};
use crate::{database::Database, validator::validate_participants_exist};
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
    #[command(description = "shortcut for the /addparticipants command")]
    Ap(String),
    #[command(
        description = "/removeparticipants participant1 participant2 removes participants that should \
                       not appear in expenses anymore (they are not removed from older expenses)."
    )]
    RemoveParticipants(String),
    #[command(description = "shortcut for the /removeparticipants command")]
    Rp(String),
    #[command(
        description = "returns the list of all registered participants (only registered participants can \
                       appear in expenses)."
    )]
    ListParticipants,
    #[command(description = "shortcut for the /listparticipants command")]
    Lp,
    #[command(
        description = "/addgroup group_name member1 member2 creates a group with two members."
    )]
    AddGroup(String),
    #[command(description = "shortcut for the /addgroup command")]
    Ag(String),
    #[command(description = "/removegroup group_name removes a group, no questions asked.")]
    RemoveGroup(String),
    #[command(description = "shortcut for the /removegroup command")]
    Rg(String),
    #[command(
        description = "/addgroupmembers group_name member1 member2 adds two members to a group if not already present."
    )]
    AddGroupMembers(String),
    #[command(description = "shortcut for the /addgroupmembers command")]
    Agm(String),
    #[command(
        description = "/removegroupmembers group_name member1 member2 removes two members from a group if present."
    )]
    RemoveGroupMembers(String),
    #[command(description = "shortcut for the /removegroupmembers command")]
    Rgm(String),
    #[command(description = "returns the list of all existing groups.")]
    ListGroups,
    #[command(description = "shortcut for the /listgroups command")]
    Lg,
    #[command(description = "returns the list of all members of the given group.")]
    ListGroupMembers(String),
    #[command(description = "shortcut for the /listgroupmembers command")]
    Lgm(String),
    #[command(
        description = "toggle the auto register mode: when active the participants in an expense are
                             automatically registered as participants if they are not already"
    )]
    ToggleAutoRegister,
    #[command(description = "return whether auto register mode is active")]
    IsAutoRegister,
}

type HandlerResult = anyhow::Result<()>;

// We would like to take this as parameter of dialogue_handler, but probably in Rust you
// cannot pass a type as a parameter. So we define it as a type alias instead.
// If the correct type of database is not provided, the thread will panic at runtime during message
// handling.
type DatabaseInUse = Arc<Mutex<SqliteDatabase>>;

pub fn dialogue_handler() -> UpdateHandler<Box<dyn std::error::Error + Send + Sync + 'static>> {
    use dptree::case;

    let command_handler =
        teloxide::filter_command::<Command, _>().branch(case![State::Normal].endpoint(
            |msg: Message, bot: Bot, cmd: Command, database: DatabaseInUse| async move {
                use Command::*;
                let result = match cmd {
                    Help => handle_help(&bot, &msg).await,
                    Expense(e) | E(e) => handle_expense(&msg, &database, &e).await,
                    Balance => handle_balance(&bot, &msg, &database).await,
                    Reset => handle_reset(&msg, &database).await,
                    List(limit) => handle_list(&bot, &msg, &database, &limit).await,
                    Delete(id) => handle_delete(&msg, &database, &id).await,
                    AddParticipants(s) | Ap(s) => {
                        handle_add_participants(&msg, &database, &s).await
                    }
                    RemoveParticipants(s) | Rp(s) => {
                        handle_remove_participants(&msg, &database, &s).await
                    }
                    ListParticipants | Lp => handle_list_participants(&bot, &msg, &database).await,
                    AddGroup(group_name) | Ag(group_name) => handle_add_group(&msg, &database, &group_name).await,
                    RemoveGroup(group_name) | Rg(group_name) => {
                        handle_remove_group(&msg, &database, &group_name).await
                    }
                    AddGroupMembers(s) | Agm(s) => {
                        handle_add_group_members(&msg, &database, &s).await
                    }
                    RemoveGroupMembers(s) | Rgm(s) => {
                        handle_remove_group_members(&msg, &database, &s).await
                    }
                    ListGroups | Lg => handle_list_groups(&bot, &msg, &database).await,
                    ListGroupMembers(group_name) | Lgm(group_name) => {
                        handle_list_group_members(&bot, &msg, &database, &group_name).await
                    }
                    ToggleAutoRegister => handle_toggle_auto_register(&bot, &msg, &database).await,
                    IsAutoRegister => handle_is_auto_register(&bot, &msg, &database).await,
                };

                // We are basically bypassing teloxide error handler and managing errors here.
                // In the future it will be worth to explore if teloxide error handler can do everything we need:
                // - log with different levels depending on the error
                // - send a message to the user
                if let Err(e) = result {
                    if let Some(e) = e.downcast_ref::<InputError>() {
                        debug!("InputError in chat {}: {:#?}", msg.chat.id.0, e);
                    } else if let Some(e) = e.downcast_ref::<DatabaseError>() {
                        if e.is_concurrency_error() {
                            debug!("Concurrency error in chat {}: {:#?}", msg.chat.id.0, e);
                        } else {
                            error!("Database error in chat {}: {:#?}", msg.chat.id.0, e);
                        }
                    } else {
                        error!("Error in chat {}: {:#?}", msg.chat.id.0, e);
                    }
                    if let Err(e) = bot.send_message(msg.chat.id, format!("{e}")).await {
                        error!(
                            "Cannot send error message in chat {}: {:#?}",
                            msg.chat.id.0, e
                        );
                    }
                }

                Ok(())
            },
        ));

    let message_handler = Update::filter_message().branch(command_handler);

    dialogue::enter::<Update, InMemStorage<State>, State, _>().branch(message_handler)
}

async fn handle_help(bot: &Bot, msg: &Message) -> HandlerResult {
    bot.send_message(msg.chat.id, Command::descriptions().to_string())
        .await
        .map_err(|e| TelegramError::new("cannot send help", e))?;
    Ok(())
}

async fn handle_expense<D: Database>(
    msg: &Message,
    database: &Arc<Mutex<D>>,
    message: &str,
) -> HandlerResult {
    let chat_id = msg.chat.id.0;
    let message_ts = msg.date;

    let expense = parse_expense(message).map_err(InputError::invalid_expense_syntax)?;
    let expense = expense.1;
    let expense = validate_and_resolve_groups(expense, chat_id, database).await?;
    validate_expense(&expense)?;
    let expense = normalize_participants(expense);

    let participants: Vec<_> = expense.participants.iter().map(|p| &p.name).collect();
    if database.lock().await.is_auto_register_active(chat_id)? {
        database
            .lock()
            .await
            .add_participants_if_not_exist(chat_id, &participants)?;
    } else {
        validate_participants_exist(&participants, chat_id, database).await?;
    }

    database
        .lock()
        .await
        .save_expense_with_message(chat_id, expense, message_ts)?;

    Ok(())
}

/// Make sure that each participant appears at most once as debtor and
/// at most once as creditor (so at most twice in total).
///
/// If a participant has a custom amount, make sure to preserve it.
fn normalize_participants(expense: ParsedExpense) -> ParsedExpense {
    let mut participants = HashMap::new();

    for participant in expense.participants {
        match participants.entry((participant.name.clone(), participant.is_creditor())) {
            Entry::Occupied(mut e) => {
                if participant.amount.is_some() {
                    // If a participant has a custom amount, it supersedes any mention without.
                    // Note that this can only happen once because we have already validated the expense.
                    e.insert(participant);
                }
            }
            Entry::Vacant(e) => {
                // We insert the participant if not already present.
                e.insert(participant);
            }
        }
    }

    ParsedExpense::new(
        participants.into_iter().map(|(_, p)| p).collect(),
        expense.amount,
        expense.message,
    )
}

async fn handle_balance<D: Database>(
    bot: &Bot,
    msg: &Message,
    database: &Arc<Mutex<D>>,
) -> HandlerResult {
    let chat_id = msg.chat.id.0;

    let active_expenses = database.lock().await.get_active_expenses(chat_id)?;
    let exchanges = compute_exchanges(active_expenses);
    let formatted_balance = format_balance(&exchanges);

    bot.send_message(msg.chat.id, formatted_balance)
        .parse_mode(ParseMode::MarkdownV2)
        .await
        .map_err(|e| TelegramError::new("cannot send balance", e))?;
    Ok(())
}

async fn handle_reset<D: Database>(msg: &Message, database: &Arc<Mutex<D>>) -> HandlerResult {
    let chat_id = msg.chat.id.0;

    database.lock().await.mark_all_as_settled(chat_id)?;
    Ok(())
}

async fn handle_list<D: Database>(
    bot: &Bot,
    msg: &Message,
    database: &Arc<Mutex<D>>,
    limit: &str,
) -> HandlerResult {
    let chat_id = msg.chat.id.0;
    let limit = if limit.is_empty() {
        1
    } else {
        limit
            .parse()
            .map_err(|_| InputError::invalid_limit(limit.to_string()))?
    };
    debug!("Producing the list of expenses with limit {}", limit);

    let active_expenses = database
        .lock()
        .await
        .get_active_expenses_with_limit(chat_id, limit)?;
    let result = format_list_expenses(&active_expenses);

    bot.send_message(msg.chat.id, result)
        .parse_mode(ParseMode::MarkdownV2)
        .await
        .map_err(|e| TelegramError::new("cannot send expense list", e))?;

    Ok(())
}

async fn handle_delete<D: Database>(
    msg: &Message,
    database: &Arc<Mutex<D>>,
    expense_id: &str,
) -> HandlerResult {
    let chat_id = msg.chat.id.0;
    let expense_id = expense_id
        .parse()
        .map_err(|_| InputError::invalid_expense_id(expense_id.to_string()))?;

    database.lock().await.delete_expense(chat_id, expense_id)?;
    Ok(())
}

async fn handle_add_participants<D: Database>(
    msg: &Message,
    database: &Arc<Mutex<D>>,
    payload: &str,
) -> HandlerResult {
    let chat_id = msg.chat.id.0;
    let participants = parse_participants(payload)?;
    validate_participant_names(&participants)?;
    debug!("Adding participants: {:#?}", participants);
    database
        .lock()
        .await
        .add_participants_if_not_exist(chat_id, &participants)?;
    Ok(())
}

async fn handle_remove_participants<D: Database>(
    msg: &Message,
    database: &Arc<Mutex<D>>,
    payload: &str,
) -> HandlerResult {
    let chat_id = msg.chat.id.0;
    let participants = parse_participants(payload)?;
    validate_participant_names(&participants)?;
    debug!("Removing participants: {:#?}", participants);
    database
        .lock()
        .await
        .remove_participants_if_exist(chat_id, &participants)?;
    Ok(())
}

async fn handle_list_participants<D: Database>(
    bot: &Bot,
    msg: &Message,
    database: &Arc<Mutex<D>>,
) -> HandlerResult {
    let chat_id = msg.chat.id.0;
    let participants = database.lock().await.get_participants(chat_id)?;

    let result = format_simple_list(&participants);

    bot.send_message(msg.chat.id, result)
        .await
        .map_err(|e| TelegramError::new("cannot send participant list", e))?;

    Ok(())
}

async fn handle_add_group<D: Database>(
    msg: &Message,
    database: &Arc<Mutex<D>>,
    group_name: &str,
) -> HandlerResult {
    let chat_id = msg.chat.id.0;
    validate_group_name(group_name)?;
    debug!("Creating group named {group_name}");

    database
        .lock()
        .await
        .add_group_if_not_exists(chat_id, group_name)?;

    Ok(())
}

async fn handle_remove_group<D: Database>(
    msg: &Message,
    database: &Arc<Mutex<D>>,
    group_name: &str,
) -> HandlerResult {
    let chat_id = msg.chat.id.0;
    validate_group_name(group_name)?;
    debug!("Removing group named {group_name}");

    validate_group_exists(group_name, chat_id, database).await?;

    database
        .lock()
        .await
        .remove_group_if_exists(chat_id, group_name)?;

    Ok(())
}

async fn handle_add_group_members<D: Database>(
    msg: &Message,
    database: &Arc<Mutex<D>>,
    payload: &str,
) -> HandlerResult {
    let chat_id = msg.chat.id.0;
    let (group_name, members) = parse_group_and_members(payload)?;
    validate_group_name(&group_name)?;
    validate_participant_names(&members)?;
    debug!(
        "Adding group members to group named {group_name}. Members: {:#?}",
        members
    );

    validate_group_exists(&group_name, chat_id, database).await?;
    validate_participants_exist(&members, chat_id, database).await?;

    database
        .lock()
        .await
        .add_group_members_if_not_exist(chat_id, &group_name, &members)?;

    Ok(())
}

async fn handle_remove_group_members<D: Database>(
    msg: &Message,
    database: &Arc<Mutex<D>>,
    payload: &str,
) -> HandlerResult {
    let chat_id = msg.chat.id.0;
    let (group_name, members) = parse_group_and_members(payload)?;
    validate_group_name(&group_name)?;
    validate_participant_names(&members)?;
    debug!(
        "Deleting group members from group named {group_name}. Members: {:#?}",
        members
    );

    validate_group_exists(&group_name, chat_id, database).await?;
    validate_participants_exist(&members, chat_id, database).await?;

    database
        .lock()
        .await
        .remove_group_members_if_exist(chat_id, &group_name, &members)?;

    Ok(())
}

async fn handle_list_groups<D: Database>(
    bot: &Bot,
    msg: &Message,
    database: &Arc<Mutex<D>>,
) -> HandlerResult {
    let chat_id = msg.chat.id.0;
    let groups = database.lock().await.get_groups(chat_id)?;

    let result = format_simple_list(&groups);

    bot.send_message(msg.chat.id, result)
        .await
        .map_err(|e| TelegramError::new("cannot send group list", e))?;

    Ok(())
}

async fn handle_list_group_members<D: Database>(
    bot: &Bot,
    msg: &Message,
    database: &Arc<Mutex<D>>,
    group_name: &str,
) -> HandlerResult {
    let chat_id = msg.chat.id.0;
    validate_group_name(group_name)?;
    debug!("Listing all members of group: {group_name}");

    validate_group_exists(group_name, chat_id, database).await?;

    let members = database
        .lock()
        .await
        .get_group_members(chat_id, group_name)?;

    let result = format_simple_list(&members);
    bot.send_message(msg.chat.id, result)
        .await
        .map_err(|e| TelegramError::new("cannot send group member list", e))?;

    Ok(())
}

async fn handle_toggle_auto_register<D: Database>(
    bot: &Bot,
    msg: &Message,
    database: &Arc<Mutex<D>>,
) -> HandlerResult {
    let chat_id = msg.chat.id.0;
    debug!("Toggling auto register");

    let auto_register = database.lock().await.toggle_auto_register(chat_id)?;

    if auto_register {
        let message = "The 'auto register' mode is ENABLED: participants used in expenses will \
                       be automatically registered as participants if they are not already.";
        bot.send_message(msg.chat.id, message)
            .await
            .map_err(|e| TelegramError::new("cannot send toggle auto register message", e))?;
    } else {
        let message = "The 'auto register' mode is DISABLED: using unregistered participants in \
                       expenses will result in an error.";
        bot.send_message(msg.chat.id, message)
            .await
            .map_err(|e| TelegramError::new("cannot send toggle auto register message", e))?;
    }

    Ok(())
}

async fn handle_is_auto_register<D: Database>(
    bot: &Bot,
    msg: &Message,
    database: &Arc<Mutex<D>>,
) -> HandlerResult {
    let chat_id = msg.chat.id.0;
    debug!("Checking auto register mode");

    let auto_register = database.lock().await.is_auto_register_active(chat_id)?;

    if auto_register {
        let message = "The 'auto register' mode is ENABLED.";
        bot.send_message(msg.chat.id, message)
            .await
            .map_err(|e| TelegramError::new("cannot send is auto register message", e))?;
    } else {
        let message = "The 'auto register' mode is DISABLED.";
        bot.send_message(msg.chat.id, message)
            .await
            .map_err(|e| TelegramError::new("cannot send is auto register message", e))?;
    }

    Ok(())
}
