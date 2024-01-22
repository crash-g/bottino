//! The core of the bot logic. It contains the algorithm that computes
//! the money exchanges needed to settle debts.

use std::collections::{HashMap, HashSet};

use log::Level::Debug;
use log::{debug, log_enabled, warn};

use crate::types::{MoneyExchange, SavedExpense};

/// Get a list of money exchanges which settle debts computed from the list
/// of expenses in input.
///
/// The algorithm works as follows:
/// - process all expenses to get a list of people who owe money (debtors) and
///   a list of people who must receive money (creditors)
/// - pick a debtor and a creditor
/// - compare debtor's debt (*d*) and creditor's credit (*c*):
///     * if bigger: let the debtor give *c* to creditor, then pick a new creditor
///     * if smaller: let the debtor give *d* to creditor, then pick a new debtor
///     * if equal: let the debtor give *d* to creditor, then pick a new debtor and
///       and a new creditor
/// - stop when there are no more debtors/creditors
///
/// The solution is correct but not necessarily optimal, in the sense that it may
/// require more money exchanges than needed. However, the optimal solution is
/// NP-complete and this approximation is normally good enough.
///
/// Another approximation is that we use floating-point math: this may cause rounding
/// errors, but in general we tolerate errors up to one cent.
pub fn compute_exchanges(expenses: Vec<SavedExpense>) -> Vec<MoneyExchange> {
    let debts_and_credits = compute_debts_and_credits(expenses);
    let mut debtors: Vec<_> = debts_and_credits
        .iter()
        .filter_map(|(p, &a)| if a < 0 { Some((p, a)) } else { None })
        .collect();
    let mut creditors: Vec<_> = debts_and_credits
        .iter()
        .filter_map(|(p, &a)| if a > 0 { Some((p, a)) } else { None })
        .collect();

    if log_enabled!(Debug) {
        let sum: i64 = debts_and_credits.iter().map(|(_, m)| m).sum();
        if sum != 0 {
            debug!("Total sum should be zero. In reality it is {sum}");
            debug!("{:?}", &debtors);
            debug!("{:?}", &creditors);
        }
    }

    let mut result = vec![];

    while !debtors.is_empty() && !creditors.is_empty() {
        let debtor = debtors.pop().expect("just checked debtors are non-empty!");
        let creditor = creditors
            .pop()
            .expect("just checked creditors are non-empty!");
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

fn compute_debts_and_credits(mut expenses: Vec<SavedExpense>) -> HashMap<String, i64> {
    let mut balance = HashMap::new();
    make_names_lowercase(&mut expenses);

    for expense in expenses {
        compute_debts(&expense, &mut balance);
        compute_credits(&expense, &mut balance);
    }

    balance
}

/// We save participants using the original string that users gave in input.
/// However, participant names are case-insensitive, so when computing
/// the actual balance we turn them into lower case.
fn make_names_lowercase(expenses: &mut Vec<SavedExpense>) {
    for e in expenses {
        for p in &mut e.participants {
            p.name = p.name.to_lowercase();
        }
    }
}

fn compute_debts(expense: &SavedExpense, balance: &mut HashMap<String, i64>) {
    let mut total_amount = expense.amount;

    let fixed_debtors: Vec<_> = expense
        .participants
        .iter()
        .filter(|p| p.is_debtor() && p.amount.is_some())
        .collect();
    let fixed_debtor_names: HashSet<_> = fixed_debtors.iter().map(|p| &p.name).collect();

    // All creditors are automatically debtors too (unless they are also registered as debtors
    // with a custom amount of zero).
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
        let amount = p.amount.expect("fixed debtors must have a custom amount!");
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

fn compute_credits(expense: &SavedExpense, balance: &mut HashMap<String, i64>) {
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
        let amount = p
            .amount
            .expect("fixed creditors must have a custom amount!");
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
    use crate::types::SavedParticipant;

    use super::*;

    #[test]
    fn test_compute_credits_and_debts() {
        let expenses = vec![
            SavedExpense::new(
                1,
                vec![
                    SavedParticipant::new_creditor("name1", None),
                    SavedParticipant::new_debtor("name2", None),
                    SavedParticipant::new_debtor("name3", Some(1040)),
                ],
                2340,
                None,
            ),
            // Also use some uppercase in names, to check that we turn them to lowercase:
            SavedExpense::new(
                2,
                vec![
                    SavedParticipant::new_creditor("NAme2", None),
                    SavedParticipant::new_debtor("NAME1", None),
                    SavedParticipant::new_debtor("naME3", None),
                ],
                3300,
                None,
            ),
        ];

        let balance = compute_debts_and_credits(expenses);

        assert_eq!(*balance.get("name1").expect("test"), 590);
        assert_eq!(*balance.get("name2").expect("test"), 1550);
        assert_eq!(*balance.get("name3").expect("test"), -2140);
    }

    #[test]
    fn test_compute_exchanges() {
        let expenses = vec![
            SavedExpense::new(
                1,
                vec![
                    SavedParticipant::new_creditor("name1", None),
                    SavedParticipant::new_debtor("name2", None),
                    SavedParticipant::new_debtor("name3", Some(1040)),
                ],
                2340,
                None,
            ),
            SavedExpense::new(
                2,
                vec![
                    SavedParticipant::new_creditor("name2", None),
                    SavedParticipant::new_debtor("name1", None),
                    SavedParticipant::new_debtor("name3", None),
                ],
                3300,
                None,
            ),
        ];

        let mut exchanges = compute_exchanges(expenses);
        assert_eq!(exchanges.len(), 2);

        exchanges.sort_by(|e1, e2| e1.amount.partial_cmp(&e2.amount).expect("test"));

        assert_eq!(exchanges[0].debtor, "name3");
        assert_eq!(exchanges[0].creditor, "name1");
        assert_eq!(exchanges[0].amount, 590);

        assert_eq!(exchanges[1].debtor, "name3");
        assert_eq!(exchanges[1].creditor, "name2");
        assert_eq!(exchanges[1].amount, 1550);
    }
}
