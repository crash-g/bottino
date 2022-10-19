use std::collections::{HashMap, HashSet};

use log::Level::Debug;
use log::{debug, log_enabled, warn};

use crate::types::{ExpenseWithId, MoneyExchange};

pub fn compute_exchanges(expenses: Vec<ExpenseWithId>) -> Vec<MoneyExchange> {
    let balance = compute_debts_and_credits(expenses);
    let mut debtors: Vec<_> = balance
        .iter()
        .filter_map(|(p, &a)| if a < 0 { Some((p, a)) } else { None })
        .collect();
    let mut creditors: Vec<_> = balance
        .iter()
        .filter_map(|(p, &a)| if a > 0 { Some((p, a)) } else { None })
        .collect();

    if log_enabled!(Debug) {
        let sum: i64 = balance.iter().map(|(_, m)| m).sum();
        if sum != 0 {
            debug!("Total sum should be zero. In reality it is {sum}");
            debug!("{:?}", &debtors);
            debug!("{:?}", &creditors);
        }
    }

    let mut result = vec![];

    while !debtors.is_empty() && !creditors.is_empty() {
        let debtor = debtors.pop().unwrap();
        let creditor = creditors.pop().unwrap();
        if are_amount_equal(debtor.1, creditor.1) {
            result.push(MoneyExchange::new(debtor.0, creditor.0, creditor.1));
        } else if -debtor.1 < creditor.1 {
            let debt = -debtor.1;
            result.push(MoneyExchange::new(debtor.0, creditor.0, debt));
            creditors.push((creditor.0, creditor.1 - debt));
        } else {
            let debt = creditor.1;
            result.push(MoneyExchange::new(debtor.0, creditor.0, debt));
            debtors.push((debtor.0, debtor.1 + debt));
        }
    }

    if !creditors.is_empty() {
        warn!(
            "We run out of debtors but we still have creditors: {:?}",
            creditors
        );
    } else if !debtors.is_empty() {
        warn!(
            "We run out of creditors but we still have debtors: {:?}",
            debtors
        );
    }

    result
}

/// Some debts cannot be split exactly (there are no fraction of a cent),
/// so we tolerate one cent of error when comparing equality.
fn are_amount_equal(d: i64, c: i64) -> bool {
    (d + c).abs() < 1
}

fn compute_debts_and_credits(mut expenses: Vec<ExpenseWithId>) -> HashMap<String, i64> {
    let mut balance = HashMap::new();
    make_names_lowercase(&mut expenses);

    for expense in expenses {
        compute_debts(&expense, &mut balance);
        compute_credits(&expense, &mut balance);
    }

    balance
}

fn make_names_lowercase(expenses: &mut Vec<ExpenseWithId>) {
    for e in expenses {
        for p in &mut e.participants {
            p.name = p.name.to_lowercase();
        }
    }
}

fn compute_debts(expense: &ExpenseWithId, balance: &mut HashMap<String, i64>) {
    let mut total_amount = expense.amount;

    let fixed_debtors: Vec<_> = expense
        .participants
        .iter()
        .filter(|p| p.is_debtor() && p.amount.is_some())
        .collect();
    let fixed_debtor_names: HashSet<_> = fixed_debtors.iter().map(|p| &p.name).collect();

    // NOTE: we use HashSet instead of Vec because a participant may be present both as CREDITOR
    // and DEBTOR, but here we only want to count them once.
    let all_others: HashSet<_> = expense
        .participants
        .iter()
        .filter(|p| !fixed_debtor_names.contains(&p.name))
        .map(|p| &p.name)
        .collect();
    let all_others_len = all_others.len();

    for p in fixed_debtors {
        let amount = p.amount.unwrap();
        let entry = balance.entry(p.name.clone()).or_insert(0);
        *entry -= amount;
        total_amount -= amount;
    }
    let single_quota = (total_amount as f64 / all_others_len as f64).round() as i64;
    for p in all_others {
        let entry = balance.entry(p.clone()).or_insert(0);
        *entry -= single_quota;
    }
}

fn compute_credits(expense: &ExpenseWithId, balance: &mut HashMap<String, i64>) {
    let mut total_amount = expense.amount;

    let fixed_creditors: Vec<_> = expense
        .participants
        .iter()
        .filter(|p| p.is_creditor() && p.amount.is_some())
        .collect();
    let other_creditors: Vec<_> = expense
        .participants
        .iter()
        .filter(|p| p.is_creditor() && p.amount.is_none())
        .collect();
    let other_creditors_len = other_creditors.len();

    for p in fixed_creditors {
        let amount = p.amount.unwrap();
        let entry = balance.entry(p.name.clone()).or_insert(0);
        *entry += amount;
        total_amount -= amount;
    }
    let single_quota = (total_amount as f64 / other_creditors_len as f64).round() as i64;
    for p in other_creditors {
        let entry = balance.entry(p.name.clone()).or_insert(0);
        *entry += single_quota;
    }
}

#[cfg(test)]
mod tests {
    use crate::types::Participant;

    use super::*;

    #[test]
    fn test_compute_credits_and_debts() {
        let expenses = vec![
            ExpenseWithId::new(
                1,
                vec![
                    Participant::new_creditor("name1", None),
                    Participant::new_debtor("name2", None),
                    Participant::new_debtor("name3", Some(1040)),
                ],
                2340,
                None,
            ),
            // Also use some uppercase in names, to check that we turn them to lowercase:
            ExpenseWithId::new(
                2,
                vec![
                    Participant::new_creditor("NAme2", None),
                    Participant::new_debtor("NAME1", None),
                    Participant::new_debtor("naME3", None),
                ],
                3300,
                None,
            ),
        ];

        let balance = compute_debts_and_credits(expenses);

        assert_eq!(*balance.get("name1").unwrap(), 590);
        assert_eq!(*balance.get("name2").unwrap(), 1550);
        assert_eq!(*balance.get("name3").unwrap(), -2140);
    }

    #[test]
    fn test_compute_exchanges() {
        let expenses = vec![
            ExpenseWithId::new(
                1,
                vec![
                    Participant::new_creditor("name1", None),
                    Participant::new_debtor("name2", None),
                    Participant::new_debtor("name3", Some(1040)),
                ],
                2340,
                None,
            ),
            ExpenseWithId::new(
                2,
                vec![
                    Participant::new_creditor("name2", None),
                    Participant::new_debtor("name1", None),
                    Participant::new_debtor("name3", None),
                ],
                3300,
                None,
            ),
        ];

        let mut exchanges = compute_exchanges(expenses);
        assert_eq!(exchanges.len(), 2);

        exchanges.sort_by(|e1, e2| e1.amount.partial_cmp(&e2.amount).unwrap());

        assert_eq!(exchanges[0].debtor, "name3");
        assert_eq!(exchanges[0].creditor, "name1");
        assert_eq!(exchanges[0].amount, 590);

        assert_eq!(exchanges[1].debtor, "name3");
        assert_eq!(exchanges[1].creditor, "name2");
        assert_eq!(exchanges[1].amount, 1550);
    }
}
