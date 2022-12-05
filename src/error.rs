use thiserror::Error;

#[derive(Error, Debug)]
pub enum InputError {
    #[error("invalid syntax for an expense; example of valid syntax: p1 p2/2.2 12 p1/1 p3 #group")]
    InvalidExpenseSyntax(String),

    #[error("invalid expense: {0}")]
    InvalidExpense(String, String),

    #[error(
        "invalid participant name `{0}`: participant names must start with a letter \
         and must be alphanumeric"
    )]
    InvalidParticipantName(String),

    #[error(
        "invalid alias `{0}`: aliases must start with a letter \
         and must be alphanumeric"
    )]
    InvalidAliasName(String),

    #[error(
        "invalid group name `{0}`: group names must start with a letter \
         and must be alphanumeric. A `#` must be \
         prepended when using them in expenses, but not in other cases"
    )]
    InvalidGroupName(String),

    #[error("`{0}` is not a registered participant")]
    UnregisteredParticipant(String),

    #[error("`{0}` is already used as the name of a participant")]
    AliasRegisteredAsParticipant(String),

    #[error("`{0}` is already used as an alias for participant `{1}`")]
    AliasRegisteredAsAlias(String, String),

    #[error("`{0}` is not an alias for participant `{1}`")]
    AliasNotRegisteredAsAlias(String, String),

    #[error("`{0}` is not a registered group")]
    UnregisteredGroup(String),

    #[error(
        "there must be at least one participant. Format must be \
             'participant_name [participant_name...]'"
    )]
    ParticipantsNotProvided,

    #[error("missing participant name. Format must be 'participant [alias1...]'")]
    ParticipantNotProvidedInAliasCommand,

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

    pub fn invalid_alias_name(name: String) -> Self {
        InputError::InvalidAliasName(name)
    }

    pub fn invalid_group_name(name: String) -> Self {
        InputError::InvalidGroupName(name)
    }

    pub fn unregistered_participant(name: String) -> Self {
        InputError::UnregisteredParticipant(name)
    }

    pub fn alias_registered_as_participant(name: String) -> Self {
        InputError::AliasRegisteredAsParticipant(name)
    }

    pub fn alias_registered_as_alias(name: String, participant: String) -> Self {
        InputError::AliasRegisteredAsAlias(name, participant)
    }

    pub fn alias_not_registered_as_alias(name: String, participant: String) -> Self {
        InputError::AliasNotRegisteredAsAlias(name, participant)
    }

    pub fn unregistered_group(name: String) -> Self {
        InputError::UnregisteredGroup(name)
    }

    pub fn participants_not_provided() -> Self {
        InputError::ParticipantsNotProvided
    }

    pub fn participant_not_provided_in_alias_command() -> Self {
        InputError::ParticipantNotProvidedInAliasCommand
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

#[derive(Error, Debug)]
#[error("Cannot query the database, please try again later.")]
pub enum DatabaseError {
    CommunicationError {
        message: String,
        source: anyhow::Error,
    },
    ConcurrencyError(String),
}

#[derive(Error, Debug)]
#[error("Cannot communicate with Telegram server, please try again later.")]
pub struct TelegramError {
    message: String,
    source: teloxide::RequestError,
}

impl DatabaseError {
    pub fn new<T: AsRef<str>>(message: T, e: anyhow::Error) -> Self {
        DatabaseError::CommunicationError {
            message: message.as_ref().to_string(),
            source: e,
        }
    }

    pub fn concurrency<T: AsRef<str>>(message: T) -> Self {
        DatabaseError::ConcurrencyError(message.as_ref().to_string())
    }

    pub fn is_concurrency_error(&self) -> bool {
        matches!(self, DatabaseError::ConcurrencyError(_))
    }
}

impl TelegramError {
    pub fn new<T: AsRef<str>>(message: T, e: teloxide::RequestError) -> Self {
        TelegramError {
            message: message.as_ref().to_string(),
            source: e,
        }
    }
}
