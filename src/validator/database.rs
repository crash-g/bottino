//! Functions that check the validity of user input by running query to the database.
//!
//! These checks are necessary in order to return nice error messages,
//! but the database should still re-run the checks and throw an error when the actual
//! query is run (in that case, a generic concurrency error is enough).

use std::collections::HashSet;
use std::sync::Arc;

use crate::database::Database;
use crate::error::InputError;
use tokio::sync::Mutex;

/// Check that all participants provided by the user exist in the database.
pub async fn validate_participants_exist<D: Database, T: AsRef<str>>(
    participants: &[T],
    chat_id: i64,
    database: &Arc<Mutex<D>>,
) -> anyhow::Result<()> {
    if !participants.is_empty() {
        let registered_participants = database.lock().await.get_participants(chat_id)?;

        let registered_participants: HashSet<_> = registered_participants.into_iter().collect();

        for participant in participants {
            if !registered_participants.contains(participant.as_ref()) {
                return Err(
                    InputError::unregistered_participant(participant.as_ref().to_string()).into(),
                );
            }
        }
    }
    Ok(())
}

/// Check that a participant provided by the user exists in the database.
pub async fn validate_participant_exists<D: Database>(
    participant: &str,
    chat_id: i64,
    database: &Arc<Mutex<D>>,
) -> anyhow::Result<()> {
    if database
        .lock()
        .await
        .participant_exists(chat_id, participant)?
    {
        Ok(())
    } else {
        Err(InputError::unregistered_participant(participant.to_string()).into())
    }
}

/// Check that all aliases provided by the user are not already registered as participants or aliases.
pub async fn validate_aliases_do_not_exist<D: Database, T: AsRef<str>>(
    participant: &str,
    aliases: &[T],
    chat_id: i64,
    database: &Arc<Mutex<D>>,
) -> anyhow::Result<()> {
    if !aliases.is_empty() {
        let registered_participants = database.lock().await.get_participants(chat_id)?;
        let registered_participants: HashSet<_> = registered_participants.into_iter().collect();

        let registered_aliases = database.lock().await.get_aliases(chat_id)?;

        for alias in aliases {
            if registered_participants.contains(alias.as_ref()) {
                return Err(InputError::alias_registered_as_participant(
                    alias.as_ref().to_string(),
                )
                .into());
            }
            if let Some(p) = registered_aliases.get(alias.as_ref()) {
                if p != participant {
                    return Err(InputError::alias_registered_as_alias(
                        alias.as_ref().to_string(),
                        p.to_string(),
                    )
                    .into());
                }
            }
        }
    }
    Ok(())
}

/// Check that all aliases provided by the user are aliases of the given participant.
pub async fn validate_aliases_exist<D: Database, T: AsRef<str>>(
    participant: &str,
    aliases: &[T],
    chat_id: i64,
    database: &Arc<Mutex<D>>,
) -> anyhow::Result<()> {
    if !aliases.is_empty() {
        let registered_aliases = database
            .lock()
            .await
            .get_participant_aliases(chat_id, participant)?;
        let registered_aliases = registered_aliases.into_iter().collect::<HashSet<_>>();

        for alias in aliases {
            if registered_aliases.contains(alias.as_ref()) {
                return Err(InputError::alias_not_registered_as_alias(
                    alias.as_ref().to_string(),
                    participant.to_string(),
                )
                .into());
            }
        }
    }
    Ok(())
}

/// Verify that a group with the given name exists in the database.
pub async fn validate_group_exists<D: Database>(
    group_name: &str,
    chat_id: i64,
    database: &Arc<Mutex<D>>,
) -> anyhow::Result<()> {
    if group_name.trim().is_empty() {
        return Err(InputError::group_not_provided().into());
    }

    let group_exists = database.lock().await.group_exists(chat_id, group_name)?;

    if group_exists {
        Ok(())
    } else {
        Err(InputError::unregistered_group(group_name.to_string()).into())
    }
}
