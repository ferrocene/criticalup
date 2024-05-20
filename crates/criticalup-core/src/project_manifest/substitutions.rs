use crate::errors::ProjectManifestLoadingError;

const VARIABLE_START: &str = "${";
const VARIABLE_END: &str = "}";

enum ParseState {
    Raw,
    Variable,
}

pub(super) fn apply_substitutions(mut input: &str) -> Result<String, ProjectManifestLoadingError> {
    let mut state = ParseState::Raw;
    let mut result = String::new();

    loop {
        match state {
            ParseState::Raw => {
                if let Some(start) = input.find(VARIABLE_START) {
                    result.push_str(&input[..start]);

                    input = &input[(start + VARIABLE_START.len())..];
                    state = ParseState::Variable;
                } else {
                    // End of the input
                    result.push_str(input);
                    return Ok(result);
                }
            }
            ParseState::Variable => {
                if let Some(end) = input.find(VARIABLE_END) {
                    result.push_str(&apply_substitution(&input[..end])?);

                    input = &input[(end + VARIABLE_END.len())..];
                    state = ParseState::Raw;
                } else {
                    // End of the input
                    return Err(ProjectManifestLoadingError::UnterminatedVariableInSubstitution);
                }
            }
        }
    }
}

fn apply_substitution(variable: &str) -> Result<String, ProjectManifestLoadingError> {
    match variable {
        "rustc-host" => Ok(env!("TARGET").into()),
        other => Err(ProjectManifestLoadingError::UnknownVariableInSubstitution(
            other.into(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apply_substitutions() {
        assert_eq!("hello world", apply_substitutions("hello world").unwrap());
        assert_eq!(
            env!("TARGET"),
            apply_substitutions("${rustc-host}").unwrap()
        );
        assert_eq!(
            concat!("hello ", env!("TARGET")),
            apply_substitutions("hello ${rustc-host}").unwrap()
        );
        assert_eq!(
            concat!("hello ", env!("TARGET"), "!"),
            apply_substitutions("hello ${rustc-host}!").unwrap()
        );
        assert_eq!(
            concat!("hello ", env!("TARGET"), "}"),
            apply_substitutions("hello ${rustc-host}}").unwrap()
        );

        assert!(matches!(
            apply_substitutions("hello ${").unwrap_err(),
            ProjectManifestLoadingError::UnterminatedVariableInSubstitution
        ));
        assert!(matches!(
            apply_substitutions("hello ${missing-var}!").unwrap_err(),
            ProjectManifestLoadingError::UnknownVariableInSubstitution(s) if s == "missing-var"
        ));
        assert!(matches!(
            apply_substitutions("hello ${}!").unwrap_err(),
            ProjectManifestLoadingError::UnknownVariableInSubstitution(s) if s.is_empty()
        ));
    }

    #[test]
    fn test_apply_substitution() {
        assert_eq!(env!("TARGET"), apply_substitution("rustc-host").unwrap());

        assert!(matches!(
            apply_substitution("rustc_host").unwrap_err(),
            ProjectManifestLoadingError::UnknownVariableInSubstitution(s) if s == "rustc_host"
        ));
        assert!(matches!(
            apply_substitution("").unwrap_err(),
            ProjectManifestLoadingError::UnknownVariableInSubstitution(s) if s.is_empty()
        ));
    }
}
