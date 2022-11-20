//! Internal representation of data.

use chrono::{DateTime, Utc};

use crate::types::{ParsedExpense, SavedExpense};

pub mod sqlite;

/// This trait abstracts over the type of memory.

/// The implementation could save the data in memory or, more likely,
/// in a database.
pub trait Memory {
    /// Save an expense inside the memory.
    fn save_expense_with_message(
        &mut self,
        chat_id: i64,
        expense: ParsedExpense,
        message_ts: DateTime<Utc>,
    ) -> anyhow::Result<()>;

    /// Get the list of all active expenses.
    ///
    /// An expense is active if it is neither settled nor deleted. Each returned expense
    /// must have a unique ID, that can be used to delete it.
    fn get_active_expenses(&self, chat_id: i64) -> anyhow::Result<Vec<SavedExpense>>;

    /// Get the latest active expenses, restricting the list by the given *limit*.
    ///
    /// An expense is active if it is neither settled nor deleted. Each returned expense
    /// must have a unique ID, that can be used to delete it.
    fn get_active_expenses_with_limit(
        &self,
        chat_id: i64,
        limit: usize,
    ) -> anyhow::Result<Vec<SavedExpense>>;

    /// Mark all active expenses as settled.
    ///
    /// An expense is active if it is neither settled nor deleted. The actual implementation
    /// could actually delete the expenses, since there is no requirement to be able to
    /// retrieve them later.
    fn mark_all_as_settled(&self, chat_id: i64) -> anyhow::Result<()>;

    /// Delete the expense with the given *expense_id*.
    ///
    /// The actual implementation could delete the expense or just mark it as deleted. The
    /// only requirement is that it does not show as active later on.
    fn delete_expense(&self, chat_id: i64, expense_id: i64) -> anyhow::Result<()>;

    /// Create a group with the given *group_name*.
    ///
    /// If the group already exists, it is a no-op.
    fn create_group_if_not_exists(&mut self, chat_id: i64, group_name: &str) -> anyhow::Result<()>;

    /// Delete a group with the given *group_name*.
    ///
    /// If the group does not exist, it is a no-op.
    fn delete_group_if_exists(&mut self, chat_id: i64, group_name: &str) -> anyhow::Result<()>;

    /// Add the given members to a group.
    ///
    /// If some of the members are already present, they are ignored. If the group does not exist,
    /// an error is returned.
    fn add_group_members_if_not_exist<T: AsRef<str>>(
        &mut self,
        chat_id: i64,
        group_name: &str,
        members: &[T],
    ) -> anyhow::Result<()>;

    /// Remove the given members from a group.
    ///
    /// If some of the members are not present, they are ignored. If the group does not exist,
    /// an error is returned.
    fn remove_group_members_if_exist<T: AsRef<str>>(
        &mut self,
        chat_id: i64,
        group_name: &str,
        members: &[T],
    ) -> anyhow::Result<()>;

    /// Get the list of all groups.
    fn get_groups(&self, chat_id: i64) -> anyhow::Result<Vec<String>>;

    /// Check if a group with the given *group_name* exists.
    fn group_exists(&self, chat_id: i64, group_name: &str) -> anyhow::Result<bool>;

    /// Get the list of members of a group.
    ///
    /// If the group does not exist, an error is returned.
    fn get_group_members(&self, chat_id: i64, group_name: &str) -> anyhow::Result<Vec<String>>;
}
