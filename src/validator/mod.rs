//! Functions that check the validity of user input.
//!
//! These functions are called after the parsing phase and execute
//! checks that are not easily done by the parser.

use std::collections::HashSet;
use std::sync::Arc;

use tokio::sync::Mutex;

mod expense;

use crate::error::{BotError, InputError};
use crate::memory::Memory;
pub use expense::{validate_and_resolve_groups, validate_expense};

/// Check that all participants provided by the user exist in the database.
pub async fn validate_participants_exist<M: Memory, T: AsRef<str>>(
    participants: &[T],
    chat_id: i64,
    memory: &Arc<Mutex<M>>,
) -> anyhow::Result<()> {
    if !participants.is_empty() {
        let registered_participants = memory
            .lock()
            .await
            .get_participants(chat_id)
            .map_err(|e| BotError::database("cannot get participants", e))?;

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

/// Verify that a group with the given name exists in the database.
pub async fn validate_group_exists<M: Memory>(
    group_name: &str,
    chat_id: i64,
    memory: &Arc<Mutex<M>>,
) -> anyhow::Result<()> {
    if group_name.trim().is_empty() {
        return Err(InputError::group_not_provided().into());
    }

    let group_exists = memory
        .lock()
        .await
        .group_exists(chat_id, group_name)
        .map_err(|e| BotError::database("cannot check if group exists", e))?;

    if group_exists {
        Ok(())
    } else {
        Err(InputError::unregistered_group(group_name.to_string()).into())
    }
}

/// Check that a list of participant names is valid.
pub fn validate_participant_names<T: AsRef<str>>(names: &[T]) -> Result<(), InputError> {
    for name in names {
        if !is_valid_name(name) {
            return Err(InputError::invalid_participant_name(
                name.as_ref().to_string(),
            ));
        }
    }

    Ok(())
}

/// Check that a group name is valid.
pub fn validate_group_name<T: AsRef<str>>(name: T) -> Result<(), InputError> {
    if is_valid_name(&name) {
        Ok(())
    } else {
        Err(InputError::invalid_group_name(name.as_ref().to_string()))
    }
}

fn is_valid_name<T: AsRef<str>>(name: T) -> bool {
    let is_ascii = name.as_ref().is_ascii();
    let is_alphanumeric = name.as_ref().chars().all(char::is_alphanumeric);
    let starts_with_letter = match name.as_ref().chars().next() {
        Some(c) => c.is_alphabetic(),
        None => true,
    };

    is_ascii && is_alphanumeric && starts_with_letter
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_name() {
        // Valid names.
        let name = "p1";
        assert!(is_valid_name(name));
        let name = "P2";
        assert!(is_valid_name(name));
        let name = "Abc";
        assert!(is_valid_name(name));
        let name = "abC";
        assert!(is_valid_name(name));
        let name = "c";
        assert!(is_valid_name(name));
        let name = "";
        assert!(is_valid_name(name));

        // Invalid names.
        let name = "1Abc"; // starts with number
        assert!(!is_valid_name(name));
        let name = "Ab_c"; // contains underscore
        assert!(!is_valid_name(name));
        let name = "pà1"; // contains non-ASCII
        assert!(!is_valid_name(name));
    }
}
