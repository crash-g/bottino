//! The implementation of a data storage using Sqlite.

use chrono::{DateTime, Utc};
use log::debug;
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::HashMap;
use tokio::task::block_in_place;

use crate::{
    error::DatabaseError,
    types::{ParsedExpense, SavedExpense, SavedParticipant},
};

use super::{Database, DatabaseResult};

mod schema;

pub struct SqliteDatabase {
    connection: Connection,
}

impl SqliteDatabase {
    pub fn new() -> DatabaseResult<SqliteDatabase> {
        block_in_place(|| {
            let connection = Connection::open("treasurer.db")
                .map_err(|e| DatabaseError::new("cannot open database", e.into()))?;
            schema::create_all_tables(&connection)
                .map_err(|e| DatabaseError::new("cannot create tables", e))?;
            Ok(SqliteDatabase { connection })
        })
    }
}

impl Database for SqliteDatabase {
    fn save_expense_with_message(
        &mut self,
        chat_id: i64,
        expense: ParsedExpense,
        message_ts: DateTime<Utc>,
    ) -> DatabaseResult<()> {
        let fn_impl = || {
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
                WHERE chat_id = ?4 AND name = ?5 AND deleted_at IS NULL"
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
                            DatabaseError::concurrency("the participant was not found").into()
                        );
                    }
                }
            }

            tx.commit()?;

            Ok(())
        };

        block_in_place(|| fn_impl().map_err(|e| map_error("cannot save expense with message", e)))
    }

    fn get_active_expenses(&self, chat_id: i64) -> DatabaseResult<Vec<SavedExpense>> {
        let fn_impl = || {
            let mut stmt = self.connection.prepare_cached(
                "SELECT e.id, e.amount, e.message, p.name, ep.is_creditor, ep.amount FROM expense e
                 INNER JOIN expense_participant ep ON e.id = ep.expense_id
                 INNER JOIN participant p ON ep.participant_id = p.id
                 WHERE e.chat_id = :chat_id AND e.settled_at IS NULL AND e.deleted_at IS NULL",
            )?;

            let expense_iter = stmt.query_map(&[(":chat_id", &chat_id)], |row| {
                Ok(ActiveExpenseQuery {
                    id: row.get(0)?,
                    e_amount: row.get(1)?,
                    e_message: row.get(2)?,
                    p_name: row.get(3)?,
                    p_is_creditor: row.get(4)?,
                    p_amount: row.get(5)?,
                })
            })?;

            let expenses: Result<Vec<_>, _> = expense_iter.collect();
            Ok(parse_active_expenses_query(expenses?))
        };

        block_in_place(|| fn_impl().map_err(|e| map_error("cannot get active expenses", e)))
    }

    fn get_active_expenses_with_limit(
        &self,
        chat_id: i64,
        limit: usize,
    ) -> DatabaseResult<Vec<SavedExpense>> {
        // It's sort of complex to run the query with a limit, so for now
        // we ask for everything and just slice the result.

        let mut all_expenses = self.get_active_expenses(chat_id)?;

        all_expenses.sort_by(|e1, e2| {
            e2.id
                .partial_cmp(&e1.id)
                .expect("cannot sort active expenses")
        });
        let limit = std::cmp::min(limit, all_expenses.len());
        Ok(all_expenses[0..limit].to_vec())
    }

    fn mark_all_as_settled(&self, chat_id: i64) -> DatabaseResult<()> {
        debug!("Marking all as settled using current timestamp. Chat ID: {chat_id}");
        let fn_impl = || {
            self.connection.execute(
                "UPDATE expense SET settled_at = CURRENT_TIMESTAMP
                 WHERE chat_id = ?1 AND settled_at IS NULL",
                params![&chat_id],
            )?;

            Ok(())
        };

        block_in_place(|| fn_impl().map_err(|e| map_error("cannot mark all as settled", e)))
    }

    fn delete_expense(&self, chat_id: i64, expense_id: i64) -> DatabaseResult<()> {
        debug!("Deleting expense. Chat ID: {chat_id}. Expense ID: {expense_id}");
        let fn_impl = || {
            self.connection.execute(
                "UPDATE expense SET deleted_at = CURRENT_TIMESTAMP
                 WHERE chat_id = ?1 AND id = ?2 AND settled_at IS NULL AND deleted_at IS NULL",
                params![&chat_id, &expense_id],
            )?;

            Ok(())
        };

        block_in_place(|| fn_impl().map_err(|e| map_error("cannot delete expense", e)))
    }

    fn add_participants_if_not_exist<T: AsRef<str>>(
        &mut self,
        chat_id: i64,
        participants: &[T],
    ) -> DatabaseResult<()> {
        let mut fn_impl = || {
            let tx = self.connection.transaction()?;

            {
                // We cannot use INSERT OR IGNORE because our UNIQUE constraint includes a nullable column,
                // and NULL values are considered distinct (https://www.sqlite.org/nulls.html).
                let mut insert_participant_stmt = tx.prepare_cached(
                    "INSERT INTO participant (chat_id, name) SELECT ?1, ?2
                     WHERE NOT EXISTS (SELECT 1 FROM participant WHERE chat_id = ?1 AND name = ?2)",
                )?;
                // It's unclear how to use an IN clause, so we use a loop
                // https://github.com/rusqlite/rusqlite/issues/345
                for participant in participants {
                    insert_participant_stmt.execute(params![&chat_id, &participant.as_ref()])?;
                }
            }

            tx.commit()?;

            Ok(())
        };

        block_in_place(|| fn_impl().map_err(|e| map_error("cannot add participants", e)))
    }

    fn remove_participants_if_exist<T: AsRef<str>>(
        &mut self,
        chat_id: i64,
        participants: &[T],
    ) -> DatabaseResult<()> {
        let mut fn_impl = || {
            let tx = self.connection.transaction()?;

            {
                let mut delete_participant_stmt = tx.prepare_cached(
                    "UPDATE participant SET deleted_at = CURRENT_TIMESTAMP
                     WHERE chat_id = ?1 AND name = ?2 AND deleted_at IS NULL",
                )?;

                // It's unclear how to use an IN clause, so we use a loop
                // https://github.com/rusqlite/rusqlite/issues/345
                for participant in participants {
                    delete_participant_stmt.execute(params![&chat_id, &participant.as_ref()])?;
                }
            }

            tx.commit()?;

            Ok(())
        };

        block_in_place(|| fn_impl().map_err(|e| map_error("cannot remove participants", e)))
    }

    fn get_participants(&self, chat_id: i64) -> DatabaseResult<Vec<String>> {
        let fn_impl = || {
            let mut stmt = self.connection.prepare_cached(
                "SELECT name FROM participant
                 WHERE chat_id = :chat_id AND deleted_at IS NULL",
            )?;

            let participant_iter = stmt.query_map(params![&chat_id], |row| row.get(0))?;

            let participants = participant_iter.collect::<Result<_, _>>()?;
            Ok(participants)
        };

        block_in_place(|| fn_impl().map_err(|e| map_error("cannot get participants", e)))
    }

    fn create_group_if_not_exists(&mut self, chat_id: i64, group_name: &str) -> DatabaseResult<()> {
        let fn_impl = || {
            // We cannot use INSERT OR IGNORE because our UNIQUE constraint includes a nullable column,
            // and NULL values are considered distinct (https://www.sqlite.org/nulls.html).
            self.connection.execute(
                "INSERT INTO participant_group (chat_id, name) SELECT ?1, ?2
                 WHERE NOT EXISTS (SELECT 1 FROM participant_group WHERE chat_id = ?1 AND name = ?2)",
                params![&chat_id, &group_name],
            )?;

            Ok(())
        };

        block_in_place(|| fn_impl().map_err(|e| map_error("cannot create group", e)))
    }

    fn delete_group_if_exists(&mut self, chat_id: i64, group_name: &str) -> DatabaseResult<()> {
        debug!("Deleting group. Chat ID: {chat_id}. Group name: {group_name}");
        let fn_impl = || {
            let mut delete_group_stmt = self.connection.prepare_cached(
                "UPDATE participant_group SET deleted_at = CURRENT_TIMESTAMP
                 WHERE chat_id = ?1 AND name = ?2 AND deleted_at IS NULL",
            )?;

            delete_group_stmt.execute(params![&chat_id, group_name])?;

            Ok(())
        };

        block_in_place(|| fn_impl().map_err(|e| map_error("cannot delete group", e)))
    }

    fn add_group_members_if_not_exist<T: AsRef<str>>(
        &mut self,
        chat_id: i64,
        group_name: &str,
        members: &[T],
    ) -> DatabaseResult<()> {
        let mut fn_impl = || {
            let tx = self.connection.transaction()?;

            let group_id: i64 = tx.query_row(
                "SELECT id FROM participant_group
                 WHERE chat_id = :chat_id AND name = :group_name AND deleted_at IS NULL",
                params![&chat_id, &group_name],
                |row| row.get(0),
            )?;

            {
                let mut get_participant_id_stmt = tx.prepare_cached(
                    "SELECT id FROM participant
                     WHERE chat_id = :chat_id AND name = :member AND deleted_at IS NULL"
                )?;

                // We cannot use INSERT OR IGNORE because our UNIQUE constraint includes a nullable column,
                // and NULL values are considered distinct (https://www.sqlite.org/nulls.html).
                let mut insert_member_stmt = tx.prepare_cached(
                    "INSERT INTO group_member (group_id, participant_id) SELECT ?1, ?2
                     WHERE NOT EXISTS (SELECT 1 FROM group_member WHERE group_id = ?1 AND participant_id = ?2)",
                )?;
                // It's unclear how to use an IN clause, so we use a loop
                // https://github.com/rusqlite/rusqlite/issues/345
                for member in members {
                    // TODO: here we run two queries per member, it would be nice to optimize it,
                    // but can we do it considering the restrictions on UPSERT and IN-clause?

                    let participant_id: Option<i64> = get_participant_id_stmt.query_row(
                        params![&chat_id, &member.as_ref()],
                        |row| row.get(0),
                    ).optional()?;

                    if let Some(participant_id) = participant_id {
                        insert_member_stmt.execute(params![
                            &group_id,
                            &participant_id
                        ])?;
                    } else {
                        return Err(
                            DatabaseError::concurrency("the participant was not found").into()
                        );
                    }
                }
            }

            tx.commit()?;

            Ok(())
        };

        block_in_place(|| fn_impl().map_err(|e| map_error("cannot add group members", e)))
    }

    fn remove_group_members_if_exist<T: AsRef<str>>(
        &mut self,
        chat_id: i64,
        group_name: &str,
        members: &[T],
    ) -> DatabaseResult<()> {
        let mut fn_impl = || {
            let tx = self.connection.transaction()?;

            let group_id: i64 = tx.query_row(
                "SELECT id FROM participant_group
                 WHERE chat_id = :chat_id AND name = :group_name AND deleted_at IS NULL",
                params![&chat_id, &group_name],
                |row| row.get(0),
            )?;

            {
                let mut delete_member_stmt = tx.prepare_cached(
                    "UPDATE group_member SET deleted_at = CURRENT_TIMESTAMP
                     WHERE group_id = ?1 AND participant_id =
                         (SELECT id FROM participant WHERE chat_id = ?2 AND name = ?3 AND deleted_at IS NULL)
                     AND deleted_at IS NULL",
                )?;

                // It's unclear how to use an IN clause, so we use a loop
                // https://github.com/rusqlite/rusqlite/issues/345
                for member in members {
                    delete_member_stmt.execute(params![&group_id, &chat_id, &member.as_ref()])?;
                }
            }

            tx.commit()?;

            Ok(())
        };

        block_in_place(|| fn_impl().map_err(|e| map_error("cannot remove group members", e)))
    }

    fn get_groups(&self, chat_id: i64) -> DatabaseResult<Vec<String>> {
        let fn_impl = || {
            let mut stmt = self.connection.prepare_cached(
                "SELECT name FROM participant_group
                 WHERE chat_id = :chat_id AND deleted_at IS NULL",
            )?;

            let group_iter = stmt.query_map(params![&chat_id], |row| row.get(0))?;

            let groups = group_iter.collect::<Result<_, _>>()?;
            Ok(groups)
        };

        block_in_place(|| fn_impl().map_err(|e| map_error("cannot get groups", e)))
    }

    fn group_exists(&self, chat_id: i64, group_name: &str) -> DatabaseResult<bool> {
        let fn_impl = || {
            let group_id: Option<i64> = self
                .connection
                .query_row(
                    "SELECT id FROM participant_group
                     WHERE chat_id = :chat_id AND name = :group_name AND deleted_at IS NULL",
                    params![&chat_id, &group_name],
                    |row| row.get(0),
                )
                .optional()?;

            if group_id.is_none() {
                Ok(false)
            } else {
                Ok(true)
            }
        };

        block_in_place(|| fn_impl().map_err(|e| map_error("cannot check if group exists", e)))
    }

    fn get_group_members(&self, chat_id: i64, group_name: &str) -> DatabaseResult<Vec<String>> {
        let fn_impl = || {
            let mut stmt = self.connection.prepare_cached(
                "SELECT p.name FROM participant_group pg
                         INNER JOIN group_member gm ON pg.id = gm.group_id
                         INNER JOIN participant p ON gm.participant_id = p.id
                         WHERE pg.chat_id = :chat_id
                         AND pg.name = :group_name
                         AND pg.deleted_at IS NULL AND gm.deleted_at IS NULL AND p.deleted_at IS NULL",
            )?;

            let group_member_iter =
                stmt.query_map(params![&chat_id, &group_name], |row| row.get(0))?;

            let group_members = group_member_iter.collect::<Result<_, _>>()?;
            Ok(group_members)
        };

        block_in_place(|| fn_impl().map_err(|e| map_error("cannot get group members", e)))
    }

    fn is_auto_register_active(&self, chat_id: i64) -> Result<bool, DatabaseError> {
        let fn_impl = || {
            let mut stmt = self
                .connection
                .prepare_cached("SELECT auto_register FROM chat_flag WHERE chat_id = :chat_id")?;

            let auto_flag: Option<bool> = stmt
                .query_row(params![&chat_id], |row| row.get(0))
                .optional()?;

            if let Some(auto_flag) = auto_flag {
                Ok(auto_flag)
            } else {
                Ok(false)
            }
        };

        block_in_place(|| fn_impl().map_err(|e| map_error("cannot get auto register flag", e)))
    }

    fn toggle_auto_register(&mut self, chat_id: i64) -> Result<bool, DatabaseError> {
        let auto_register = self.is_auto_register_active(chat_id)?;
        let target_auto_register = !auto_register;

        let mut fn_impl = || {
            let tx = self.connection.transaction()?;

            // Sqlite does not support UPSERT, so we first try to update and if we fail we insert.

            let num_rows_updated = tx.execute(
                "UPDATE chat_flag SET auto_register = ?1 WHERE chat_id = ?2",
                params![&target_auto_register, &chat_id],
            )?;

            if num_rows_updated < 1 {
                // The table did not have an entry for this chat: let's create it now.
                tx.execute(
                    "INSERT INTO chat_flag (chat_id, auto_register) VALUES (?1, ?2)",
                    params![&chat_id, &target_auto_register],
                )?;
            }

            tx.commit()?;

            Ok(target_auto_register)
        };

        block_in_place(|| fn_impl().map_err(|e| map_error("cannot toggle auto register flag", e)))
    }
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

fn map_error<T: AsRef<str>>(message: T, e: anyhow::Error) -> DatabaseError {
    match e.downcast::<DatabaseError>() {
        Ok(e) => e,
        Err(e) => DatabaseError::new(message, e),
    }
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
