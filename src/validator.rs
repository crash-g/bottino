//! Functions that check the validity of user input.
//!
//! These functions are called after the parsing phase and execute
//! checks that are not easily done by the parser.

use std::collections::HashSet;
use std::hash::Hash;

use crate::error::BotError;
use crate::types::ParsedExpense;

/// Some sanity checks on the expense that was submitted.
pub fn validate_expense(expense: &ParsedExpense) -> Result<(), BotError> {
    let amount = expense.amount;

    let no_participants = expense.participants.is_empty();
    let no_creditors = expense
        .participants
        .iter()
        .filter(|p| p.is_creditor())
        .peekable()
        .peek()
        .is_none();

    let has_duplicate_creditors = has_duplicates(
        expense
            .participants
            .iter()
            .filter(|p| p.is_creditor())
            .map(|p| p.name.to_lowercase()),
    );
    let has_duplicate_debtors = has_duplicates(
        expense
            .participants
            .iter()
            .filter(|p| p.is_debtor())
            .map(|p| p.name.to_lowercase()),
    );

    let only_fixed_creditors = expense
        .participants
        .iter()
        .filter(|p| p.is_creditor() && p.amount.is_none())
        .peekable()
        .peek()
        .is_none();

    let total_credit: i64 = expense
        .participants
        .iter()
        .filter(|p| p.is_creditor() && p.amount.is_some())
        .map(|p| p.amount.expect("just checked the amount is non-empty"))
        .sum();
    let total_debt: i64 = expense
        .participants
        .iter()
        .filter(|p| p.is_debtor() && p.amount.is_some())
        .map(|p| p.amount.expect("just checked the amount is non-empty"))
        .sum();

    if no_participants {
        Err(BotError::new(
            format!(
                "there are neither debtors nor creditors in this expense!\n{:#?}",
                expense
            ),
            "there are neither debtors nor creditors in this expense!".to_string(),
        ))
    } else if no_creditors {
        Err(BotError::new(
            format!("there are no creditors in this expense!\n{:#?}", expense),
            "there are no creditors in this expense!".to_string(),
        ))
    } else if total_credit > amount {
        Err(BotError::new(
            format!(
                "the money that people paid are more than the total expense amount!\n{:#?}",
                expense
            ),
            "the money that people paid are more than the total expense amount!".to_string(),
        ))
    } else if total_credit < amount && only_fixed_creditors {
        Err(BotError::new(
            format!(
                "all creditors paid a fixed amount and the total is less than the expense amount!\n{:#?}",
                expense
            ),
            "all creditors paid a fixed amount and the total is less than the expense amount!".to_string()
        ))
    } else if total_debt > amount {
        Err(BotError::new(
            format!(
                "the money owed by people are more than the total expense amount!\n{:#?}",
                expense
            ),
            "the money owed by people are more than the total expense amount!".to_string(),
        ))
    } else if has_duplicate_creditors {
        Err(BotError::new(
            format!("there are creditors with the same name!\n{:#?}", expense),
            "there are creditors with the same name!".to_string(),
        ))
    } else if has_duplicate_debtors {
        Err(BotError::new(
            format!("there are debtors with the same name!\n{:#?}", expense),
            "there are debtors with the same name!".to_string(),
        ))
    } else {
        Ok(())
    }
}

fn has_duplicates<T>(iter: T) -> bool
where
    T: IntoIterator,
    T::Item: Eq + Hash,
{
    let mut uniq = HashSet::new();
    !iter.into_iter().all(move |x| uniq.insert(x))
}

#[cfg(test)]
mod tests {
    use crate::types::ParsedParticipant;

    use super::*;

    #[test]
    fn test_no_participants() {
        let expense = ParsedExpense {
            participants: vec![],
            amount: 33,
            message: None,
        };
        assert!(validate_expense(&expense).is_err());
    }

    #[test]
    fn test_duplicates() {
        let participants = vec![
            ParsedParticipant::new_creditor("a", None),
            ParsedParticipant::new_creditor("b", None),
            ParsedParticipant::new_creditor("a", None),
        ];
        let expense = ParsedExpense::new(participants, 33, None);
        assert!(validate_expense(&expense).is_err());

        let participants = vec![
            ParsedParticipant::new_debtor("a", None),
            ParsedParticipant::new_debtor("b", None),
            ParsedParticipant::new_debtor("a", None),
        ];
        let expense = ParsedExpense::new(participants, 33, None);
        assert!(validate_expense(&expense).is_err());

        let participants = vec![
            ParsedParticipant::new_debtor("a", None),
            ParsedParticipant::new_debtor("b", None),
            ParsedParticipant::new_creditor("a", None),
        ];
        let expense = ParsedExpense::new(participants, 33, None);
        assert!(validate_expense(&expense).is_ok());
    }
}
