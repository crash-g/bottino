use chrono::{DateTime, Utc};

use crate::types::{Expense, ExpenseWithId};

pub mod sqlite;

pub trait Memory {
    fn save_expense_with_message(
        &mut self,
        chat_id: i64,
        expense: Expense,
        message_ts: DateTime<Utc>,
    ) -> anyhow::Result<()>;
    fn get_active_expenses(&self, chat_id: i64) -> anyhow::Result<Vec<ExpenseWithId>>;
    fn get_active_expenses_with_limit(
        &self,
        chat_id: i64,
        limit: usize,
    ) -> anyhow::Result<Vec<ExpenseWithId>>;
    fn mark_all_as_settled(&self, chat_id: i64) -> anyhow::Result<()>;
    fn delete_expense(&self, chat_id: i64, expense_id: i64) -> anyhow::Result<()>;
}
