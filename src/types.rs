// #[derive(Clone, Debug)]
// struct NoteMessage {
//     participants: Vec<Participant>,
//     amount: f64,
//     message: String,
// }

pub type Amount = i64;

#[derive(Clone, Debug)]
pub struct Expense {
    pub participants: Vec<Participant>,
    pub amount: Amount,
    pub message: Option<String>,
}

#[derive(Clone, Debug)]
pub struct MoneyExchange {
    pub debtor: String,
    pub creditor: String,
    pub amount: Amount,
}

#[derive(Clone, Debug)]
pub struct ExpenseWithId {
    pub id: i64,
    pub participants: Vec<Participant>,
    pub amount: Amount,
    pub message: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParticipantMode {
    Creditor,
    Debtor,
}

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
