//! The implementation of a data storage using Sqlite.

use chrono::{DateTime, Utc};
use log::debug;
use rusqlite::{params, Connection, OptionalExtension};
use std::{collections::HashMap, path::Path};
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
    pub fn new<P: AsRef<Path>>(path: P) -> DatabaseResult<SqliteDatabase> {
        block_in_place(|| {
            let connection = Connection::open(path)
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

    fn get_expenses(
        &self,
        chat_id: i64,
        only_active: bool,
    ) -> Result<Vec<SavedExpense>, DatabaseError> {
        let fn_impl = || {
            let base_query = "SELECT
                     e.id, e.settled_at is null is_active, e.amount, e.message, e.message_ts,
                     p.name, ep.is_creditor, ep.amount
                 FROM expense e
                 INNER JOIN expense_participant ep ON e.id = ep.expense_id
                 INNER JOIN participant p ON ep.participant_id = p.id
                 WHERE e.chat_id = :chat_id AND e.deleted_at IS NULL";
            let query = if only_active {
                format!("{} AND e.settled_at IS NULL", base_query)
            } else {
                base_query.to_string()
            };
            let mut stmt = self.connection.prepare_cached(&query)?;

            let expense_iter = stmt.query_map(&[(":chat_id", &chat_id)], |row| {
                Ok(GetExpenseQuery {
                    id: row.get(0)?,
                    is_active: row.get(1)?,
                    e_amount: row.get(2)?,
                    e_message: row.get(3)?,
                    e_message_ts: row.get(4)?,
                    p_name: row.get(5)?,
                    p_is_creditor: row.get(6)?,
                    p_amount: row.get(7)?,
                })
            })?;

            let expenses: Result<Vec<_>, _> = expense_iter.collect();
            Ok(parse_expenses_query(expenses?))
        };

        block_in_place(|| fn_impl().map_err(|e| map_error("cannot get expenses", e)))
    }

    fn get_expenses_with_limit(
        &self,
        chat_id: i64,
        start: usize,
        limit: usize,
        only_active: bool,
    ) -> Result<Vec<SavedExpense>, DatabaseError> {
        // It's sort of complex to run the query with a limit, so for now
        // we ask for everything and just slice the result.

        let mut all_expenses = self.get_expenses(chat_id, only_active)?;
        let num_expenses = all_expenses.len();

        all_expenses.sort_by(|e1, e2| e2.id.partial_cmp(&e1.id).expect("cannot sort expenses"));

        if start >= num_expenses {
            Ok(vec![])
        } else {
            let limit = std::cmp::min(start + limit, all_expenses.len());
            Ok(all_expenses[start..limit].to_vec())
        }
    }

    fn mark_all_as_settled(&mut self, chat_id: i64) -> DatabaseResult<()> {
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

    fn delete_expense(&mut self, chat_id: i64, expense_id: i64) -> DatabaseResult<()> {
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
                     WHERE NOT EXISTS (
                         SELECT 1 FROM participant WHERE chat_id = ?1 AND name = ?2 AND deleted_at IS NULL
                     )",
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
                let mut remove_participant_stmt = tx.prepare_cached(
                    "UPDATE participant SET deleted_at = CURRENT_TIMESTAMP
                     WHERE chat_id = ?1 AND name = ?2 AND deleted_at IS NULL",
                )?;

                // It's unclear how to use an IN clause, so we use a loop
                // https://github.com/rusqlite/rusqlite/issues/345
                for participant in participants {
                    remove_participant_stmt.execute(params![&chat_id, &participant.as_ref()])?;
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

    fn participant_exists(
        &self,
        chat_id: i64,
        participant_name: &str,
    ) -> Result<bool, DatabaseError> {
        let fn_impl = || {
            let participant_id: Option<i64> = self
                .connection
                .query_row(
                    "SELECT id FROM participant
                     WHERE chat_id = :chat_id AND name = :participant_name AND deleted_at IS NULL",
                    params![&chat_id, &participant_name],
                    |row| row.get(0),
                )
                .optional()?;

            if participant_id.is_none() {
                Ok(false)
            } else {
                Ok(true)
            }
        };

        block_in_place(|| fn_impl().map_err(|e| map_error("cannot check if participant exists", e)))
    }

    fn add_aliases_if_not_exist<T: AsRef<str>>(
        &mut self,
        chat_id: i64,
        participant: &str,
        aliases: &[T],
    ) -> Result<(), DatabaseError> {
        let mut fn_impl = || {
            let tx = self.connection.transaction()?;

            let participant_id: Option<i64> = tx
                .query_row(
                    "SELECT id FROM participant
                     WHERE chat_id = :chat_id AND name = :participant AND deleted_at IS NULL",
                    params![&chat_id, &participant],
                    |row| row.get(0),
                )
                .optional()?;

            if let Some(participant_id) = participant_id {
                // We cannot use INSERT OR IGNORE because our UNIQUE constraint includes a nullable column,
                // and NULL values are considered distinct (https://www.sqlite.org/nulls.html).
                let mut insert_alias_stmt = tx.prepare_cached(
                    "INSERT INTO alias (chat_id, name, participant_id) SELECT ?1, ?2, ?3
                     WHERE NOT EXISTS (
                         SELECT 1 FROM alias WHERE chat_id = ?1 AND name = ?2 AND deleted_at IS NULL
                     ) AND NOT EXISTS (
                         SELECT 1 FROM participant WHERE chat_id = ?1 AND name = ?2 AND deleted_at IS NULL
                     )",
                )?;
                // It's unclear how to use an IN clause, so we use a loop
                // https://github.com/rusqlite/rusqlite/issues/345
                for alias in aliases {
                    insert_alias_stmt.execute(params![
                        &chat_id,
                        &alias.as_ref(),
                        &participant_id
                    ])?;
                    // Here the insert may be skipped because:
                    // 1. the alias was already present for the given participant
                    // 2. the alias was present for a different participant
                    // Case (1) is OK, but case (2) is a problem. The bot logic will (should?) check this
                    // already, so if it happens it can only be because of concurrency. Still, we have
                    // no way to know the reason without an additional query, which for now we will not do.
                }
            } else {
                return Err(DatabaseError::concurrency("the participant was not found").into());
            }

            tx.commit()?;

            Ok(())
        };

        block_in_place(|| fn_impl().map_err(|e| map_error("cannot add aliases", e)))
    }

    fn remove_aliases_if_exist<T: AsRef<str>>(
        &mut self,
        chat_id: i64,
        participant: &str,
        aliases: &[T],
    ) -> Result<(), DatabaseError> {
        let mut fn_impl = || {
            let tx = self.connection.transaction()?;

            let participant_id: Option<i64> = tx
                .query_row(
                    "SELECT id FROM participant
                 WHERE chat_id = :chat_id AND name = :participant AND deleted_at IS NULL",
                    params![&chat_id, &participant],
                    |row| row.get(0),
                )
                .optional()?;

            if let Some(participant_id) = participant_id {
                let mut remove_alias_stmt = tx.prepare_cached(
                    "UPDATE alias SET deleted_at = CURRENT_TIMESTAMP
                     WHERE name = ?1 AND participant_id = ?2 AND deleted_at IS NULL",
                )?;

                // It's unclear how to use an IN clause, so we use a loop
                // https://github.com/rusqlite/rusqlite/issues/345
                for alias in aliases {
                    remove_alias_stmt.execute(params![&alias.as_ref(), &participant_id])?;
                }
            } else {
                return Err(DatabaseError::concurrency("the participant was not found").into());
            }

            tx.commit()?;

            Ok(())
        };

        block_in_place(|| fn_impl().map_err(|e| map_error("cannot remove aliases", e)))
    }

    fn get_aliases(&self, chat_id: i64) -> Result<HashMap<String, String>, DatabaseError> {
        let fn_impl = || {
            let mut stmt = self.connection.prepare_cached(
                "SELECT a.name, p.name FROM alias a
                 INNER JOIN participant p ON a.participant_id = p.id
                 WHERE a.chat_id = :chat_id AND a.deleted_at IS NULL AND p.deleted_at IS NULL",
            )?;

            let alias_iter = stmt.query_map(params![&chat_id], |row| {
                Ok(AliasQuery {
                    alias_name: row.get(0)?,
                    participant_name: row.get(1)?,
                })
            })?;

            let aliases = alias_iter.collect::<Result<Vec<_>, _>>()?;
            let aliases = aliases
                .into_iter()
                .map(|r| (r.alias_name, r.participant_name))
                .collect::<HashMap<_, _>>();

            Ok(aliases)
        };

        block_in_place(|| fn_impl().map_err(|e| map_error("cannot get aliases", e)))
    }

    fn get_participant_aliases(
        &self,
        chat_id: i64,
        participant: &str,
    ) -> Result<Vec<String>, DatabaseError> {
        let fn_impl = || {
            let mut stmt = self.connection.prepare_cached(
                "SELECT a.name FROM alias a
                 INNER JOIN participant p ON a.participant_id = p.id
                 WHERE a.chat_id = :chat_id AND p.name = :participant_name
                     AND a.deleted_at IS NULL AND p.deleted_at IS NULL",
            )?;

            let alias_iter = stmt.query_map(params![&chat_id, &participant], |row| row.get(0))?;

            let aliases = alias_iter.collect::<Result<Vec<_>, _>>()?;
            Ok(aliases)
        };

        block_in_place(|| fn_impl().map_err(|e| map_error("cannot get participant aliases", e)))
    }

    fn add_group_if_not_exists(&mut self, chat_id: i64, group_name: &str) -> DatabaseResult<()> {
        let fn_impl = || {
            // We cannot use INSERT OR IGNORE because our UNIQUE constraint includes a nullable column,
            // and NULL values are considered distinct (https://www.sqlite.org/nulls.html).
            self.connection.execute(
                "INSERT INTO participant_group (chat_id, name) SELECT ?1, ?2
                 WHERE NOT EXISTS (
                     SELECT 1 FROM participant_group WHERE chat_id = ?1 AND name = ?2 AND deleted_at IS NULL
                 )",
                params![&chat_id, &group_name],
            )?;

            Ok(())
        };

        block_in_place(|| fn_impl().map_err(|e| map_error("cannot create group", e)))
    }

    fn remove_group_if_exists(&mut self, chat_id: i64, group_name: &str) -> DatabaseResult<()> {
        debug!("Removing group. Chat ID: {chat_id}. Group name: {group_name}");
        let fn_impl = || {
            let mut remove_group_stmt = self.connection.prepare_cached(
                "UPDATE participant_group SET deleted_at = CURRENT_TIMESTAMP
                 WHERE chat_id = ?1 AND name = ?2 AND deleted_at IS NULL",
            )?;

            remove_group_stmt.execute(params![&chat_id, group_name])?;

            Ok(())
        };

        block_in_place(|| fn_impl().map_err(|e| map_error("cannot remove group", e)))
    }

    fn add_group_members_if_not_exist<T: AsRef<str>>(
        &mut self,
        chat_id: i64,
        group_name: &str,
        members: &[T],
    ) -> DatabaseResult<()> {
        let mut fn_impl = || {
            let tx = self.connection.transaction()?;

            let group_id: Option<i64> = tx
                .query_row(
                    "SELECT id FROM participant_group
                 WHERE chat_id = :chat_id AND name = :group_name AND deleted_at IS NULL",
                    params![&chat_id, &group_name],
                    |row| row.get(0),
                )
                .optional()?;

            if let Some(group_id) = group_id {
                let mut get_participant_id_stmt = tx.prepare_cached(
                    "SELECT id FROM participant
                     WHERE chat_id = :chat_id AND name = :member AND deleted_at IS NULL",
                )?;

                // We cannot use INSERT OR IGNORE because our UNIQUE constraint includes a nullable column,
                // and NULL values are considered distinct (https://www.sqlite.org/nulls.html).
                let mut insert_member_stmt = tx.prepare_cached(
                    "INSERT INTO group_member (group_id, participant_id) SELECT ?1, ?2
                     WHERE NOT EXISTS (
                         SELECT 1 FROM group_member WHERE group_id = ?1 AND participant_id = ?2 AND deleted_at IS NULL
                     )",
                )?;
                // It's unclear how to use an IN clause, so we use a loop
                // https://github.com/rusqlite/rusqlite/issues/345
                for member in members {
                    // TODO: here we run two queries per member, it would be nice to optimize it,
                    // but can we do it considering the restrictions on UPSERT and IN-clause?

                    let participant_id: Option<i64> = get_participant_id_stmt
                        .query_row(params![&chat_id, &member.as_ref()], |row| row.get(0))
                        .optional()?;

                    if let Some(participant_id) = participant_id {
                        insert_member_stmt.execute(params![&group_id, &participant_id])?;
                    } else {
                        return Err(
                            DatabaseError::concurrency("the participant was not found").into()
                        );
                    }
                }
            } else {
                return Err(DatabaseError::concurrency("the group was not found").into());
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

            let group_id: Option<i64> = tx
                .query_row(
                    "SELECT id FROM participant_group
                 WHERE chat_id = :chat_id AND name = :group_name AND deleted_at IS NULL",
                    params![&chat_id, &group_name],
                    |row| row.get(0),
                )
                .optional()?;

            if let Some(group_id) = group_id {
                let mut remove_member_stmt = tx.prepare_cached(
                    "UPDATE group_member SET deleted_at = CURRENT_TIMESTAMP
                     WHERE group_id = ?1 AND participant_id =
                         (SELECT id FROM participant WHERE chat_id = ?2 AND name = ?3 AND deleted_at IS NULL)
                     AND deleted_at IS NULL",
                )?;

                // It's unclear how to use an IN clause, so we use a loop
                // https://github.com/rusqlite/rusqlite/issues/345
                for member in members {
                    remove_member_stmt.execute(params![&group_id, &chat_id, &member.as_ref()])?;
                }
            } else {
                return Err(DatabaseError::concurrency("the group was not found").into());
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

fn parse_expenses_query(expenses: Vec<GetExpenseQuery>) -> Vec<SavedExpense> {
    let mut result = HashMap::new();
    for expense in expenses {
        let entry = result.entry(expense.id).or_insert_with(|| {
            SavedExpense::new(
                expense.id,
                expense.is_active,
                vec![],
                expense.e_amount,
                expense.e_message,
                expense.e_message_ts,
            )
        });

        let name = &expense.p_name;
        let amount = expense.p_amount;
        let participant = if expense.p_is_creditor {
            SavedParticipant::new_creditor(name, amount)
        } else {
            SavedParticipant::new_debtor(name, amount)
        };
        entry.participants.push(participant);
    }

    result.into_iter().map(|(_, e)| e).collect()
}

struct GetExpenseQuery {
    id: i64,
    is_active: bool,
    e_amount: i64,
    e_message: Option<String>,
    e_message_ts: DateTime<Utc>,
    p_name: String,
    p_is_creditor: bool,
    p_amount: Option<i64>,
}

struct AliasQuery {
    alias_name: String,
    participant_name: String,
}

fn map_error<T: AsRef<str>>(message: T, e: anyhow::Error) -> DatabaseError {
    match e.downcast::<DatabaseError>() {
        Ok(e) => e,
        Err(e) => DatabaseError::new(message, e),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use tempdir::TempDir;

    use crate::types::ParsedParticipant;

    use super::*;

    fn temp_database() -> anyhow::Result<(SqliteDatabase, TempDir)> {
        let tmp_dir = TempDir::new("treasurer")?;
        println!("Temporary directory is: {:#?}", tmp_dir);

        let file_path = tmp_dir.path().join("treasurer.db");
        let database = SqliteDatabase::new(file_path)?;
        Ok((database, tmp_dir))
    }

    fn to_hash_set<S: AsRef<str>>(v: Vec<S>) -> HashSet<String> {
        v.into_iter().map(|s| s.as_ref().to_string()).collect()
    }

    #[test]
    #[ignore]
    fn test_get_expenses_with_limit() -> anyhow::Result<()> {
        let (mut database, _tmp_dir) = temp_database()?;

        let chat_id = 1;

        let participants = &["aa", "bb", "cc", "dd", "ee"];
        database.add_participants_if_not_exist(chat_id, participants)?;

        // Add expenses.
        let expense = ParsedExpense::new(
            vec![
                ParsedParticipant::new_creditor("aa", None),
                ParsedParticipant::new_creditor("bb", None),
                ParsedParticipant::new_debtor("cc", None),
                ParsedParticipant::new_debtor("dd", None),
            ],
            1,
            None,
        );
        database.save_expense_with_message(chat_id, expense, DateTime::<Utc>::MIN_UTC)?;
        let expense = ParsedExpense::new(
            vec![
                ParsedParticipant::new_creditor("aa", None),
                ParsedParticipant::new_debtor("dd", None),
            ],
            2,
            None,
        );
        database.save_expense_with_message(chat_id, expense, DateTime::<Utc>::MIN_UTC)?;
        let expense = ParsedExpense::new(
            vec![
                ParsedParticipant::new_creditor("bb", None),
                ParsedParticipant::new_debtor("aa", None),
            ],
            3,
            None,
        );
        database.save_expense_with_message(chat_id, expense, DateTime::<Utc>::MIN_UTC)?;
        // Reset and add one more.
        database.mark_all_as_settled(chat_id)?;
        let expense = ParsedExpense::new(
            vec![
                ParsedParticipant::new_creditor("dd", None),
                ParsedParticipant::new_debtor("bb", None),
            ],
            4,
            None,
        );
        database.save_expense_with_message(chat_id, expense, DateTime::<Utc>::MIN_UTC)?;

        // Asking only active expenses returns one element.
        let expenses = database.get_expenses_with_limit(chat_id, 0, 2, true)?;
        assert_eq!(1, expenses.len());
        assert_eq!(4, expenses.get(0).unwrap().amount);

        // Asking all expenses return them all.
        let expenses = database.get_expenses_with_limit(chat_id, 0, 10, false)?;
        assert_eq!(4, expenses.len());

        // Asking all expenses with low limit returns only some (newest first).
        let expenses = database.get_expenses_with_limit(chat_id, 0, 2, false)?;
        assert_eq!(2, expenses.len());
        assert_eq!(4, expenses.get(0).unwrap().amount);
        assert_eq!(3, expenses.get(1).unwrap().amount);

        Ok(())
    }

    #[test]
    #[ignore]
    fn test_add_and_remove_participants() -> anyhow::Result<()> {
        let (mut database, _tmp_dir) = temp_database()?;

        let chat_id = 1;

        let participants = vec!["aa", "bb"];
        database.add_participants_if_not_exist(chat_id, &participants)?;

        let saved_participants = database.get_participants(chat_id)?;
        assert_eq!(2, saved_participants.len());
        assert_eq!(to_hash_set(participants), to_hash_set(saved_participants));

        // Now add duplicates.

        let participants = &["aa", "cc"];
        database.add_participants_if_not_exist(chat_id, participants)?;
        assert_eq!(3, database.get_participants(chat_id)?.len());

        // Now remove one participant and then add it back.

        database.remove_participants_if_exist(chat_id, &["bb"])?;
        assert_eq!(2, database.get_participants(chat_id)?.len());

        database.add_participants_if_not_exist(chat_id, &["bb"])?;
        assert_eq!(3, database.get_participants(chat_id)?.len());

        {
            let mut stmt = database
                .connection
                .prepare("SELECT name FROM participant WHERE chat_id = :chat_id")?;

            let iter = stmt.query_map(params![&chat_id], |row| row.get(0))?;

            let saved_participants: Vec<String> = iter.collect::<Result<_, _>>()?;
            assert_eq!(4, saved_participants.len());
            assert_eq!(
                to_hash_set(vec!["aa", "bb", "cc"]),
                to_hash_set(saved_participants)
            );
        }

        Ok(())
    }

    #[test]
    #[ignore]
    fn test_add_groups() -> anyhow::Result<()> {
        let (mut database, _tmp_dir) = temp_database()?;

        let chat_id = 1;

        let participants = &["aa", "bb", "cc", "dd", "ee"];
        database.add_participants_if_not_exist(chat_id, participants)?;

        let group1 = "all";
        database.add_group_if_not_exists(chat_id, group1)?;
        database.add_group_members_if_not_exist(chat_id, group1, &["aa", "bb"])?;

        let group2 = "g2";
        database.add_group_if_not_exists(chat_id, group2)?;

        assert_eq!(
            to_hash_set(vec!["all", "g2"]),
            to_hash_set(database.get_groups(chat_id)?)
        );

        database.add_group_members_if_not_exist(chat_id, group2, &["cc", "bb", "ee", "dd"])?;
        database.add_group_members_if_not_exist(chat_id, group1, &["bb", "dd"])?;

        database.remove_group_members_if_exist(chat_id, group2, &["dd"])?;
        database.remove_participants_if_exist(chat_id, &["aa", "cc"])?;

        let all_members = database.get_group_members(chat_id, "all")?;
        assert_eq!(2, all_members.len());
        assert_eq!(to_hash_set(vec!["bb", "dd"]), to_hash_set(all_members));

        let g2_members = database.get_group_members(chat_id, group2)?;
        assert_eq!(2, g2_members.len());
        assert_eq!(to_hash_set(vec!["ee", "bb"]), to_hash_set(g2_members));

        Ok(())
    }

    #[test]
    fn test_conversion() {
        let expenses = vec![
            GetExpenseQuery {
                id: 1,
                is_active: true,
                e_amount: 300,
                e_message: None,
                e_message_ts: DateTime::<Utc>::MIN_UTC,
                p_name: "name1".to_string(),
                p_is_creditor: true,
                p_amount: None,
            },
            GetExpenseQuery {
                id: 1,
                is_active: true,
                e_amount: 300,
                e_message: None,
                e_message_ts: DateTime::<Utc>::MIN_UTC,
                p_name: "name2".to_string(),
                p_is_creditor: false,
                p_amount: None,
            },
            GetExpenseQuery {
                id: 1,
                is_active: true,
                e_amount: 300,
                e_message: None,
                e_message_ts: DateTime::<Utc>::MIN_UTC,
                p_name: "name3".to_string(),
                p_is_creditor: false,
                p_amount: Some(100),
            },
            GetExpenseQuery {
                id: 2,
                is_active: true,
                e_amount: 5400,
                e_message: None,
                e_message_ts: DateTime::<Utc>::MIN_UTC,
                p_name: "name1".to_string(),
                p_is_creditor: true,
                p_amount: None,
            },
            GetExpenseQuery {
                id: 2,
                is_active: true,
                e_amount: 5400,
                e_message: None,
                e_message_ts: DateTime::<Utc>::MIN_UTC,
                p_name: "name2".to_string(),
                p_is_creditor: false,
                p_amount: None,
            },
        ];

        let result = parse_expenses_query(expenses);
        dbg!("{}", result);
    }
}
