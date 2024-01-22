//! Parse the user input.

mod expense;

pub use expense::parse_expense;

use crate::error::BotError;

pub fn parse_group_and_members(s: &str) -> Result<(String, Vec<String>), BotError> {
    let mut parts: Vec<_> = s
        .split(' ')
        .filter_map(|x| {
            if x.is_empty() {
                None
            } else {
                Some(x.to_string())
            }
        })
        .collect();
    if parts.is_empty() {
        Err(BotError::new(
            format!("Missing group name: {}", s),
            "Missing group name. Format must be 'group_name [member_name...]'".to_string(),
        ))
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
    fn test_parse_group_and_members() -> anyhow::Result<()> {
        let (group_name, members) = parse_group_and_members("g1 p1  p2 p3 ")?;
        assert_eq!(group_name, "g1");
        assert_eq!(members, vec!["p1", "p2", "p3"]);

        let (group_name, members) = parse_group_and_members(" g1  ")?;
        assert_eq!(group_name, "g1");
        assert_eq!(members, Vec::<String>::new());
        Ok(())
    }
}
