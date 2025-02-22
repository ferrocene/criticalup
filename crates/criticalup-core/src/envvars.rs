use std::env::VarError;

pub const CRITICALUP_TOKEN_ENV_VAR_NAME: &str = "CRITICALUP_TOKEN";

pub struct EnvVars {
    pub criticalup_token: Option<String>,
}

impl Default for EnvVars {
    fn default() -> Self {
        let criticalup_token = match std::env::var(CRITICALUP_TOKEN_ENV_VAR_NAME) {
            Ok(value) => {
                if !value.is_empty() {
                    Some(value)
                } else {
                    None
                }
            }
            Err(var_err) => {
                if let VarError::NotUnicode(_) = var_err {
                    tracing::error!(
                        "Environment variable {} is not Unicode.",
                        CRITICALUP_TOKEN_ENV_VAR_NAME
                    );
                }
                None
            }
        };

        EnvVars { criticalup_token }
    }
}
