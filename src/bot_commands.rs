//! Definition of Telegram bot commands and handlers.

use std::sync::Arc;

use anyhow::bail;
use log::{debug, error};
use teloxide::{
    dispatching::{
        dialogue::{self, GetChatId, InMemStorage},
        UpdateHandler,
    },
    prelude::*,
    types::{InlineKeyboardButton, InlineKeyboardMarkup, MessageId, ParseMode},
    utils::command::BotCommands,
};
use tokio::sync::Mutex;

use crate::{
    database::{sqlite::SqliteDatabase, Database},
    endpoints,
    error::{DatabaseError, InputError, TelegramError},
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
    #[command(description = "shortcut for the /balance command.")]
    B,
    #[command(description = "marks all expenses as settled.")]
    Reset,
    #[command(description = "/list shows all the expenses added since the latest call to reset.")]
    List,
    #[command(description = "shortcut for the /list command.")]
    L,
    #[command(
        description = "/listall shows all the expenses; the ones that were added \
                             before the latest call to reset have a red icon."
    )]
    ListAll,
    #[command(description = "shortcut for the /listall command.")]
    La,
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
        description = "/addparticipantaliases participant alias1 alias2 adds two aliases for a participant \
                       if not already present."
    )]
    AddParticipantAliases(String),
    #[command(description = "shortcut for the /addparticipantaliases command")]
    Apa(String),
    #[command(
        description = "/removeparticipantaliases participant alias1 alias2 removes two aliases for a participant \
                       if present."
    )]
    RemoveParticipantAliases(String),
    #[command(description = "shortcut for the /removeparticipantaliases command")]
    Rpa(String),
    #[command(description = "returns the list of all aliases of the given participant.")]
    ListParticipantAliases(String),
    #[command(description = "shortcut for the /listparticipantaliases command")]
    Lpa(String),
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
        description = "toggle the auto register mode: when active the participants in an expense are \
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

const DEFAULT_LIMIT: usize = 15;
const LIST_CALLBACK_PREFIX: &str = "list";
const LIST_ALL_CALLBACK_PREFIX: &str = "list-all";

pub fn dialogue_handler() -> UpdateHandler<Box<dyn std::error::Error + Send + Sync + 'static>> {
    use dptree::case;

    let command_handler =
        teloxide::filter_command::<Command, _>().branch(case![State::Normal].endpoint(
            |msg: Message, bot: Bot, cmd: Command, database: DatabaseInUse| async move {
                use Command::*;
                let result = match cmd {
                    Help => handle_help(&bot, &msg).await,
                    Expense(e) | E(e) => handle_expense(&msg, &database, &e).await,
                    Balance | B => handle_balance(&bot, &msg, &database).await,
                    Reset => handle_reset(&msg, &database).await,
                    List | L => handle_list(&bot, &msg, &database, true).await,
                    ListAll | La => handle_list(&bot, &msg, &database, false).await,
                    Delete(id) => handle_delete(&msg, &database, &id).await,
                    AddParticipants(s) | Ap(s) => {
                        handle_add_participants(&msg, &database, &s).await
                    }
                    RemoveParticipants(s) | Rp(s) => {
                        handle_remove_participants(&msg, &database, &s).await
                    }
                    ListParticipants | Lp => handle_list_participants(&bot, &msg, &database).await,
                    AddParticipantAliases(s) | Apa(s) => {
                        handle_add_participant_aliases(&msg, &database, &s).await
                    }
                    RemoveParticipantAliases(s) | Rpa(s) => {
                        handle_remove_participant_aliases(&msg, &database, &s).await
                    }
                    ListParticipantAliases(participant) | Lpa(participant) => {
                        handle_list_participant_aliases(&bot, &msg, &database, &participant).await
                    }
                    AddGroup(group_name) | Ag(group_name) => {
                        handle_add_group(&msg, &database, &group_name).await
                    }
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

    let callback_query_handler =
        Update::filter_callback_query().branch(case![State::Normal].endpoint(
            |q: CallbackQuery, bot: Bot, database: DatabaseInUse| async move {
                let chat_id = q.chat_id();
                let message = q.message;
                let callback_data = q.data;

                match (chat_id, message, callback_data) {
                    (Some(chat_id), Some(message), Some(callback_data)) => {
                        let message_id = message.id;
                        let result =
                            dispatch_callback(chat_id, message_id, &bot, &database, callback_data)
                                .await;
                        if result.is_err() {
                            debug!("Cannot dispatch callback: {:#?}", result);
                        }
                    }
                    _ => {
                        debug!("Missing chat id, message or callback data");
                    }
                }
                Ok(())
            },
        ));

    dialogue::enter::<Update, InMemStorage<State>, State, _>()
        .branch(message_handler)
        .branch(callback_query_handler)
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
    endpoints::handle_expense(chat_id, message, database, message_ts).await?;
    Ok(())
}

async fn handle_balance<D: Database>(
    bot: &Bot,
    msg: &Message,
    database: &Arc<Mutex<D>>,
) -> HandlerResult {
    let chat_id = msg.chat.id.0;
    let formatted_balance = endpoints::handle_balance(chat_id, database).await?;
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
    only_active: bool,
) -> HandlerResult {
    let chat_id = msg.chat.id.0;
    let (result, there_are_more) =
        endpoints::handle_list(chat_id, database, 0, DEFAULT_LIMIT, only_active).await?;

    let buttons = if there_are_more {
        vec![InlineKeyboardButton::callback(
            "Next",
            make_list_callback_data(DEFAULT_LIMIT, only_active),
        )]
    } else {
        vec![]
    };

    bot.send_message(msg.chat.id, result)
        .parse_mode(ParseMode::MarkdownV2)
        .reply_markup(InlineKeyboardMarkup::new([buttons]))
        .await
        .map_err(|e| TelegramError::new("cannot send expense list", e))?;

    Ok(())
}

async fn dispatch_callback<D: Database>(
    chat_id: ChatId,
    message_id: MessageId,
    bot: &Bot,
    database: &Arc<Mutex<D>>,
    callback_data: String,
) -> HandlerResult {
    let parsed_callback_data = callback_data.split_once(" ");
    match parsed_callback_data {
        Some((LIST_CALLBACK_PREFIX, start)) => {
            handle_list_callback(chat_id, message_id, bot, database, start, true).await
        }
        Some((LIST_ALL_CALLBACK_PREFIX, start)) => {
            handle_list_callback(chat_id, message_id, bot, database, start, false).await
        }
        Some((prefix, _)) => bail!("Unknown callback data prefix: {}", prefix),
        None => bail!("Invalid callback data: {}", callback_data),
    }
}

async fn handle_list_callback<D: Database>(
    chat_id: ChatId,
    message_id: MessageId,
    bot: &Bot,
    database: &Arc<Mutex<D>>,
    start: &str,
    only_active: bool,
) -> HandlerResult {
    let start = start.trim();
    let start = start.parse()?;

    let (result, there_are_more) =
        endpoints::handle_list(chat_id.0, database, start, DEFAULT_LIMIT, only_active).await?;

    let mut buttons = vec![];
    if start > 0 {
        if start <= DEFAULT_LIMIT {
            let button =
                InlineKeyboardButton::callback("Previous", make_list_callback_data(0, only_active));
            buttons.push(button);
        } else {
            let button = InlineKeyboardButton::callback(
                "Previous",
                make_list_callback_data(start - DEFAULT_LIMIT, only_active),
            );
            buttons.push(button);
        }
    }
    if there_are_more {
        let button = InlineKeyboardButton::callback(
            "Next",
            make_list_callback_data(start + DEFAULT_LIMIT, only_active),
        );
        buttons.push(button);
    }

    bot.edit_message_text(chat_id, message_id, result)
        .parse_mode(ParseMode::MarkdownV2)
        .reply_markup(InlineKeyboardMarkup::new([buttons]))
        .await?;
    Ok(())
}

fn make_list_callback_data(start: usize, only_active: bool) -> String {
    if only_active {
        format!("{} {}", LIST_CALLBACK_PREFIX, start)
    } else {
        format!("{} {}", LIST_ALL_CALLBACK_PREFIX, start)
    }
}

async fn handle_delete<D: Database>(
    msg: &Message,
    database: &Arc<Mutex<D>>,
    expense_id: &str,
) -> HandlerResult {
    let chat_id = msg.chat.id.0;
    endpoints::handle_delete(chat_id, database, expense_id).await?;
    Ok(())
}

async fn handle_add_participants<D: Database>(
    msg: &Message,
    database: &Arc<Mutex<D>>,
    payload: &str,
) -> HandlerResult {
    let chat_id = msg.chat.id.0;
    endpoints::handle_add_participants(chat_id, database, payload).await?;
    Ok(())
}

async fn handle_remove_participants<D: Database>(
    msg: &Message,
    database: &Arc<Mutex<D>>,
    payload: &str,
) -> HandlerResult {
    let chat_id = msg.chat.id.0;
    endpoints::handle_remove_participants(chat_id, database, payload).await?;
    Ok(())
}

async fn handle_list_participants<D: Database>(
    bot: &Bot,
    msg: &Message,
    database: &Arc<Mutex<D>>,
) -> HandlerResult {
    let chat_id = msg.chat.id.0;
    let result = endpoints::handle_list_participants(chat_id, database).await?;
    bot.send_message(msg.chat.id, result)
        .await
        .map_err(|e| TelegramError::new("cannot send participant list", e))?;
    Ok(())
}

async fn handle_add_participant_aliases<D: Database>(
    msg: &Message,
    database: &Arc<Mutex<D>>,
    payload: &str,
) -> HandlerResult {
    let chat_id = msg.chat.id.0;
    endpoints::handle_add_participant_aliases(chat_id, database, payload).await?;
    Ok(())
}

async fn handle_remove_participant_aliases<D: Database>(
    msg: &Message,
    database: &Arc<Mutex<D>>,
    payload: &str,
) -> HandlerResult {
    let chat_id = msg.chat.id.0;
    endpoints::handle_remove_participant_aliases(chat_id, database, payload).await?;
    Ok(())
}

async fn handle_list_participant_aliases<D: Database>(
    bot: &Bot,
    msg: &Message,
    database: &Arc<Mutex<D>>,
    participant: &str,
) -> HandlerResult {
    let chat_id = msg.chat.id.0;
    let result = endpoints::handle_list_participant_aliases(chat_id, database, participant).await?;
    bot.send_message(msg.chat.id, result)
        .await
        .map_err(|e| TelegramError::new("cannot send participant alias list", e))?;

    Ok(())
}

async fn handle_add_group<D: Database>(
    msg: &Message,
    database: &Arc<Mutex<D>>,
    group_name: &str,
) -> HandlerResult {
    let chat_id = msg.chat.id.0;
    endpoints::handle_add_group(chat_id, database, group_name).await?;
    Ok(())
}

async fn handle_remove_group<D: Database>(
    msg: &Message,
    database: &Arc<Mutex<D>>,
    group_name: &str,
) -> HandlerResult {
    let chat_id = msg.chat.id.0;
    endpoints::handle_remove_group(chat_id, database, group_name).await?;
    Ok(())
}

async fn handle_add_group_members<D: Database>(
    msg: &Message,
    database: &Arc<Mutex<D>>,
    payload: &str,
) -> HandlerResult {
    let chat_id = msg.chat.id.0;
    endpoints::handle_add_group_members(chat_id, database, payload).await?;
    Ok(())
}

async fn handle_remove_group_members<D: Database>(
    msg: &Message,
    database: &Arc<Mutex<D>>,
    payload: &str,
) -> HandlerResult {
    let chat_id = msg.chat.id.0;
    endpoints::handle_remove_group_members(chat_id, database, payload).await?;
    Ok(())
}

async fn handle_list_groups<D: Database>(
    bot: &Bot,
    msg: &Message,
    database: &Arc<Mutex<D>>,
) -> HandlerResult {
    let chat_id = msg.chat.id.0;
    let result = endpoints::handle_list_groups(chat_id, database).await?;
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
    let result = endpoints::handle_list_group_members(chat_id, database, group_name).await?;
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
