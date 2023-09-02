//! The core of the bot logic. It contains the algorithm that computes
//! the money exchanges needed to settle debts.

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

use log::Level::Debug;
use log::{debug, log_enabled, warn};

use crate::types::{MoneyExchange, SavedExpense};

/// Get a list of money exchanges which settle debts computed from the list
/// of expenses in input. The output is sorted by debtors first and creditors
/// second.
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
/// errors, but in general we tolerate errors up to one cent. In the future it may
/// be better to use fixed-precision numbers, since the integral part is what
/// we care about and we don't need a lot of precision in the decimal part.
pub fn compute_exchanges(expenses: Vec<SavedExpense>) -> Vec<MoneyExchange> {
    let debts_and_credits = compute_debts_and_credits(expenses);
    let mut debtors: Vec<_> = debts_and_credits
        .iter()
        .filter_map(|(p, &a)| if a < 0.0 { Some((p, a)) } else { None })
        .collect();
    let mut creditors: Vec<_> = debts_and_credits
        .iter()
        .filter_map(|(p, &a)| if a > 0.0 { Some((p, a)) } else { None })
        .collect();

    // Sort debtors and creditors to ensure consistent results. The order is reversed cause
    // then we use `pop`, so we iterate the vectors in reverse order.
    debtors.sort_by(|x, y| reverse_ordering(x.0.partial_cmp(y.0).expect("Cannot sort debtors")));
    creditors.sort_by(|x, y| reverse_ordering(x.0.partial_cmp(y.0).expect("Cannot sort creditors")));

    if log_enabled!(Debug) {
        let sum: i64 = debts_and_credits
            .iter()
            .map(|(_, m)| m.round() as i64)
            .sum();
        if sum > 1 || sum < -1 {
            debug!(
                "Total sum should be 0 (or 1 or -1 in some corner cases). In reality it is {sum}"
            );
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
            result.push(MoneyExchange::new(
                debtor.0,
                creditor.0,
                creditor.1.round() as i64,
            ));
        } else if -debtor.1 < creditor.1 {
            let debt = -debtor.1;
            result.push(MoneyExchange::new(
                debtor.0,
                creditor.0,
                debt.round() as i64,
            ));
            creditors.push((creditor.0, creditor.1 - debt));
        } else {
            let debt = creditor.1;
            result.push(MoneyExchange::new(
                debtor.0,
                creditor.0,
                debt.round() as i64,
            ));
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

fn reverse_ordering(o: Ordering) -> Ordering {
    use Ordering::*;
    match o {
        Greater => Less,
        Equal => Equal,
        Less => Greater,
    }
}

/// Some debts cannot be split exactly (there are no fractions of a cent),
/// so we tolerate one cent of error when comparing equality.
fn are_amount_equal(d: f64, c: f64) -> bool {
    (d + c).abs() < 1.0
}

/// In a previous version this was a map of integers, but this meant that each
/// expense potentially introduced a one cent error and in unfortunate scenarios
/// these errors summed up. So now we wait until the last second (i.e. when outputting
/// a money exchange) before converting back to integer.
fn compute_debts_and_credits(expenses: Vec<SavedExpense>) -> HashMap<String, f64> {
    let mut balance = HashMap::new();

    for expense in expenses {
        compute_debts(&expense, &mut balance);
        compute_credits(&expense, &mut balance);
    }

    balance
}

fn compute_debts(expense: &SavedExpense, balance: &mut HashMap<String, f64>) {
    let mut total_amount = expense.amount as f64;

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
        let amount = p.amount.expect("fixed debtors must have a custom amount!") as f64;
        let entry = balance.entry(p.name.clone()).or_insert(0.0);
        *entry -= amount;
        total_amount -= amount;
    }

    let single_quota = total_amount / all_others_len as f64;
    for p in all_others {
        let entry = balance.entry(p.clone()).or_insert(0.0);
        *entry -= single_quota;
    }
}

fn compute_credits(expense: &SavedExpense, balance: &mut HashMap<String, f64>) {
    let mut total_amount = expense.amount as f64;

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
            .expect("fixed creditors must have a custom amount!") as f64;
        let entry = balance.entry(p.name.clone()).or_insert(0.0);
        *entry += amount;
        total_amount -= amount;
    }
    let single_quota = total_amount / other_creditors_len as f64;
    for p in other_creditors {
        let entry = balance.entry(p.name.clone()).or_insert(0.0);
        *entry += single_quota;
    }
}

#[cfg(test)]
mod tests {
    use approx::assert_abs_diff_eq;
    use chrono::{DateTime, Utc};

    use crate::types::SavedParticipant;

    use super::*;

    fn make_expenses() -> Vec<SavedExpense> {
        vec![
            SavedExpense::new(
                1,
                true,
                vec![
                    SavedParticipant::new_creditor("p2", None),
                    SavedParticipant::new_debtor("p1", None),
                    SavedParticipant::new_debtor("a3", Some(1040)),
                    SavedParticipant::new_debtor("à3", Some(200)),
                ],
                2340,
                None,
                DateTime::<Utc>::MIN_UTC,
            ),
            SavedExpense::new(
                2,
                true,
                vec![
                    SavedParticipant::new_creditor("ã2", None),
                    SavedParticipant::new_debtor("à3", None),
                    SavedParticipant::new_debtor("a3", None),
                ],
                3300,
                None,
                DateTime::<Utc>::MIN_UTC,
            ),
            SavedExpense::new(
                3,
                true,
                vec![
                    SavedParticipant::new_creditor("p4", None),
                    SavedParticipant::new_debtor("a3", None),
                ],
                2000,
                None,
                DateTime::<Utc>::MIN_UTC,
            ),
        ]
    }

    #[test]
    fn test_compute_credits_and_debts() {
        let expenses = make_expenses();
        let balance = compute_debts_and_credits(expenses);

        assert_eq!(balance.len(), 6);
        assert_abs_diff_eq!(*balance.get("a3").expect("test"), -3140.0);
        assert_abs_diff_eq!(*balance.get("à3").expect("test"), -1300.0);
        assert_abs_diff_eq!(*balance.get("p1").expect("test"), -550.0);
        assert_abs_diff_eq!(*balance.get("ã2").expect("test"), 2200.0);
        assert_abs_diff_eq!(*balance.get("p2").expect("test"), 1790.0);
        assert_abs_diff_eq!(*balance.get("p4").expect("test"), 1000.0);
    }

    #[test]
    fn test_compute_exchanges() {
        let expenses = make_expenses();
        let exchanges = compute_exchanges(expenses);
        assert_eq!(exchanges.len(), 5);

        assert_eq!(exchanges[0].debtor, "a3");
        assert_eq!(exchanges[0].creditor, "p2");
        assert_eq!(exchanges[0].amount, 1790);

        assert_eq!(exchanges[1].debtor, "a3");
        assert_eq!(exchanges[1].creditor, "p4");
        assert_eq!(exchanges[1].amount, 1000);

        assert_eq!(exchanges[2].debtor, "a3");
        assert_eq!(exchanges[2].creditor, "ã2");
        assert_eq!(exchanges[2].amount, 350);

        assert_eq!(exchanges[3].debtor, "p1");
        assert_eq!(exchanges[3].creditor, "ã2");
        assert_eq!(exchanges[3].amount, 550);

        assert_eq!(exchanges[4].debtor, "à3");
        assert_eq!(exchanges[4].creditor, "ã2");
        assert_eq!(exchanges[4].amount, 1300);
    }
}
