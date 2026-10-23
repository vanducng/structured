use std::process::ExitCode;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("not signed in")]
    Auth,
    #[error("{0}")]
    Api(String),
    #[error("{message}")]
    Usage { message: String, steps: Vec<String> },
    #[error("{message}")]
    NotFound { message: String, steps: Vec<String> },
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
    pub fn usage(message: impl Into<String>) -> Self {
        Self::Usage {
            message: message.into(),
            steps: Vec::new(),
        }
    }

    pub fn usage_with(message: impl Into<String>, steps: &[&str]) -> Self {
        Self::Usage {
            message: message.into(),
            steps: steps.iter().map(|s| (*s).to_owned()).collect(),
        }
    }

    pub fn not_found(message: impl Into<String>, steps: Vec<String>) -> Self {
        Self::NotFound {
            message: message.into(),
            steps,
        }
    }

    pub fn code(&self) -> &'static str {
        match self {
            Self::Auth => "AUTH",
            Self::Api(_) => "API",
            Self::Usage { .. } | Self::Help => "USAGE",
            Self::NotFound { .. } => "NOT_FOUND",
            Self::Network(_) => "NETWORK",
            Self::Io(_) => "IO",
            Self::Json(_) => "DATA",
        }
    }

    pub fn retryable(&self) -> bool {
        matches!(self, Self::Network(_))
    }

    pub fn next_steps(&self) -> Vec<String> {
        let steps: &[&str] = match self {
            Self::Usage { steps, .. } | Self::NotFound { steps, .. } if !steps.is_empty() => {
                return steps.clone();
            }
            Self::Auth => {
                &["ask the person to run `structured login` in their own terminal, then retry"]
            }
            Self::Network(_) => &["retry the command once"],
            Self::NotFound { .. } => &[
                "structured today --json",
                "structured list --from YYYY-MM-DD --json",
            ],
            _ => &["structured <command> --help"],
        };
        steps.iter().map(|s| (*s).to_owned()).collect()
    }

    pub fn exit_code(&self) -> ExitCode {
        match self {
            Self::Usage { .. } | Self::Help => ExitCode::from(2),
            _ => ExitCode::from(1),
        }
    }

    pub fn quiet(&self) -> bool {
        matches!(self, Self::Help)
    }
}

pub type Result<T> = std::result::Result<T, Error>;
