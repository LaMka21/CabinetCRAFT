use cc_i18n::tr;

/// Chyba vykonania príkazu. Projekt po nej ostáva v stave pred príkazom.
///
/// Text chyby (`Display`) je v zvolenom jazyku (`engine.errors.*` v `locales/*.toml`).
#[derive(Debug, Clone, PartialEq)]
pub enum EngineError {
    UnknownCommand(String),
    Disabled { command: String, reason: String },
    Params { command: String, message: String },
    Failed(String),
    Panicked { command: String, message: String },
}

impl std::fmt::Display for EngineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = match self {
            EngineError::UnknownCommand(command) => {
                tr!("engine.errors.unknown_command", command = command)
            }
            EngineError::Disabled { command, reason } => {
                tr!("engine.errors.disabled", command = command, reason = reason)
            }
            EngineError::Params { command, message } => {
                tr!("engine.errors.params", command = command, message = message)
            }
            EngineError::Failed(message) => return f.write_str(message),
            EngineError::Panicked { command, message } => {
                tr!(
                    "engine.errors.panicked",
                    command = command,
                    message = message
                )
            }
        };
        f.write_str(&text)
    }
}

impl std::error::Error for EngineError {}

impl EngineError {
    /// Krátke označenie druhu chyby pre riadiaci kanál.
    pub fn kind(&self) -> &'static str {
        match self {
            EngineError::UnknownCommand(_) => "unknown-command",
            EngineError::Disabled { .. } => "disabled",
            EngineError::Params { .. } => "params",
            EngineError::Failed(_) => "failed",
            EngineError::Panicked { .. } => "panicked",
        }
    }
}

impl From<String> for EngineError {
    fn from(message: String) -> Self {
        EngineError::Failed(message)
    }
}

impl From<&str> for EngineError {
    fn from(message: &str) -> Self {
        EngineError::Failed(message.to_owned())
    }
}
