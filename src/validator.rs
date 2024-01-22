//! Functions that check the validity of user input.
//!
//! These functions are called after the parsing phase and execute
//! checks that are not easily done by the parser.

use std::collections::HashSet;
use std::sync::Arc;

use tokio::sync::Mutex;

use crate::error::BotError;
use crate::memory::Memory;
use crate::types::{ParsedExpense, ParsedParticipant};

/// Check that groups do not have custom amount set and replace them with their members.
pub async fn validate_and_resolve_groups<M: Memory>(
    mut expense: ParsedExpense,
    chat_id: i64,
    memory: &Arc<Mutex<M>>,
) -> Result<ParsedExpense, BotError> {
    for participant in &expense.participants {
        if participant.is_group() && participant.amount.is_some() {
            return Err(BotError::new(
                format!(
                    "there is a group with a custom amount in this expense!\n{:#?}",
                    expense
                ),
                "custom amounts are not allowed for groups!".to_string(),
            ));
        }
    }

    let mut participants = Vec::with_capacity(expense.participants.len());

    for participant in expense.participants {
        if participant.is_group() {
            let members = memory
                .lock()
                .await
                .get_group_members(chat_id, &participant.name)
                .map_err(|e| BotError::database("cannot get group members", e))?;

            for member in members {
                let p = if participant.is_creditor() {
                    ParsedParticipant::new_creditor(&member, None)
                } else {
                    ParsedParticipant::new_debtor(&member, None)
                };
                participants.push(p);
            }
        } else {
            participants.push(participant);
        }
    }

    expense.participants = participants;
    Ok(expense)
}

/// Some sanity checks on the expense that was submitted.
///
/// List of checks:
/// - there is at least one participant
/// - there is at least one creditor (which is also automatically a debtor)
/// - the total fixed credit is less or equal to the total amount
/// - the total fixed credit is equal to the total amount when all creditors are fixed
/// - the total fixed debt is less or equal to the total amount
/// - the total fixed debt is equal to the total amount when all debtors are fixed
/// - a creditor appears at most once with a custom amount
/// - a debtor appears at most once with a custom amount
pub fn validate_expense(expense: &ParsedExpense) -> Result<(), BotError> {
    let amount = expense.amount;

    let no_participants = expense.participants.is_empty();
    let no_creditors = !expense.participants.iter().any(|p| p.is_creditor());

    let only_fixed_creditors = expense
        .participants
        .iter()
        .filter(|p| p.is_creditor())
        .all(|p| p.amount.is_some());
    let only_fixed_debtors = are_all_debtors_fixed(&expense.participants);

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

    let creditors_have_multiple_custom_amount = has_multiple_custom_amounts(
        expense
            .participants
            .iter()
            .filter(|p| p.is_creditor())
            .collect(),
    );
    let debtors_have_multiple_custom_amount = has_multiple_custom_amounts(
        expense
            .participants
            .iter()
            .filter(|p| p.is_debtor())
            .collect(),
    );

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
    } else if total_debt < amount && only_fixed_debtors {
        Err(BotError::new(
            format!(
                "all debtors owe a fixed amount and the total is less than the expense amount!\n{:#?}",
                expense
            ),
            "all debtors owe a fixed amount and the total is less than the expense amount!".to_string()
        ))
    } else if creditors_have_multiple_custom_amount {
        Err(BotError::new(
            format!(
                "there are creditors appearing multiple times with custom amounts!\n{:#?}",
                expense
            ),
            "there are creditors appearing multiple times with custom amounts!".to_string(),
        ))
    } else if debtors_have_multiple_custom_amount {
        Err(BotError::new(
            format!(
                "there are debtors appearing multiple times with custom amounts!\n{:#?}",
                expense
            ),
            "there are debtors appearing multiple times with custom amounts!".to_string(),
        ))
    } else {
        Ok(())
    }
}

/// This is more difficult than checking if all creditors are fixed,
/// because a creditor is automatically a debtor. The only way that all
/// debtors can be fixed is if all debtors are fixed and all creditors also
/// appear as debtors.
fn are_all_debtors_fixed(participants: &[ParsedParticipant]) -> bool {
    let only_fixed_debtors = participants
        .iter()
        .filter(|p| p.is_debtor())
        .all(|p| p.amount.is_some());

    let debtors: HashSet<_> = participants
        .iter()
        .filter_map(|p| {
            if p.is_debtor() {
                Some(&p.name)
            } else {
                None
            }
        })
        .collect();

    let all_creditors_appear_as_debtors = participants
        .iter()
        .filter(|p| p.is_creditor())
        .all(|p| debtors.contains(&p.name));

    only_fixed_debtors && all_creditors_appear_as_debtors
}

/// Check if there are a participant present more than once with
/// a custom amount.
fn has_multiple_custom_amounts(participants: Vec<&ParsedParticipant>) -> bool {
    // We use the fact that a HashSet returns false upon insertion if the element
    // is present. We only try to insert a name if the participant has a custom amount.
    let mut uniq = HashSet::new();
    let result = !participants.iter().all(|x| {
        if x.amount.is_some() {
            uniq.insert(&x.name)
        } else {
            true
        }
    });
    result
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
    fn test_are_all_debtors_fixed() {
        let participants = vec![
            ParsedParticipant::new_creditor("a", Some(3)),
            ParsedParticipant::new_debtor("b", Some(3)),
            ParsedParticipant::new_debtor("a", Some(2)),
        ];
        assert_eq!(true, are_all_debtors_fixed(&participants));

        let participants = vec![
            ParsedParticipant::new_creditor("a", Some(3)),
            ParsedParticipant::new_debtor("b", Some(3)),
        ];
        assert_eq!(false, are_all_debtors_fixed(&participants));

        let participants = vec![
            ParsedParticipant::new_creditor("a", Some(3)),
            ParsedParticipant::new_debtor("b", Some(3)),
            ParsedParticipant::new_debtor("a", Some(2)),
            ParsedParticipant::new_debtor("c", None),
        ];
        assert_eq!(false, are_all_debtors_fixed(&participants));
    }

    #[test]
    fn test_multiple_custom_amounts() {
        let participants = vec![
            ParsedParticipant::new_creditor("a", None),
            ParsedParticipant::new_creditor("b", None),
            ParsedParticipant::new_creditor("a", None),
        ];
        let expense = ParsedExpense::new(participants, 33, None);
        assert!(validate_expense(&expense).is_ok());

        let participants = vec![
            ParsedParticipant::new_creditor("a", None),
            ParsedParticipant::new_debtor("a", None),
            ParsedParticipant::new_debtor("b", None),
            ParsedParticipant::new_debtor("a", None),
        ];
        let expense = ParsedExpense::new(participants, 33, None);
        assert!(validate_expense(&expense).is_ok());

        let participants = vec![
            ParsedParticipant::new_creditor("a", Some(3)),
            ParsedParticipant::new_creditor("b", None),
            ParsedParticipant::new_creditor("a", Some(3)),
        ];
        let expense = ParsedExpense::new(participants, 33, None);
        assert!(validate_expense(&expense).is_err());

        let participants = vec![
            ParsedParticipant::new_creditor("a", None),
            ParsedParticipant::new_debtor("a", Some(2)),
            ParsedParticipant::new_debtor("b", None),
            ParsedParticipant::new_debtor("a", Some(3)),
        ];
        let expense = ParsedExpense::new(participants, 33, None);
        assert!(validate_expense(&expense).is_err());

        let participants = vec![
            ParsedParticipant::new_creditor("a", Some(3)),
            ParsedParticipant::new_creditor("c", None),
            ParsedParticipant::new_debtor("a", Some(2)),
            ParsedParticipant::new_debtor("b", None),
            ParsedParticipant::new_debtor("a", None),
        ];
        let expense = ParsedExpense::new(participants, 33, None);
        assert!(validate_expense(&expense).is_ok());
    }
}
