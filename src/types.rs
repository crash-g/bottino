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
pub struct ParsedExpense {
    pub participants: Vec<ParsedParticipant>,
    pub amount: Amount,
    pub message: Option<String>,
}

/// A participant to an expense as defined by the user.
///
/// The `amount` is an optional custom amount for the
/// participant. If the participant is a debtor, it corresponds to the money that the
/// participant owes to someone. If the participant is a creditor, it corresponds
/// to the amount of money that someone owes to the participant.
#[derive(Clone, Debug)]
pub struct ParsedParticipant {
    pub name: String,
    mode: ParticipantMode,
    pub amount: Option<Amount>,
    is_group: bool,
}

/// An expense that is read from the database.
#[derive(Clone, Debug)]
pub struct SavedExpense {
    pub id: i64,
    pub participants: Vec<SavedParticipant>,
    pub amount: Amount,
    pub message: Option<String>,
}

/// A participant to an expense that is read from the database.
///
/// The `amount` is an optional custom amount for the
/// participant. If the participant is a debtor, it corresponds to the money that the
/// participant owes to someone. If the participant is a creditor, it corresponds
/// to the amount of money that someone owes to the participant.
#[derive(Clone, Debug)]
pub struct SavedParticipant {
    pub name: String,
    mode: ParticipantMode,
    pub amount: Option<Amount>,
}

/// A debtor, a creditor and the amount of money that the debtor owes to the creditor.
#[derive(Clone, Debug)]
pub struct MoneyExchange {
    pub debtor: String,
    pub creditor: String,
    pub amount: Amount,
}

/// Whether a participant to an expense is a creditor or a debtor.
#[derive(Clone, Debug, PartialEq, Eq)]
enum ParticipantMode {
    Creditor,
    Debtor,
}

impl ParsedExpense {
    pub fn new(
        participants: Vec<ParsedParticipant>,
        amount: Amount,
        message: Option<String>,
    ) -> ParsedExpense {
        ParsedExpense {
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

impl SavedExpense {
    pub fn new(
        id: i64,
        participants: Vec<SavedParticipant>,
        amount: Amount,
        message: Option<String>,
    ) -> SavedExpense {
        SavedExpense {
            id,
            participants,
            amount,
            message,
        }
    }
}

impl ParsedParticipant {
    fn new(name: String, mode: ParticipantMode, amount: Option<Amount>, is_group: bool) -> Self {
        Self {
            name,
            mode,
            amount,
            is_group,
        }
    }

    pub fn new_creditor(name: &str, amount: Option<Amount>) -> Self {
        Self::new(name.to_string(), ParticipantMode::Creditor, amount, false)
    }

    pub fn new_debtor(name: &str, amount: Option<Amount>) -> Self {
        Self::new(name.to_string(), ParticipantMode::Debtor, amount, false)
    }

    pub fn new_creditor_group(name: &str, amount: Option<Amount>) -> Self {
        Self::new(name.to_string(), ParticipantMode::Creditor, amount, true)
    }

    pub fn new_debtor_group(name: &str, amount: Option<Amount>) -> Self {
        Self::new(name.to_string(), ParticipantMode::Debtor, amount, true)
    }

    pub fn is_creditor(&self) -> bool {
        self.mode == ParticipantMode::Creditor
    }

    pub fn is_debtor(&self) -> bool {
        self.mode == ParticipantMode::Debtor
    }

    pub fn is_group(&self) -> bool {
        self.is_group
    }
}

impl SavedParticipant {
    fn new(name: String, mode: ParticipantMode, amount: Option<Amount>) -> Self {
        Self { name, mode, amount }
    }

    pub fn new_creditor(name: &str, amount: Option<Amount>) -> Self {
        Self::new(name.to_string(), ParticipantMode::Creditor, amount)
    }

    pub fn new_debtor(name: &str, amount: Option<Amount>) -> Self {
        Self::new(name.to_string(), ParticipantMode::Debtor, amount)
    }

    pub fn is_creditor(&self) -> bool {
        self.mode == ParticipantMode::Creditor
    }

    pub fn is_debtor(&self) -> bool {
        self.mode == ParticipantMode::Debtor
    }
}
