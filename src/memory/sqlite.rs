use anyhow::Context;
use chrono::{DateTime, Utc};
use log::{debug, info};
use rusqlite::{params, Connection, Params, Statement};
use std::collections::HashMap;
use tokio::task::block_in_place;

use super::Memory;
use crate::types::{Expense, ExpenseWithId, Participant};

pub struct SqlLiteMemory {
    connection: Connection,
}

impl SqlLiteMemory {
    pub fn new() -> anyhow::Result<SqlLiteMemory> {
        block_in_place(|| {
            let connection = Connection::open("treasurer.db")?;
            connection.execute(
                "CREATE TABLE IF NOT EXISTS expense (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 chat_id INTEGER NOT NULL,
                 amount INTEGER NOT NULL,
                 message TEXT,
                 message_ts DATETIME NOT NULL,
                 created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
                 settled_at DATETIME,
                 deleted_at DATETIME
               )",
                (),
            )?;
            connection.execute(
                "CREATE TABLE IF NOT EXISTS participant (
                 name TEXT NOT NULL,
                 is_creditor BOOL NOT NULL,
                 expense_id INTEGER NOT NULL,
                 amount INTEGER
             )",
                (),
            )?;
            Ok(SqlLiteMemory { connection })
        })
    }

    pub fn connection() -> anyhow::Result<SqlLiteMemory> {
        block_in_place(|| {
            Ok(SqlLiteMemory {
                connection: Connection::open("treasurer.db")?,
            })
        })
    }
}

impl Memory for SqlLiteMemory {
    fn save_expense_with_message(
        &mut self,
        chat_id: i64,
        expense: Expense,
        message_ts: DateTime<Utc>,
    ) -> anyhow::Result<()> {
        block_in_place(|| {
            let tx = self.connection.transaction()?;

            let expense_id: i64 = tx.query_row(
                "INSERT INTO expense (chat_id, amount, message, message_ts) values (?1, ?2, ?3, ?4) RETURNING id",
                params![&chat_id, &expense.amount, &expense.message, &message_ts],
                |row| row.get(0),
            )?;

            debug!("expense_id is {expense_id}");

            for participant in expense.participants {
                tx.execute(
                    "INSERT INTO participant (name, is_creditor, expense_id, amount) values (?1, ?2, ?3, ?4)",
                    params![&participant.name, &participant.is_creditor(), &expense_id, &participant.amount],
                )?;
            }

            tx.commit()?;

            Ok(())
        })
    }

    fn get_active_expenses(&self, chat_id: i64) -> anyhow::Result<Vec<ExpenseWithId>> {
        block_in_place(|| {
            let stmt = self
                .connection
                .prepare(
                    "SELECT e.id, e.amount, e.message, p.name, p.is_creditor, p.amount FROM expense e
                 INNER JOIN participant p ON e.id = p.expense_id
                 WHERE e.chat_id = :chat_id AND e.settled_at IS NULL AND e.deleted_at IS NULL",
                )
                .with_context(|| "Could not prepare statement")?;

            query_active_expenses(stmt, &[(":chat_id", &chat_id)])
        })
    }

    fn get_active_expenses_with_limit(
        &self,
        chat_id: i64,
        limit: usize,
    ) -> anyhow::Result<Vec<ExpenseWithId>> {
        block_in_place(|| {
            let stmt = self
                .connection
                .prepare(
                    "SELECT e.id, e.amount, e.message, p.name, p.is_creditor, p.amount FROM expense e
                 INNER JOIN participant p ON e.id = p.expense_id
                 WHERE e.chat_id = :chat_id AND e.settled_at IS NULL AND e.deleted_at IS NULL
                 ORDER BY created_at DESC
                 LIMIT :limit",
                )
                .with_context(|| "Could not prepare statement")?;

            let limit = limit as i64;
            query_active_expenses(stmt, &[(":chat_id", &chat_id), (":limit", &limit)])
        })
    }

    fn mark_all_as_settled(&self, chat_id: i64) -> anyhow::Result<()> {
        debug!("Marking all as settled using current timestamp. Chat ID: {chat_id}");
        self.connection
            .execute(
                "UPDATE expense SET settled_at = CURRENT_TIMESTAMP
             WHERE chat_id = ?1 AND settled_at IS NULL AND deleted_at IS NULL",
                params![&chat_id],
            )
            .with_context(|| "Query to set all settled failed")?;

        Ok(())
    }

    fn delete_expense(&self, chat_id: i64, expense_id: i64) -> anyhow::Result<()> {
        info!("Deleting expense. Chat ID: {chat_id}. Expense ID: {expense_id}");
        self.connection
            .execute(
                "UPDATE expense SET deleted_at = CURRENT_TIMESTAMP
                 WHERE chat_id = ?1 AND id = ?2 AND settled_at IS NULL AND deleted_at IS NULL",
                params![&chat_id, &expense_id],
            )
            .with_context(|| "Query to delete expense failed")?;

        Ok(())
    }
}

fn query_active_expenses(
    mut statement: Statement,
    params: impl Params,
) -> anyhow::Result<Vec<ExpenseWithId>> {
    let expense_iter = statement
        .query_map(params, |row| {
            Ok(ActiveExpenseQuery {
                id: row.get(0)?,
                e_amount: row.get(1)?,
                e_message: row.get(2)?,
                p_name: row.get(3)?,
                p_is_creditor: row.get(4)?,
                p_amount: row.get(5)?,
            })
        })
        .with_context(|| "Query to get active connections has failed")?;

    let expenses: Result<Vec<_>, _> = expense_iter.collect();
    Ok(parse_active_expenses_query(expenses?))
}

fn parse_active_expenses_query(expenses: Vec<ActiveExpenseQuery>) -> Vec<ExpenseWithId> {
    let mut result = HashMap::new();
    for active_expense in expenses {
        let entry = result.entry(active_expense.id).or_insert_with(|| {
            ExpenseWithId::new(
                active_expense.id,
                vec![],
                active_expense.e_amount,
                active_expense.e_message,
            )
        });

        let name = &active_expense.p_name;
        let amount = active_expense.p_amount;
        let participant = if active_expense.p_is_creditor {
            Participant::new_creditor(name, amount)
        } else {
            Participant::new_debtor(name, amount)
        };
        entry.participants.push(participant);
    }

    result.into_iter().map(|(_, e)| e).collect()
}

struct ActiveExpenseQuery {
    id: i64,
    e_amount: i64,
    e_message: Option<String>,
    p_name: String,
    p_is_creditor: bool,
    p_amount: Option<i64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_conversion() {
        let expenses = vec![
            ActiveExpenseQuery {
                id: 1,
                e_amount: 300,
                e_message: None,
                p_name: "name1".to_string(),
                p_is_creditor: true,
                p_amount: None,
            },
            ActiveExpenseQuery {
                id: 1,
                e_amount: 300,
                e_message: None,
                p_name: "name2".to_string(),
                p_is_creditor: false,
                p_amount: None,
            },
            ActiveExpenseQuery {
                id: 1,
                e_amount: 300,
                e_message: None,
                p_name: "name3".to_string(),
                p_is_creditor: false,
                p_amount: Some(100),
            },
            ActiveExpenseQuery {
                id: 2,
                e_amount: 5400,
                e_message: None,
                p_name: "name1".to_string(),
                p_is_creditor: true,
                p_amount: None,
            },
            ActiveExpenseQuery {
                id: 2,
                e_amount: 5400,
                e_message: None,
                p_name: "name2".to_string(),
                p_is_creditor: false,
                p_amount: None,
            },
        ];

        let result = parse_active_expenses_query(expenses);
        dbg!("{}", result);
    }
}
