use std::process::ExitCode;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("not signed in")]
    Auth,
    #[error("{0}")]
    Api(String),
    #[error("{0}")]
    Usage(String),
    #[error("a command is required")]
    Help,
    #[error("network: {0}")]
    Network(#[from] reqwest::Error),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid response: {0}")]
    Json(#[from] serde_json::Error),
}

impl Error {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Auth => "AUTH",
            Self::Api(_) => "API",
            Self::Usage(_) | Self::Help => "USAGE",
            Self::Network(_) => "NETWORK",
            Self::Io(_) => "IO",
            Self::Json(_) => "DATA",
        }
    }

    pub fn retryable(&self) -> bool {
        matches!(self, Self::Network(_))
    }

    pub fn next_steps(&self) -> Vec<&'static str> {
        match self {
            Self::Auth => vec!["structured login"],
            Self::Network(_) => vec!["retry the command"],
            _ => vec!["structured --help"],
        }
    }

    pub fn exit_code(&self) -> ExitCode {
        match self {
            Self::Usage(_) | Self::Help => ExitCode::from(2),
            _ => ExitCode::from(1),
        }
    }

    pub fn quiet(&self) -> bool {
        matches!(self, Self::Help)
    }
}

pub type Result<T> = std::result::Result<T, Error>;
