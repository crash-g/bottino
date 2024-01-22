//! The implementation of a data storage using Sqlite.

use anyhow::{anyhow, Context};
use chrono::{DateTime, Utc};
use log::{debug, info};
use rusqlite::{params, CachedStatement, Connection, OptionalExtension, Params};
use std::collections::HashMap;
use tokio::task::block_in_place;

use crate::types::{ParsedExpense, SavedExpense, SavedParticipant};

use super::Memory;

mod schema;

pub struct SqliteMemory {
    connection: Connection,
}

impl SqliteMemory {
    pub fn new() -> anyhow::Result<SqliteMemory> {
        block_in_place(|| {
            let connection = Connection::open("treasurer.db")?;
            schema::create_all_tables(&connection)?;
            Ok(SqliteMemory { connection })
        })
    }
}

impl Memory for SqliteMemory {
    fn save_expense_with_message(
        &mut self,
        chat_id: i64,
        expense: ParsedExpense,
        message_ts: DateTime<Utc>,
    ) -> anyhow::Result<()> {
        block_in_place(|| {
            let tx = self.connection.transaction()?;

            let expense_id: i64 = {
                let mut insert_expense_stmt = tx.prepare_cached(
                    "INSERT INTO expense (chat_id, amount, message, message_ts) VALUES (?1, ?2, ?3, ?4) RETURNING id"
                )?;

                insert_expense_stmt.query_row(
                    params![&chat_id, &expense.amount, &expense.message, &message_ts],
                    |row| row.get(0),
                )?
            };

            debug!("expense_id is {expense_id}");

            {
                let mut insert_participant_stmt = tx.prepare_cached(
                    "INSERT INTO expense_participant (expense_id, participant_id, is_creditor, amount)
                SELECT ?1, id, ?2, ?3 FROM participant
                WHERE chat_id = ?4 AND name = ?5"
                )?;

                for participant in expense.participants {
                    let num_inserted_rows = insert_participant_stmt.execute(params![
                        &expense_id,
                        &participant.is_creditor(),
                        &participant.amount,
                        &chat_id,
                        &participant.name,
                    ])?;
                    if num_inserted_rows == 0 {
                        return Err(
                            // This is a concurrency error: fail and let the user try again.
                            anyhow!("the participant was not found"),
                        );
                    }
                }
            }

            tx.commit()?;

            Ok(())
        })
    }

    fn get_active_expenses(&self, chat_id: i64) -> anyhow::Result<Vec<SavedExpense>> {
        block_in_place(|| {
            let stmt = self
                .connection
                .prepare_cached(
                    "SELECT e.id, e.amount, e.message, p.name, ep.is_creditor, ep.amount FROM expense e
                 INNER JOIN expense_participant ep ON e.id = ep.expense_id
                 INNER JOIN participant p ON ep.participant_id = p.id
                 WHERE e.chat_id = :chat_id AND e.settled_at IS NULL AND e.deleted_at IS NULL",
                )
                .with_context(|| "Could not prepare get active expense statement")?;

            query_active_expenses(stmt, &[(":chat_id", &chat_id)])
        })
    }

    /// The current implementation of this function applies the limit to the number of participants,
    /// while it should apply it to the number of expenses. It is still left here because we could
    /// improve it in the future, but better not to use it for now.
    fn get_active_expenses_with_limit(
        &self,
        chat_id: i64,
        limit: usize,
    ) -> anyhow::Result<Vec<SavedExpense>> {
        block_in_place(|| {
            let stmt = self
                .connection
                .prepare_cached(
                    "SELECT e.id, e.amount, e.message, p.name, ep.is_creditor, ep.amount FROM expense e
                 INNER JOIN expense_participant ep ON e.id = p.expense_id
                 INNER JOIN participant p ON ep.participant_id = p.id
                 WHERE e.chat_id = :chat_id AND e.settled_at IS NULL AND e.deleted_at IS NULL
                 ORDER BY created_at DESC
                 LIMIT :limit",
                )
                .with_context(|| "Could not prepare get active expense with limit statement")?;

            let limit = limit as i64;
            query_active_expenses(stmt, &[(":chat_id", &chat_id), (":limit", &limit)])
        })
    }

    fn mark_all_as_settled(&self, chat_id: i64) -> anyhow::Result<()> {
        debug!("Marking all as settled using current timestamp. Chat ID: {chat_id}");
        block_in_place(|| {
            self.connection
                .execute(
                    "UPDATE expense SET settled_at = CURRENT_TIMESTAMP
                 WHERE chat_id = ?1 AND settled_at IS NULL",
                    params![&chat_id],
                )
                .with_context(|| "Query to set all settled failed")?;

            Ok(())
        })
    }

    fn delete_expense(&self, chat_id: i64, expense_id: i64) -> anyhow::Result<()> {
        info!("Deleting expense. Chat ID: {chat_id}. Expense ID: {expense_id}");
        block_in_place(|| {
            self.connection
                .execute(
                    "UPDATE expense SET deleted_at = CURRENT_TIMESTAMP
                 WHERE chat_id = ?1 AND id = ?2 AND settled_at IS NULL AND deleted_at IS NULL",
                    params![&chat_id, &expense_id],
                )
                .with_context(|| "Query to delete expense failed")?;

            Ok(())
        })
    }

    fn add_participants_if_not_exist<T: AsRef<str>>(
        &mut self,
        chat_id: i64,
        participants: &[T],
    ) -> anyhow::Result<()> {
        block_in_place(|| {
            let tx = self.connection.transaction()?;

            {
                let mut insert_participant_stmt = tx
                    .prepare_cached(
                        "INSERT OR IGNORE INTO participant (chat_id, name) VALUES (?1, ?2)",
                    )
                    .with_context(|| "Could not prepare add participants statement")?;
                // It's unclear how to use an IN clause, so we use a loop
                // https://github.com/rusqlite/rusqlite/issues/345
                for participant in participants {
                    insert_participant_stmt.execute(params![&chat_id, &participant.as_ref()])?;
                }
            }

            tx.commit()?;

            Ok(())
        })
    }

    fn remove_participants_if_exist<T: AsRef<str>>(
        &mut self,
        chat_id: i64,
        participants: &[T],
    ) -> anyhow::Result<()> {
        block_in_place(|| {
            let tx = self.connection.transaction()?;

            {
                let mut delete_participant_stmt = tx
                    .prepare_cached(
                        "DELETE FROM participant WHERE chat_id = ?1 AND name = ?2 RETURNING id",
                    )
                    .with_context(|| "Could not prepare remove participants statement")?;

                for participant in participants {
                    let participant_id: Option<i64> = delete_participant_stmt
                        .query_row(params![&chat_id, &participant.as_ref()], |row| row.get(0))
                        .optional()?;

                    if participant_id.is_some() {
                        let mut delete_member_stmt = tx.prepare_cached(
                            "DELETE FROM group_member WHERE participant_id = ?1 AND group_id IN
                         (SELECT id FROM participant_group WHERE chat_id = ?2)",
                        )?;

                        delete_member_stmt.execute(params![
                            &participant_id.expect("Just checked that participant_id is not empty"),
                            &chat_id
                        ])?;
                    }
                }
            }

            tx.commit()?;

            Ok(())
        })
    }

    fn get_participants(&self, chat_id: i64) -> anyhow::Result<Vec<String>> {
        block_in_place(|| {
            let mut stmt = self
                .connection
                .prepare_cached("SELECT name FROM participant WHERE chat_id = :chat_id")
                .with_context(|| "Could not prepare get participants statement")?;

            let participant_iter = stmt
                .query_map(params![&chat_id], |row| row.get(0))
                .with_context(|| "Query to get participants failed")?;

            let participants = participant_iter.collect::<Result<_, _>>()?;
            Ok(participants)
        })
    }

    fn create_group_if_not_exists(&mut self, chat_id: i64, group_name: &str) -> anyhow::Result<()> {
        block_in_place(|| {
            self.connection.execute(
                "INSERT OR IGNORE INTO participant_group (chat_id, name) VALUES (?1, ?2)",
                params![&chat_id, &group_name],
            )?;

            Ok(())
        })
    }

    fn delete_group_if_exists(&mut self, chat_id: i64, group_name: &str) -> anyhow::Result<()> {
        info!("Deleting group. Chat ID: {chat_id}. Group name: {group_name}");
        block_in_place(|| {
            let tx = self.connection.transaction()?;

            let group_id: Option<i64> = tx
                .query_row(
                    "DELETE FROM participant_group WHERE chat_id = ?1 AND name = ?2 RETURNING id",
                    params![&chat_id, &group_name],
                    |row| row.get(0),
                )
                .optional()?;

            if group_id.is_some() {
                tx.execute(
                    "DELETE FROM group_member WHERE group_id = :group_id",
                    params![&group_id.expect("Just checked that group_id is not empty")],
                )?;
            }

            tx.commit()?;

            Ok(())
        })
    }

    fn add_group_members_if_not_exist<T: AsRef<str>>(
        &mut self,
        chat_id: i64,
        group_name: &str,
        members: &[T],
    ) -> anyhow::Result<()> {
        block_in_place(|| {
            let tx = self.connection.transaction()?;

            let group_id: i64 = tx.query_row(
                "SELECT id FROM participant_group WHERE chat_id = :chat_id AND name = :group_name",
                params![&chat_id, &group_name],
                |row| row.get(0),
            )?;

            {
                let mut insert_member_stmt = tx.prepare_cached(
                    "INSERT OR IGNORE INTO group_member (group_id, participant_id)
                     SELECT ?1, p.id FROM participant
                     WHERE chat_id = ?2 AND name = ?3",
                )?;
                // It's unclear how to use an IN clause, so we use a loop
                // https://github.com/rusqlite/rusqlite/issues/345
                for member in members {
                    let num_inserted_rows = insert_member_stmt.execute(params![
                        &group_id,
                        &chat_id,
                        &member.as_ref()
                    ])?;
                    if num_inserted_rows == 0 {
                        return Err(
                            // This is a concurrency error: fail and let the user try again.
                            anyhow!("the participant was not found"),
                        );
                    }
                }
            }

            tx.commit()?;

            Ok(())
        })
    }

    fn remove_group_members_if_exist<T: AsRef<str>>(
        &mut self,
        chat_id: i64,
        group_name: &str,
        members: &[T],
    ) -> anyhow::Result<()> {
        block_in_place(|| {
            let tx = self.connection.transaction()?;

            let group_id: i64 = tx.query_row(
                "SELECT id FROM participant_group WHERE chat_id = :chat_id AND name = :group_name",
                params![&chat_id, &group_name],
                |row| row.get(0),
            )?;

            {
                let mut delete_member_stmt = tx.prepare_cached(
                    "DELETE FROM group_member WHERE group_id = ?1 AND participant_id =
                         (SELECT id FROM participant WHERE chat_id = ?2 AND name = ?3)",
                )?;

                // It's unclear how to use an IN clause, so we use a loop
                // https://github.com/rusqlite/rusqlite/issues/345
                for member in members {
                    delete_member_stmt.execute(params![&group_id, &chat_id, &member.as_ref()])?;
                }
            }

            tx.commit()?;

            Ok(())
        })
    }

    fn get_groups(&self, chat_id: i64) -> anyhow::Result<Vec<String>> {
        block_in_place(|| {
            let mut stmt = self
                .connection
                .prepare_cached("SELECT name FROM participant_group WHERE chat_id = :chat_id")
                .with_context(|| "Could not prepare get groups statement")?;

            let group_iter = stmt
                .query_map(params![&chat_id], |row| row.get(0))
                .with_context(|| "Query to get groups failed")?;

            let groups = group_iter.collect::<Result<_, _>>()?;
            Ok(groups)
        })
    }

    fn group_exists(&self, chat_id: i64, group_name: &str) -> anyhow::Result<bool> {
        block_in_place(|| {
            let group_id: Option<i64> = self
                .connection
                .query_row(
                    "SELECT id FROM participant_group WHERE chat_id = :chat_id AND name = :group_name",
                    params![&chat_id, &group_name],
                    |row| row.get(0),
                ).optional()?;

            if group_id.is_none() {
                Ok(false)
            } else {
                Ok(true)
            }
        })
    }

    fn get_group_members(&self, chat_id: i64, group_name: &str) -> anyhow::Result<Vec<String>> {
        block_in_place(|| {
            let mut stmt = self
                .connection
                .prepare_cached(
                    "SELECT p.name FROM participant_group pg
                         INNER JOIN group_member gm ON pg.id = gm.group_id
                         INNER JOIN participant p ON gm.participant_id = p.id
                         WHERE pg.chat_id = :chat_id AND pg.name = :group_name",
                )
                .with_context(|| "Could not prepare get group members statement")?;

            let group_member_iter = stmt
                .query_map(params![&chat_id, &group_name], |row| row.get(0))
                .with_context(|| "Query to get group members failed")?;

            let group_members = group_member_iter.collect::<Result<_, _>>()?;
            Ok(group_members)
        })
    }
}

fn query_active_expenses(
    mut statement: CachedStatement,
    params: impl Params,
) -> anyhow::Result<Vec<SavedExpense>> {
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
        .with_context(|| "Query to get active expenses failed")?;

    let expenses: Result<Vec<_>, _> = expense_iter.collect();
    Ok(parse_active_expenses_query(expenses?))
}

fn parse_active_expenses_query(expenses: Vec<ActiveExpenseQuery>) -> Vec<SavedExpense> {
    let mut result = HashMap::new();
    for active_expense in expenses {
        let entry = result.entry(active_expense.id).or_insert_with(|| {
            SavedExpense::new(
                active_expense.id,
                vec![],
                active_expense.e_amount,
                active_expense.e_message,
            )
        });

        let name = &active_expense.p_name;
        let amount = active_expense.p_amount;
        let participant = if active_expense.p_is_creditor {
            SavedParticipant::new_creditor(name, amount)
        } else {
            SavedParticipant::new_debtor(name, amount)
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
