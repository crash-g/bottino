//! Parse the user input.

mod expense;

pub use expense::parse_expense;

use crate::error::InputError;

pub fn parse_participants(s: &str) -> Result<Vec<String>, InputError> {
    let parts: Vec<_> = s
        .split(' ')
        .filter_map(|x| {
            if x.is_empty() {
                None
            } else {
                Some(x.to_lowercase())
            }
        })
        .collect();
    if parts.is_empty() {
        Err(InputError::participants_not_provided())
    } else {
        Ok(parts)
    }
}

pub fn parse_group_and_members(s: &str) -> Result<(String, Vec<String>), InputError> {
    let mut parts: Vec<_> = s
        .split(' ')
        .filter_map(|x| {
            if x.is_empty() {
                None
            } else {
                Some(x.to_lowercase())
            }
        })
        .collect();
    if parts.is_empty() {
        Err(InputError::group_not_provided())
    } else {
        let members = parts.split_off(1);
        Ok((
            parts
                .pop()
                .expect("Just checked that the Vec contains at least one element"),
            members,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_participants() -> anyhow::Result<()> {
        let participants = parse_participants("p1  P2 P3 ")?;
        assert_eq!(participants, vec!["p1", "p2", "p3"]);

        let result = parse_participants("   ");
        assert!(result.is_err());
        Ok(())
    }

    #[test]
    fn test_parse_group_and_members() -> anyhow::Result<()> {
        let (group_name, members) = parse_group_and_members("g1 p1  P2 p3 ")?;
        assert_eq!(group_name, "g1");
        assert_eq!(members, vec!["p1", "p2", "p3"]);

        let (group_name, members) = parse_group_and_members(" g1  ")?;
        assert_eq!(group_name, "g1");
        assert_eq!(members, Vec::<String>::new());
        Ok(())
    }
}
