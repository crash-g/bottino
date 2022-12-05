//! Functions that check the validity of user input.
//!
//! These functions are called after the parsing phase and execute
//! checks that are not easily done by the parser.

mod database;
mod expense;

use crate::error::InputError;
pub use database::{
    validate_aliases_do_not_exist, validate_aliases_exist, validate_group_exists,
    validate_participant_exists, validate_participants_exist,
};
pub use expense::{validate_expense, validate_groups};

/// Check that a list of participant names is valid.
pub fn validate_participant_names<T: AsRef<str>>(names: &[T]) -> Result<(), InputError> {
    for name in names {
        if !is_valid_name(name.as_ref()) {
            return Err(InputError::invalid_participant_name(
                name.as_ref().to_string(),
            ));
        }
    }

    Ok(())
}

/// Check that a participant name is valid.
pub fn validate_participant_name(name: &str) -> Result<(), InputError> {
    if is_valid_name(&name) {
        Ok(())
    } else {
        Err(InputError::invalid_participant_name(name.to_string()))
    }
}

/// Check that a list of alias names is valid.
pub fn validate_alias_names<T: AsRef<str>>(names: &[T]) -> Result<(), InputError> {
    for name in names {
        if !is_valid_name(name.as_ref()) {
            return Err(InputError::invalid_alias_name(name.as_ref().to_string()));
        }
    }

    Ok(())
}

/// Check that a group name is valid.
pub fn validate_group_name(name: &str) -> Result<(), InputError> {
    if is_valid_name(&name) {
        Ok(())
    } else {
        Err(InputError::invalid_group_name(name.to_string()))
    }
}

/// Check that a name is valid: the name can be the name of a participant,
/// of an alias or of a group. There cannot be '@' or '#' at the start.
pub fn is_valid_name(name: &str) -> bool {
    let is_alphanumeric = name.chars().all(char::is_alphanumeric);
    let starts_with_letter = match name.chars().next() {
        Some(c) => c.is_alphabetic(),
        None => true,
    };

    is_alphanumeric && starts_with_letter
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
        let name = "àẽë";
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
    }
}
