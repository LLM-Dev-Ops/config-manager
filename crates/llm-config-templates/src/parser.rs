//! Template parsing functionality

use crate::{Result, TemplateError};

/// Token in a parsed template
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token {
    /// Literal text
    Literal(String),
    /// Variable placeholder
    Variable(String),
}

/// Parse a template string into tokens
pub fn parse(template: &str) -> Result<Vec<Token>> {
    let mut tokens = Vec::new();
    let mut current_literal = String::new();
    let mut chars = template.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '{' && chars.peek() == Some(&'{') {
            // Start of variable
            chars.next(); // Consume second '{'

            // Save any accumulated literal
            if !current_literal.is_empty() {
                tokens.push(Token::Literal(current_literal.clone()));
                current_literal.clear();
            }

            // Extract variable name
            let mut var_name = String::new();
            let mut found_close = false;

            while let Some(ch) = chars.next() {
                if ch == '}' && chars.peek() == Some(&'}') {
                    chars.next(); // Consume second '}'
                    found_close = true;
                    break;
                } else if ch.is_whitespace() || ch.is_alphanumeric() || ch == '_' || ch == '-' {
                    var_name.push(ch);
                } else {
                    return Err(TemplateError::ParseError(format!(
                        "Invalid character in variable name: {}",
                        ch
                    )));
                }
            }

            if !found_close {
                return Err(TemplateError::ParseError(
                    "Unclosed variable placeholder".to_string(),
                ));
            }

            let var_name = var_name.trim().to_string();
            if var_name.is_empty() {
                return Err(TemplateError::ParseError("Empty variable name".to_string()));
            }

            tokens.push(Token::Variable(var_name));
        } else {
            // Regular character
            current_literal.push(ch);
        }
    }

    // Save final literal
    if !current_literal.is_empty() {
        tokens.push(Token::Literal(current_literal));
    }

    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_literal_only() {
        let tokens = parse("Hello, World!").unwrap();
        assert_eq!(tokens, vec![Token::Literal("Hello, World!".to_string())]);
    }

    #[test]
    fn test_parse_variable_only() {
        let tokens = parse("{{name}}").unwrap();
        assert_eq!(tokens, vec![Token::Variable("name".to_string())]);
    }

    #[test]
    fn test_parse_mixed() {
        let tokens = parse("Hello, {{name}}! Your score is {{score}}.").unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::Literal("Hello, ".to_string()),
                Token::Variable("name".to_string()),
                Token::Literal("! Your score is ".to_string()),
                Token::Variable("score".to_string()),
                Token::Literal(".".to_string()),
            ]
        );
    }

    #[test]
    fn test_parse_multiple_variables() {
        let tokens = parse("{{var1}} and {{var2}}").unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::Variable("var1".to_string()),
                Token::Literal(" and ".to_string()),
                Token::Variable("var2".to_string()),
            ]
        );
    }

    #[test]
    fn test_parse_variable_with_underscores() {
        let tokens = parse("{{my_variable_name}}").unwrap();
        assert_eq!(
            tokens,
            vec![Token::Variable("my_variable_name".to_string())]
        );
    }

    #[test]
    fn test_parse_variable_with_dashes() {
        let tokens = parse("{{my-variable-name}}").unwrap();
        assert_eq!(
            tokens,
            vec![Token::Variable("my-variable-name".to_string())]
        );
    }

    #[test]
    fn test_parse_variable_with_whitespace() {
        let tokens = parse("{{ name }}").unwrap();
        assert_eq!(tokens, vec![Token::Variable("name".to_string())]);
    }

    #[test]
    fn test_parse_unclosed_variable() {
        let result = parse("{{name");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_empty_variable() {
        let result = parse("{{}}");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_single_brace() {
        let tokens = parse("This { is } a test").unwrap();
        assert_eq!(
            tokens,
            vec![Token::Literal("This { is } a test".to_string())]
        );
    }
}
