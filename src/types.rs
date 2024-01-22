//! The definition of data structures used in multiple modules.

/// A certain quantity of money.
///
/// The amount is an integer because we assume that all numbers have two decimal
/// points (the minimum unit is the *cent*) and we save them as if there was no
/// decimal part. This ensure maximum precision in our representation.
///
/// When computing the balance in [`crate::bot_logic::compute_exchanges`] we
/// still approximate, but saving with maximum precision leaves the possibility
/// to improve the balance precision later on if needed.
pub type Amount = i64;

/// An expense as created by the user.
#[derive(Clone, Debug)]
pub struct Expense {
    pub participants: Vec<Participant>,
    pub amount: Amount,
    pub message: Option<String>,
}

/// A debtor, a creditor and the amount of money that the debtor owes to the creditor.
#[derive(Clone, Debug)]
pub struct MoneyExchange {
    pub debtor: String,
    pub creditor: String,
    pub amount: Amount,
}

/// Same as [`Expense`], but with the `id`. This is used for expenses that are
/// read from memory.
#[derive(Clone, Debug)]
pub struct ExpenseWithId {
    pub id: i64,
    pub participants: Vec<Participant>,
    pub amount: Amount,
    pub message: Option<String>,
}

/// Whether a participant to an expense is a creditor or a debtor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParticipantMode {
    Creditor,
    Debtor,
}

/// A participant to an expense. The `amount` is an optional custom amount for the
/// participant. If the participant is a debtor, it corresponds to the money that the
/// participant owes to someone. If the participant is a creditor, it corresponds
/// to the amount of money that someone owes to the participant.
#[derive(Clone, Debug)]
pub struct Participant {
    pub name: String,
    pub mode: ParticipantMode,
    pub amount: Option<Amount>,
}

impl Expense {
    pub fn new(participants: Vec<Participant>, amount: Amount, message: Option<String>) -> Expense {
        Expense {
            participants,
            amount,
            message,
        }
    }
}

impl MoneyExchange {
    pub fn new(debtor: &str, creditor: &str, amount: Amount) -> MoneyExchange {
        MoneyExchange {
            debtor: debtor.to_string(),
            creditor: creditor.to_string(),
            amount,
        }
    }
}

impl ExpenseWithId {
    pub fn new(
        id: i64,
        participants: Vec<Participant>,
        amount: Amount,
        message: Option<String>,
    ) -> ExpenseWithId {
        ExpenseWithId {
            id,
            participants,
            amount,
            message,
        }
    }
}

impl Participant {
    pub fn new(name: String, mode: ParticipantMode, amount: Option<Amount>) -> Participant {
        Participant { name, mode, amount }
    }

    pub fn new_creditor(name: &str, amount: Option<Amount>) -> Participant {
        Participant::new(name.to_string(), ParticipantMode::Creditor, amount)
    }

    pub fn new_debtor(name: &str, amount: Option<Amount>) -> Participant {
        Participant::new(name.to_string(), ParticipantMode::Debtor, amount)
    }

    pub fn is_creditor(&self) -> bool {
        self.mode == ParticipantMode::Creditor
    }

    pub fn is_debtor(&self) -> bool {
        self.mode == ParticipantMode::Debtor
    }
}
