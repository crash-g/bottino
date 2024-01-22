//! Functions that check the validity of user input.
//!
//! These functions are called after the parsing phase and execute
//! checks that are not easily done by the parser.

use std::collections::HashSet;
use std::sync::Arc;

use tokio::sync::Mutex;

mod expense;

use crate::error::BotError;
use crate::memory::Memory;
pub use expense::{validate_and_resolve_groups, validate_expense};

/// Check that all participants provided by the user are valid participants.
pub async fn validate_participants_exist<M: Memory, T: AsRef<str>>(
    participants: &[T],
    chat_id: i64,
    memory: &Arc<Mutex<M>>,
) -> Result<(), BotError> {
    if !participants.is_empty() {
        let registered_participants = memory
            .lock()
            .await
            .get_participants(chat_id)
            .map_err(|e| BotError::database("cannot get participants", e))?;

        let registered_participants: HashSet<_> = registered_participants.into_iter().collect();

        for participant in participants {
            if !registered_participants.contains(participant.as_ref()) {
                let user_message =
                    format!("'{}' is not a registered participant", participant.as_ref());
                let message = user_message.clone();
                return Err(BotError::new(message, user_message));
            }
        }
    }
    Ok(())
}

/// Verify that a group with the given name exists.
pub async fn validate_group_exists<M: Memory>(
    group_name: &str,
    chat_id: i64,
    memory: &Arc<Mutex<M>>,
) -> Result<(), BotError> {
    if group_name.trim().is_empty() {
        let user_message = "Missing group name".to_string();
        let message = user_message.clone();
        return Err(BotError::new(message, user_message).into());
    }

    let group_exists = memory
        .lock()
        .await
        .group_exists(chat_id, group_name)
        .map_err(|e| BotError::database("cannot check if group exists", e))?;

    if group_exists {
        Ok(())
    } else {
        let message = format!("The group '{group_name}' does not exist!");
        let user_message = message.clone();
        Err(BotError::new(message, user_message).into())
    }
}
