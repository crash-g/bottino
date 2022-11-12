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

    fn create_group(&mut self, chat_id: i64, group_name: &str) -> anyhow::Result<()>;
    fn remove_group(&mut self, chat_id: i64, group_name: &str) -> anyhow::Result<()>;
    fn add_group_members(
        &mut self,
        chat_id: i64,
        group_name: &str,
        members: &[&str],
    ) -> anyhow::Result<()>;
    fn remove_group_members(
        &mut self,
        chat_id: i64,
        group_name: &str,
        members: &[&str],
    ) -> anyhow::Result<()>;
    fn get_groups(&self, chat_id: i64) -> anyhow::Result<Vec<String>>;
    fn get_group_members(&self, chat_id: i64, group_name: &str) -> anyhow::Result<Vec<String>>;
}
