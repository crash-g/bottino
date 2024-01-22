//! Internal representation of data.

use std::collections::HashMap;

use chrono::{DateTime, Utc};

use crate::{
    error::DatabaseError,
    types::{ParsedExpense, SavedExpense},
};

type DatabaseResult<T> = Result<T, DatabaseError>;

pub mod sqlite;

/// This trait abstracts over the type of database.

/// The implementation could save the data in any suitable database or even in memory.
pub trait Database {
    /// Save an expense inside the database.
    fn save_expense_with_message(
        &mut self,
        chat_id: i64,
        expense: ParsedExpense,
        message_ts: DateTime<Utc>,
    ) -> Result<(), DatabaseError>;

    /// Get the list of all active expenses.
    ///
    /// An expense is active if it is neither settled nor deleted. Each returned expense
    /// must have a unique ID, that can be used to delete it.
    fn get_active_expenses(&self, chat_id: i64) -> Result<Vec<SavedExpense>, DatabaseError>;

    /// Get the list active expenses starting from *start* and restricting the list by the given
    /// *limit*.
    ///
    /// An expense is active if it is neither settled nor deleted. Each returned expense
    /// must have a unique ID, that can be used to delete it.
    fn get_active_expenses_with_limit(
        &self,
        chat_id: i64,
        start: usize,
        limit: usize,
    ) -> Result<Vec<SavedExpense>, DatabaseError>;

    /// Mark all active expenses as settled.
    ///
    /// An expense is active if it is neither settled nor deleted. The actual implementation
    /// could actually delete the expenses, since there is no requirement to be able to
    /// retrieve them later.
    fn mark_all_as_settled(&mut self, chat_id: i64) -> Result<(), DatabaseError>;

    /// Delete the expense with the given *expense_id*.
    ///
    /// The actual implementation could delete the expense or just mark it as deleted. The
    /// only requirement is that it does not show as active later on.
    fn delete_expense(&mut self, chat_id: i64, expense_id: i64) -> Result<(), DatabaseError>;

    /// Add participants to the given chat.
    ///
    /// If some participants already exist, ignore them.
    fn add_participants_if_not_exist<T: AsRef<str>>(
        &mut self,
        chat_id: i64,
        participants: &[T],
    ) -> Result<(), DatabaseError>;

    /// Remove participants from the given chat.
    ///
    /// If some participants do not exist, ignore them. Removed participants are also removed
    /// from all groups they are part of.
    fn remove_participants_if_exist<T: AsRef<str>>(
        &mut self,
        chat_id: i64,
        participants: &[T],
    ) -> Result<(), DatabaseError>;

    /// Get the list of all participants in the given chat.
    fn get_participants(&self, chat_id: i64) -> Result<Vec<String>, DatabaseError>;

    /// Check if a participant with the given *participant_name* exists.
    fn participant_exists(
        &self,
        chat_id: i64,
        participant_name: &str,
    ) -> Result<bool, DatabaseError>;

    /// Add the given aliases for a participant.
    ///
    /// If some aliases are already present, they are ignored. If the participant does not exist,
    /// an error is returned.
    fn add_aliases_if_not_exist<T: AsRef<str>>(
        &mut self,
        chat_id: i64,
        participant: &str,
        aliases: &[T],
    ) -> Result<(), DatabaseError>;

    /// Remove the given participant aliases.
    ///
    /// If some aliases are not present, they are ignored. If the participant does not exist,
    /// an error is returned.
    fn remove_aliases_if_exist<T: AsRef<str>>(
        &mut self,
        chat_id: i64,
        participant: &str,
        aliases: &[T],
    ) -> Result<(), DatabaseError>;

    /// Get the list of all aliases in the given chat.
    ///
    /// The keys are the aliases and the value their corresponding participant name.
    fn get_aliases(&self, chat_id: i64) -> Result<HashMap<String, String>, DatabaseError>;

    /// Get the list of all aliases of the given participant.
    fn get_participant_aliases(
        &self,
        chat_id: i64,
        participant: &str,
    ) -> Result<Vec<String>, DatabaseError>;

    /// Add a group with the given *group_name*.
    ///
    /// If the group already exists, it is a no-op.
    fn add_group_if_not_exists(
        &mut self,
        chat_id: i64,
        group_name: &str,
    ) -> Result<(), DatabaseError>;

    /// Remove a group with the given *group_name*.
    ///
    /// If the group does not exist, it is a no-op.
    fn remove_group_if_exists(
        &mut self,
        chat_id: i64,
        group_name: &str,
    ) -> Result<(), DatabaseError>;

    /// Add the given members to a group.
    ///
    /// If some of the members are already present, they are ignored. If the group does not exist,
    /// an error is returned.
    fn add_group_members_if_not_exist<T: AsRef<str>>(
        &mut self,
        chat_id: i64,
        group_name: &str,
        members: &[T],
    ) -> Result<(), DatabaseError>;

    /// Remove the given members from a group.
    ///
    /// If some of the members are not present, they are ignored. If the group does not exist,
    /// an error is returned.
    fn remove_group_members_if_exist<T: AsRef<str>>(
        &mut self,
        chat_id: i64,
        group_name: &str,
        members: &[T],
    ) -> Result<(), DatabaseError>;

    /// Get the list of all groups.
    fn get_groups(&self, chat_id: i64) -> Result<Vec<String>, DatabaseError>;

    /// Check if a group with the given *group_name* exists.
    fn group_exists(&self, chat_id: i64, group_name: &str) -> Result<bool, DatabaseError>;

    /// Get the list of members of a group.
    ///
    /// If the group does not exist, an error is returned.
    fn get_group_members(
        &self,
        chat_id: i64,
        group_name: &str,
    ) -> Result<Vec<String>, DatabaseError>;

    /// Check if the auto_register flag is active.
    fn is_auto_register_active(&self, chat_id: i64) -> Result<bool, DatabaseError>;

    /// Toggle the auto_register flag.
    fn toggle_auto_register(&mut self, chat_id: i64) -> Result<bool, DatabaseError>;
}
