//! Core implementation of bot handlers.
//!
//! This is split from `bot_commands` because that module was becoming very large
//! and also because these methods are the largest subset of logic that can be tested
//! without mocking Telegram APIs.

use chrono::{DateTime, Utc};
use log::debug;
use std::{
    cmp::Ordering,
    collections::{hash_map::Entry, HashMap},
    sync::Arc,
};
use tokio::sync::Mutex;

use crate::{
    bot_logic::compute_exchanges,
    database::Database,
    error::{DatabaseError, InputError},
    formatter::{format_balance, format_list_expenses, format_simple_list},
    parser::{
        parse_expense, parse_group_and_members, parse_participant_and_aliases, parse_participants,
    },
    types::{ParsedExpense, ParsedParticipant},
    validator::{
        validate_alias_names, validate_aliases_do_not_exist, validate_aliases_exist,
        validate_expense, validate_group_exists, validate_group_name, validate_groups,
        validate_participant_exists, validate_participant_name, validate_participant_names,
        validate_participants_exist,
    },
};

pub async fn handle_expense<D: Database>(
    chat_id: i64,
    message: &str,
    database: &Arc<Mutex<D>>,
    message_ts: DateTime<Utc>,
) -> anyhow::Result<()> {
    let expense = parse_expense(message).map_err(InputError::invalid_expense_syntax)?;
    let expense = expense.1;
    validate_groups(&expense, chat_id, database).await?;
    let expense = resolve_groups(expense, chat_id, database).await?;
    let expense = resolve_aliases(expense, chat_id, database).await?;

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

/// Replace groups with their participants.
async fn resolve_groups<D: Database>(
    mut expense: ParsedExpense,
    chat_id: i64,
    database: &Arc<Mutex<D>>,
) -> Result<ParsedExpense, DatabaseError> {
    let mut participants = Vec::with_capacity(expense.participants.len());

    for participant in expense.participants {
        if participant.is_group() {
            let members = database
                .lock()
                .await
                .get_group_members(chat_id, &participant.name)?;

            for member in members {
                let p = if participant.is_creditor() {
                    ParsedParticipant::new_creditor(&member, None)
                } else {
                    ParsedParticipant::new_debtor(&member, None)
                };
                participants.push(p);
            }
        } else {
            participants.push(participant);
        }
    }

    expense.participants = participants;
    Ok(expense)
}

/// Replace aliases with the corresponding participant.
async fn resolve_aliases<D: Database>(
    mut expense: ParsedExpense,
    chat_id: i64,
    database: &Arc<Mutex<D>>,
) -> Result<ParsedExpense, DatabaseError> {
    let aliases = database.lock().await.get_aliases(chat_id)?;

    for participant in &mut expense.participants {
        if !participant.is_group() && aliases.contains_key(&participant.name) {
            participant.name = aliases
                .get(&participant.name)
                .expect("Just checked that the key is present!")
                .clone();
        }
    }

    Ok(expense)
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

pub async fn handle_balance<D: Database>(
    chat_id: i64,
    database: &Arc<Mutex<D>>,
) -> anyhow::Result<String> {
    let active_expenses = database.lock().await.get_active_expenses(chat_id)?;
    let mut exchanges = compute_exchanges(active_expenses);
    exchanges.sort_by(|e1, e2| match e1.debtor.cmp(&e2.debtor) {
        Ordering::Equal => e1.creditor.cmp(&e2.creditor),
        o => o,
    });
    let formatted_balance = format_balance(&exchanges);
    Ok(formatted_balance)
}

/// This method returns the formatted string and a boolean: if the
/// boolean is true then there are more results available.
pub async fn handle_list<D: Database>(
    chat_id: i64,
    database: &Arc<Mutex<D>>,
    start: usize,
    limit: usize,
) -> anyhow::Result<(String, bool)> {
    debug!(
        "Producing the list of expenses from {} with limit {}",
        start, limit
    );

    let active_expenses =
        database
            .lock()
            .await
            .get_active_expenses_with_limit(chat_id, start, limit + 1)?;

    if active_expenses.len() <= limit {
        let result = format_list_expenses(&active_expenses);
        Ok((result, false))
    } else {
        let result = format_list_expenses(&active_expenses[0..limit]);
        Ok((result, true))
    }
}

pub async fn handle_delete<D: Database>(
    chat_id: i64,
    database: &Arc<Mutex<D>>,
    expense_id: &str,
) -> anyhow::Result<()> {
    let expense_id = expense_id
        .trim()
        .parse()
        .map_err(|_| InputError::invalid_expense_id(expense_id.to_string()))?;

    database.lock().await.delete_expense(chat_id, expense_id)?;
    Ok(())
}

pub async fn handle_add_participants<D: Database>(
    chat_id: i64,
    database: &Arc<Mutex<D>>,
    payload: &str,
) -> anyhow::Result<()> {
    let participants = parse_participants(payload)?;
    validate_participant_names(&participants)?;
    debug!("Adding participants: {:#?}", participants);
    database
        .lock()
        .await
        .add_participants_if_not_exist(chat_id, &participants)?;
    Ok(())
}

pub async fn handle_remove_participants<D: Database>(
    chat_id: i64,
    database: &Arc<Mutex<D>>,
    payload: &str,
) -> anyhow::Result<()> {
    let participants = parse_participants(payload)?;
    validate_participant_names(&participants)?;
    debug!("Removing participants: {:#?}", participants);

    validate_participants_exist(&participants, chat_id, database).await?;

    database
        .lock()
        .await
        .remove_participants_if_exist(chat_id, &participants)?;
    Ok(())
}

pub async fn handle_list_participants<D: Database>(
    chat_id: i64,
    database: &Arc<Mutex<D>>,
) -> anyhow::Result<String> {
    let mut participants = database.lock().await.get_participants(chat_id)?;
    participants.sort();
    let result = format_simple_list(&participants);
    Ok(result)
}

pub async fn handle_add_participant_aliases<D: Database>(
    chat_id: i64,
    database: &Arc<Mutex<D>>,
    payload: &str,
) -> anyhow::Result<()> {
    let (participant, aliases) = parse_participant_and_aliases(payload)?;
    validate_participant_name(&participant)?;
    validate_alias_names(&aliases)?;
    debug!(
        "Adding aliases to participant named {participant}. Aliases: {:#?}",
        aliases
    );

    validate_participant_exists(&participant, chat_id, database).await?;
    validate_aliases_do_not_exist(&participant, &aliases, chat_id, database).await?;

    database
        .lock()
        .await
        .add_aliases_if_not_exist(chat_id, &participant, &aliases)?;
    Ok(())
}

pub async fn handle_remove_participant_aliases<D: Database>(
    chat_id: i64,
    database: &Arc<Mutex<D>>,
    payload: &str,
) -> anyhow::Result<()> {
    let (participant, aliases) = parse_participant_and_aliases(payload)?;
    validate_participant_name(&participant)?;
    validate_alias_names(&aliases)?;
    debug!(
        "Removing aliases from participant named {participant}. Aliases: {:#?}",
        aliases
    );

    validate_participant_exists(&participant, chat_id, database).await?;
    validate_aliases_exist(&participant, &aliases, chat_id, database).await?;

    database
        .lock()
        .await
        .remove_aliases_if_exist(chat_id, &participant, &aliases)?;
    Ok(())
}

pub async fn handle_list_participant_aliases<D: Database>(
    chat_id: i64,
    database: &Arc<Mutex<D>>,
    participant: &str,
) -> anyhow::Result<String> {
    let participant = participant.trim();
    validate_participant_name(participant)?;
    debug!("Listing all aliases of participant: {participant}");

    validate_participant_exists(participant, chat_id, database).await?;

    let mut aliases = database
        .lock()
        .await
        .get_participant_aliases(chat_id, participant)?;
    aliases.sort();

    let result = format_simple_list(&aliases);
    Ok(result)
}

pub async fn handle_add_group<D: Database>(
    chat_id: i64,
    database: &Arc<Mutex<D>>,
    group_name: &str,
) -> anyhow::Result<()> {
    let group_name = group_name.trim();
    validate_group_name(group_name)?;
    debug!("Creating group named {group_name}");

    database
        .lock()
        .await
        .add_group_if_not_exists(chat_id, group_name)?;
    Ok(())
}

pub async fn handle_remove_group<D: Database>(
    chat_id: i64,
    database: &Arc<Mutex<D>>,
    group_name: &str,
) -> anyhow::Result<()> {
    let group_name = group_name.trim();
    validate_group_name(group_name)?;
    debug!("Removing group named {group_name}");

    validate_group_exists(group_name, chat_id, database).await?;

    database
        .lock()
        .await
        .remove_group_if_exists(chat_id, group_name)?;
    Ok(())
}

pub async fn handle_add_group_members<D: Database>(
    chat_id: i64,
    database: &Arc<Mutex<D>>,
    payload: &str,
) -> anyhow::Result<()> {
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

pub async fn handle_remove_group_members<D: Database>(
    chat_id: i64,
    database: &Arc<Mutex<D>>,
    payload: &str,
) -> anyhow::Result<()> {
    let (group_name, members) = parse_group_and_members(payload)?;
    validate_group_name(&group_name)?;
    validate_participant_names(&members)?;
    debug!(
        "Removing group members from group named {group_name}. Members: {:#?}",
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

pub async fn handle_list_groups<D: Database>(
    chat_id: i64,
    database: &Arc<Mutex<D>>,
) -> anyhow::Result<String> {
    let mut groups = database.lock().await.get_groups(chat_id)?;
    groups.sort();
    let result = format_simple_list(&groups);
    Ok(result)
}

pub async fn handle_list_group_members<D: Database>(
    chat_id: i64,
    database: &Arc<Mutex<D>>,
    group_name: &str,
) -> anyhow::Result<String> {
    let group_name = group_name.trim();
    validate_group_name(group_name)?;
    debug!("Listing all members of group: {group_name}");

    validate_group_exists(group_name, chat_id, database).await?;

    let mut members = database
        .lock()
        .await
        .get_group_members(chat_id, group_name)?;
    members.sort();

    let result = format_simple_list(&members);
    Ok(result)
}
