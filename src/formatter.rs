//! Produce the strings that are sent as bot messages.
//! The formatting consists in using basic markdown formatting, emojis
//! and composing the actual output string.

use std::iter::repeat;
use teloxide::utils::markdown::{bold, escape};

use crate::types::{Amount, MoneyExchange, SavedExpense, SavedParticipant};

const AMOUNT_TO_FLOAT_DIVISOR: f64 = 100.0;

pub fn format_list_expenses(expenses: &[SavedExpense]) -> String {
    if expenses.is_empty() {
        escape("Nothing to show!")
    } else {
        expenses
            .iter()
            .map(format_expense)
            .fold(String::new(), |a, b| a + &b + "\n")
    }
}

fn format_expense(expense: &SavedExpense) -> String {
    let result = format!(
        "💰  {}: {} {} {}",
        expense.id,
        format_participants(expense, true),
        bold(&escape(&format_amount(expense.amount))),
        format_participants(expense, false)
    );

    if expense.message.is_some() {
        let message = escape(&format!(
            "- {}",
            expense
                .message
                .clone()
                .expect("just checked the option is non-empty!")
        ));
        format!("{} {}", result, message)
    } else {
        result
    }
}

fn format_amount(amount: Amount) -> String {
    let amount = amount as f64 / AMOUNT_TO_FLOAT_DIVISOR;
    format!("{:.2}", amount)
}

fn format_participants(expense: &SavedExpense, are_creditors: bool) -> String {
    expense
        .participants
        .iter()
        .filter(|p| p.is_creditor() == are_creditors)
        .map(|p| escape(&format_participant(p)))
        .fold(String::new(), |a, b| a + &b + " ")
}

fn format_participant(participant: &SavedParticipant) -> String {
    if participant.amount.is_some() {
        let amount = participant
            .amount
            .expect("just checked the amount is non-empty!") as f64
            / AMOUNT_TO_FLOAT_DIVISOR;
        format!("{}/{:.2}", participant.name, amount)
    } else {
        participant.name.to_string()
    }
}

pub fn format_balance(exchanges: &Vec<MoneyExchange>) -> String {
    if exchanges.is_empty() {
        escape("All clean!")
    } else {
        let max_debtor_length = exchanges
            .iter()
            .map(|e| e.debtor.len())
            .max()
            .expect("just checked there are exchanges!");
        exchanges
            .iter()
            .map(|e| format_exchange(e, max_debtor_length))
            .fold(String::new(), |a, b| a + &b + "\n")
    }
}

fn format_exchange(exchange: &MoneyExchange, target_length: usize) -> String {
    // We make sure that the amounts are always aligned, by padding the debtors where needed:
    let debtor = if exchange.debtor.len() < target_length {
        exchange.debtor.clone() + &make_string_of_char(' ', target_length - exchange.debtor.len())
    } else {
        exchange.debtor.clone()
    };

    let amount = exchange.amount as f64 / AMOUNT_TO_FLOAT_DIVISOR;

    format!(
        "💰  {} 💸  {}  {} 🤑",
        debtor,
        bold(&escape(&format!("{:2}", amount))),
        exchange.creditor
    )
}

fn make_string_of_char(c: char, length: usize) -> String {
    repeat(c).take(length).collect::<String>()
}

pub fn format_simple_list<T: AsRef<str>>(elements: &[T]) -> String {
    if elements.is_empty() {
        "Nothing to show!".to_string()
    } else {
        elements
            .iter()
            .map(|g| format!("- {}", g.as_ref()))
            .fold(String::new(), |a, b| a + &b + "\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_expense() {
        let participants = vec![
            SavedParticipant::new_debtor("aa", None),
            SavedParticipant::new_debtor("bbb", Some(123)),
            SavedParticipant::new_creditor("cccc", None),
        ];
        let expense = SavedExpense::new(1, participants, 4343, None);
        let result = format_expense(&expense);
        assert_eq!("💰  1: cccc  *43\\.43* aa bbb/1\\.23 ", result);
    }

    #[test]
    fn test_format_balance() {
        let exchanges = vec![
            MoneyExchange::new("aa", "bb", 3400),
            MoneyExchange::new("aacc", "bb", 2112),
            MoneyExchange::new("abc", "bb", 32323),
        ];

        let result = format_balance(&exchanges);

        assert_eq!("💰  aa   💸  *34*  bb 🤑\n💰  aacc 💸  *21\\.12*  bb 🤑\n💰  abc  💸  *323\\.23*  bb 🤑\n", result);
    }

    #[test]
    fn test_format_simple_list() {
        let elements = vec!["g1", "g2", "g3"];
        let result = format_simple_list(&elements);

        assert_eq!("- g1\n- g2\n- g3\n", result);
    }
}
