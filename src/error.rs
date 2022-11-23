use teloxide::RequestError;
use thiserror::Error;

#[derive(Error)]
#[error("An error occurred: {user_message}")]
pub struct BotError {
    message: String,
    user_message: String,
}

#[derive(Error, Debug)]
pub enum InputError {
    #[error("invalid syntax for an expense; example of valid syntax: p1 p2/2.2 12 p1/1 p3 #group")]
    InvalidExpenseSyntax(String),

    #[error("invalid expense: {0}")]
    InvalidExpense(String, String),

    #[error(
        "invalid participant name `{0}`: participant names must be alphanumeric, can only \
             include ASCII characters and must start with a letter"
    )]
    InvalidParticipantName(String),

    #[error(
        "invalid group name `{0}`: group names must be alphanumeric, can only \
             include ASCII characters and must start with a letter. A `#` must be \
             prepended when using them in expenses, but not in other cases"
    )]
    InvalidGroupName(String),

    #[error("`{0}` is not a registered participant")]
    UnregisteredParticipant(String),

    #[error("`{0}` is not a registered group")]
    UnregisteredGroup(String),

    #[error(
        "there must be at least one participant. Format must be \
             'participant_name [participant_name...]'"
    )]
    ParticipantsNotProvided,

    #[error("missing group name. Format must be 'group_name [member_name...]'")]
    GroupNotProvided,

    #[error("custom amounts are not allowed for groups!")]
    GroupWithCustomAmount,

    #[error("invalid value `{0}` for limit: expected an integer")]
    InvalidLimit(String),

    #[error("invalid value `{0}` for expense ID: expected an integer")]
    InvalidExpenseId(String),
}

impl InputError {
    // TODO: it should be possible to improve nom error messages.
    pub fn invalid_expense_syntax(e: nom::Err<nom::error::Error<&str>>) -> Self {
        InputError::InvalidExpenseSyntax(e.to_string())
    }

    pub fn invalid_expense(reason: String, expense: String) -> Self {
        InputError::InvalidExpense(reason, expense)
    }

    pub fn invalid_participant_name(name: String) -> Self {
        InputError::InvalidParticipantName(name)
    }

    pub fn invalid_group_name(name: String) -> Self {
        InputError::InvalidGroupName(name)
    }

    pub fn unregistered_participant(name: String) -> Self {
        InputError::UnregisteredParticipant(name)
    }

    pub fn unregistered_group(name: String) -> Self {
        InputError::UnregisteredGroup(name)
    }

    pub fn participants_not_provided() -> Self {
        InputError::ParticipantsNotProvided
    }

    pub fn group_not_provided() -> Self {
        InputError::GroupNotProvided
    }

    pub fn group_with_custom_amount() -> Self {
        InputError::GroupWithCustomAmount
    }

    pub fn invalid_limit(limit: String) -> Self {
        InputError::InvalidLimit(limit)
    }

    pub fn invalid_expense_id(id: String) -> Self {
        InputError::InvalidExpenseId(id)
    }
}

impl BotError {
    pub fn database(message: &str, e: anyhow::Error) -> Self {
        let message = format!("{message}: {e}");
        let user_message = "cannot query the database, please try again later".to_string();
        BotError {
            message,
            user_message,
        }
    }

    pub fn telegram(message: &str, e: RequestError) -> Self {
        let message = format!("{message}: {e}");
        let user_message =
            "cannot communicate with Telegram server, please try again later".to_string();
        BotError {
            message,
            user_message,
        }
    }
}

impl std::fmt::Debug for BotError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}
