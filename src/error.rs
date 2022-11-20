use teloxide::RequestError;
use thiserror::Error;

#[derive(Error)]
#[error("An error occurred: {user_message}")]
pub struct BotError {
    message: String,
    user_message: String,
}

impl BotError {
    pub fn new(message: String, user_message: String) -> Self {
        BotError {
            message,
            user_message,
        }
    }

    // TODO: it should be possible to improve nom error messages.
    pub fn nom_parse(message: &str, e: nom::Err<nom::error::Error<&str>>) -> Self {
        let message = format!("{message}: {e}");
        let user_message = "cannot parse message".to_string();
        BotError {
            message,
            user_message,
        }
    }

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
