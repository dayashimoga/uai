//! Built-in safe mathematical expression evaluator (`system.calc`).

use aissistant_orchestrator::{OrchestratorError, ToolHandler};
use aissistant_security::RiskTier;
use serde_json::Value;

/// Tool safely evaluating arithmetic and algebraic mathematical expressions without code execution.
#[derive(Debug, Default, Clone, Copy)]
pub struct CalcSkill;

impl CalcSkill {
    /// Creates a new `CalcSkill` instance.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl ToolHandler for CalcSkill {
    fn name(&self) -> &str {
        "system.calc"
    }

    fn description(&self) -> &str {
        "Evaluates mathematical expressions safely (supports arithmetic, trigonometry, exponents, and constants)."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "expression": {
                    "type": "string",
                    "description": "Mathematical expression to evaluate (e.g. '(12.5 + 7.5) * 3 ^ 2 - sqrt(16)')."
                }
            },
            "required": ["expression"]
        })
    }

    fn risk_tier(&self) -> RiskTier {
        RiskTier::ReadOnly
    }

    fn execute(&self, arguments: Value) -> Result<Value, OrchestratorError> {
        let expression = arguments
            .get("expression")
            .and_then(Value::as_str)
            .ok_or_else(|| OrchestratorError::InvalidToolArguments {
                tool_name: "system.calc".to_string(),
                reason: "Missing 'expression' argument".to_string(),
            })?;

        let trimmed = expression.trim();
        if trimmed.is_empty() {
            return Err(OrchestratorError::InvalidToolArguments {
                tool_name: "system.calc".to_string(),
                reason: "Expression cannot be empty".to_string(),
            });
        }

        let result =
            evaluate_expression(trimmed).map_err(|err| OrchestratorError::ToolExecutionFailed {
                tool_name: "system.calc".to_string(),
                reason: format!("Evaluation error in '{trimmed}': {err}"),
            })?;

        // Format neatly: if whole number, format without redundant .0 for readability
        let formatted = if (result.fract()).abs() < 1e-9 {
            format!("{:.0}", result)
        } else {
            format!("{:.6}", result)
                .trim_end_matches('0')
                .trim_end_matches('.')
                .to_string()
        };

        Ok(serde_json::json!({
            "expression": trimmed,
            "result": result,
            "formatted": formatted
        }))
    }
}

// -----------------------------------------------------------------------------
// Recursive-Descent Safe Math Parser
// -----------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Number(f64),
    Ident(String),
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Caret,
    LParen,
    RParen,
    Comma,
}

struct Lexer<'a> {
    chars: std::iter::Peekable<std::str::Chars<'a>>,
}

impl<'a> Lexer<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            chars: input.chars().peekable(),
        }
    }

    fn tokenize(&mut self) -> Result<Vec<Token>, String> {
        let mut tokens = Vec::new();

        while let Some(&c) = self.chars.peek() {
            if c.is_whitespace() {
                self.chars.next();
                continue;
            }

            match c {
                '+' => {
                    self.chars.next();
                    tokens.push(Token::Plus);
                }
                '-' => {
                    self.chars.next();
                    tokens.push(Token::Minus);
                }
                '*' => {
                    self.chars.next();
                    tokens.push(Token::Star);
                }
                '/' => {
                    self.chars.next();
                    tokens.push(Token::Slash);
                }
                '%' => {
                    self.chars.next();
                    tokens.push(Token::Percent);
                }
                '^' => {
                    self.chars.next();
                    tokens.push(Token::Caret);
                }
                '(' => {
                    self.chars.next();
                    tokens.push(Token::LParen);
                }
                ')' => {
                    self.chars.next();
                    tokens.push(Token::RParen);
                }
                ',' => {
                    self.chars.next();
                    tokens.push(Token::Comma);
                }
                '0'..='9' | '.' => {
                    let num = self.read_number()?;
                    tokens.push(Token::Number(num));
                }
                'a'..='z' | 'A'..='Z' | '_' => {
                    let ident = self.read_ident();
                    tokens.push(Token::Ident(ident.to_ascii_lowercase()));
                }
                other => return Err(format!("Unexpected character: '{other}'")),
            }
        }

        Ok(tokens)
    }

    fn read_number(&mut self) -> Result<f64, String> {
        let mut num_str = String::new();
        let mut has_dot = false;

        while let Some(&c) = self.chars.peek() {
            if c.is_ascii_digit() {
                num_str.push(c);
                self.chars.next();
            } else if c == '.' && !has_dot {
                has_dot = true;
                num_str.push(c);
                self.chars.next();
            } else {
                break;
            }
        }

        num_str
            .parse::<f64>()
            .map_err(|e| format!("Invalid number literal '{num_str}': {e}"))
    }

    fn read_ident(&mut self) -> String {
        let mut ident = String::new();
        while let Some(&c) = self.chars.peek() {
            if c.is_ascii_alphanumeric() || c == '_' {
                ident.push(c);
                self.chars.next();
            } else {
                break;
            }
        }
        ident
    }
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn advance(&mut self) -> Option<Token> {
        if self.pos < self.tokens.len() {
            let tok = self.tokens[self.pos].clone();
            self.pos += 1;
            Some(tok)
        } else {
            None
        }
    }

    fn parse_expression(&mut self) -> Result<f64, String> {
        let mut left = self.parse_term()?;

        while let Some(tok) = self.peek() {
            match tok {
                Token::Plus => {
                    self.advance();
                    left += self.parse_term()?;
                }
                Token::Minus => {
                    self.advance();
                    left -= self.parse_term()?;
                }
                _ => break,
            }
        }

        Ok(left)
    }

    fn parse_term(&mut self) -> Result<f64, String> {
        let mut left = self.parse_power()?;

        while let Some(tok) = self.peek() {
            match tok {
                Token::Star => {
                    self.advance();
                    left *= self.parse_power()?;
                }
                Token::Slash => {
                    self.advance();
                    let right = self.parse_power()?;
                    if right.abs() < 1e-15 {
                        return Err("Division by zero".to_string());
                    }
                    left /= right;
                }
                Token::Percent => {
                    self.advance();
                    let right = self.parse_power()?;
                    if right.abs() < 1e-15 {
                        return Err("Modulo by zero".to_string());
                    }
                    left %= right;
                }
                _ => break,
            }
        }

        Ok(left)
    }

    fn parse_power(&mut self) -> Result<f64, String> {
        let left = self.parse_factor()?;

        if let Some(Token::Caret) = self.peek() {
            self.advance();
            let right = self.parse_power()?; // Right-associative exponentiation
            Ok(left.powf(right))
        } else {
            Ok(left)
        }
    }

    fn parse_factor(&mut self) -> Result<f64, String> {
        if let Some(tok) = self.peek() {
            match tok {
                Token::Plus => {
                    self.advance();
                    self.parse_factor()
                }
                Token::Minus => {
                    self.advance();
                    Ok(-self.parse_factor()?)
                }
                _ => self.parse_primary(),
            }
        } else {
            Err("Unexpected end of expression".to_string())
        }
    }

    fn parse_primary(&mut self) -> Result<f64, String> {
        match self.advance() {
            Some(Token::Number(n)) => Ok(n),
            Some(Token::LParen) => {
                let inner = self.parse_expression()?;
                match self.advance() {
                    Some(Token::RParen) => Ok(inner),
                    _ => Err("Missing closing parenthesis ')'".to_string()),
                }
            }
            Some(Token::Ident(name)) => {
                // Check if followed by '(' -> function call
                if let Some(Token::LParen) = self.peek() {
                    self.advance();
                    let mut args = Vec::new();

                    if let Some(Token::RParen) = self.peek() {
                        self.advance();
                    } else {
                        loop {
                            args.push(self.parse_expression()?);
                            match self.peek() {
                                Some(Token::Comma) => {
                                    self.advance();
                                }
                                Some(Token::RParen) => {
                                    self.advance();
                                    break;
                                }
                                _ => return Err("Expected ',' or ')' in argument list".to_string()),
                            }
                        }
                    }

                    evaluate_function(&name, &args)
                } else {
                    // Constant evaluation
                    evaluate_constant(&name)
                }
            }
            Some(other) => Err(format!("Unexpected token: {other:?}")),
            None => Err("Unexpected end of expression".to_string()),
        }
    }
}

fn evaluate_constant(name: &str) -> Result<f64, String> {
    match name {
        "pi" => Ok(std::f64::consts::PI),
        "e" => Ok(std::f64::consts::E),
        "tau" => Ok(std::f64::consts::TAU),
        other => Err(format!("Unknown identifier or constant: '{other}'")),
    }
}

fn evaluate_function(name: &str, args: &[f64]) -> Result<f64, String> {
    match name {
        "sqrt" => {
            if args.len() != 1 {
                return Err("sqrt() expects 1 argument".to_string());
            }
            if args[0] < 0.0 {
                return Err("Cannot compute square root of negative number".to_string());
            }
            Ok(args[0].sqrt())
        }
        "abs" => {
            if args.len() != 1 {
                return Err("abs() expects 1 argument".to_string());
            }
            Ok(args[0].abs())
        }
        "round" => {
            if args.len() != 1 {
                return Err("round() expects 1 argument".to_string());
            }
            Ok(args[0].round())
        }
        "floor" => {
            if args.len() != 1 {
                return Err("floor() expects 1 argument".to_string());
            }
            Ok(args[0].floor())
        }
        "ceil" => {
            if args.len() != 1 {
                return Err("ceil() expects 1 argument".to_string());
            }
            Ok(args[0].ceil())
        }
        "sin" => {
            if args.len() != 1 {
                return Err("sin() expects 1 argument".to_string());
            }
            Ok(args[0].sin())
        }
        "cos" => {
            if args.len() != 1 {
                return Err("cos() expects 1 argument".to_string());
            }
            Ok(args[0].cos())
        }
        "tan" => {
            if args.len() != 1 {
                return Err("tan() expects 1 argument".to_string());
            }
            Ok(args[0].tan())
        }
        "ln" | "log" => {
            if args.len() != 1 {
                return Err("ln() expects 1 argument".to_string());
            }
            if args[0] <= 0.0 {
                return Err("Cannot compute logarithm of non-positive number".to_string());
            }
            Ok(args[0].ln())
        }
        "log10" => {
            if args.len() != 1 {
                return Err("log10() expects 1 argument".to_string());
            }
            if args[0] <= 0.0 {
                return Err("Cannot compute logarithm of non-positive number".to_string());
            }
            Ok(args[0].log10())
        }
        "exp" => {
            if args.len() != 1 {
                return Err("exp() expects 1 argument".to_string());
            }
            Ok(args[0].exp())
        }
        "min" => {
            if args.len() != 2 {
                return Err("min() expects 2 arguments".to_string());
            }
            Ok(args[0].min(args[1]))
        }
        "max" => {
            if args.len() != 2 {
                return Err("max() expects 2 arguments".to_string());
            }
            Ok(args[0].max(args[1]))
        }
        "pow" => {
            if args.len() != 2 {
                return Err("pow() expects 2 arguments".to_string());
            }
            Ok(args[0].powf(args[1]))
        }
        other => Err(format!("Unknown math function: '{other}'")),
    }
}

/// Evaluates a mathematical string expression.
pub fn evaluate_expression(input: &str) -> Result<f64, String> {
    let mut lexer = Lexer::new(input);
    let tokens = lexer.tokenize()?;
    if tokens.is_empty() {
        return Err("Empty expression".to_string());
    }

    let mut parser = Parser::new(tokens);
    let result = parser.parse_expression()?;

    if parser.pos < parser.tokens.len() {
        return Err(format!(
            "Unexpected tokens remaining after expression: {:?}",
            &parser.tokens[parser.pos..]
        ));
    }

    Ok(result)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_arithmetic() {
        assert_eq!(evaluate_expression("2 + 2").unwrap(), 4.0);
        assert_eq!(evaluate_expression("10 - 3 * 2").unwrap(), 4.0);
        assert_eq!(evaluate_expression("(10 - 3) * 2").unwrap(), 14.0);
        assert_eq!(evaluate_expression("2 ^ 3").unwrap(), 8.0);
        assert_eq!(evaluate_expression("10 % 3").unwrap(), 1.0);
    }

    #[test]
    fn test_complex_expression() {
        // (12.5 + 7.5) * 3 ^ 2 - sqrt(16) = 20 * 9 - 4 = 176
        let res = evaluate_expression("(12.5 + 7.5) * 3 ^ 2 - sqrt(16)").unwrap();
        assert_eq!(res, 176.0);
    }

    #[test]
    fn test_constants_and_functions() {
        let pi_val = evaluate_expression("pi").unwrap();
        assert!((pi_val - std::f64::consts::PI).abs() < 1e-9);

        let sin_pi = evaluate_expression("sin(pi / 2)").unwrap();
        assert!((sin_pi - 1.0).abs() < 1e-9);

        let max_val = evaluate_expression("max(10, 20)").unwrap();
        assert_eq!(max_val, 20.0);
    }

    #[test]
    fn test_division_by_zero_rejection() {
        assert!(evaluate_expression("10 / 0").is_err());
    }

    #[test]
    fn test_calc_skill_tool_handler() {
        let skill = CalcSkill::new();
        assert_eq!(skill.name(), "system.calc");
        assert_eq!(skill.risk_tier(), RiskTier::ReadOnly);

        let res = skill
            .execute(serde_json::json!({ "expression": "100 / 4 + 5" }))
            .unwrap();
        assert_eq!(res["result"], 30.0);
        assert_eq!(res["formatted"], "30");
    }
}
