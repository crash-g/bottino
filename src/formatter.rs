//! Produce the strings that are sent as bot messages.
//! The formatting consists in using basic markdown formatting, emojis
//! and composing the actual output string.

use std::iter::repeat;
use teloxide::utils::markdown::{bold, escape};

use crate::types::{Amount, ExpenseWithId, MoneyExchange, Participant, ParticipantMode};

const AMOUNT_TO_FLOAT_DIVISOR: f64 = 100.0;

pub fn format_list_expenses(expenses: &[ExpenseWithId]) -> String {
    if expenses.is_empty() {
        escape("Nothing to show!")
    } else {
        expenses
            .iter()
            .map(format_expense)
            .fold(String::new(), |a, b| a + &b + "\n")
    }
}

fn format_expense(expense: &ExpenseWithId) -> String {
    let result = format!(
        "💰  {}: {} {} {}",
        expense.id,
        format_participants(expense, true),
        bold(&escape(&format_amount(expense.amount))),
        format_participants(expense, false)
    );

    if expense.message.is_some() {
        let message = escape(&format!("- {}", expense.message.clone().unwrap()));
        format!("{} {}", result, message)
    } else {
        result
    }
}

fn format_amount(amount: Amount) -> String {
    let amount = amount as f64 / AMOUNT_TO_FLOAT_DIVISOR;
    format!("{:.2}", amount)
}

fn format_participants(expense: &ExpenseWithId, are_creditors: bool) -> String {
    let mode = if are_creditors {
        ParticipantMode::Creditor
    } else {
        ParticipantMode::Debtor
    };
    expense
        .participants
        .iter()
        .filter(|p| p.mode == mode)
        .map(|p| escape(&format_participant(p)))
        .fold(String::new(), |a, b| a + &b + " ")
}

fn format_participant(participant: &Participant) -> String {
    if participant.amount.is_some() {
        let amount = participant.amount.unwrap() as f64 / AMOUNT_TO_FLOAT_DIVISOR;
        format!("{}/{:.2}", participant.name, amount)
    } else {
        participant.name.to_string()
    }
}

pub fn format_balance(exchanges: &Vec<MoneyExchange>) -> String {
    if exchanges.is_empty() {
        escape("All clean!")
    } else {
        let max_debtor_length = exchanges.iter().map(|e| e.debtor.len()).max().unwrap();
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_expense() {
        let participants = vec![
            Participant::new_debtor("aa", None),
            Participant::new_debtor("bbb", Some(123)),
            Participant::new_creditor("cccc", None),
        ];
        let expense = ExpenseWithId::new(1, participants, 4343, None);
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
}