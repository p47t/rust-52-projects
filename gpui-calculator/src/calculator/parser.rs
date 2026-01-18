use std::collections::HashMap;

use super::lexer::TokenStream;
use super::token::Token;

pub struct Calculator {
    tokens: Vec<Token>,
    pos: usize,
    current_token: Option<Token>,
    pub symbols: HashMap<String, f64>,
}

impl Calculator {
    pub fn new() -> Self {
        let mut symbols = HashMap::new();
        symbols.insert("pi".to_owned(), std::f64::consts::PI);
        symbols.insert("e".to_owned(), std::f64::consts::E);

        Calculator {
            tokens: Vec::new(),
            pos: 0,
            current_token: None,
            symbols,
        }
    }

    /// Evaluate a single expression, returning the result or an error message.
    /// Variables are persisted across calls.
    pub fn evaluate(&mut self, input: &str) -> Result<f64, String> {
        self.tokens = TokenStream::new(input).collect();
        self.pos = 0;
        self.current_token = None;

        if self.tokens.is_empty() {
            return Err("empty expression".to_owned());
        }

        let first_token = self.next_token();
        self.expr(first_token)
    }

    fn next_token(&mut self) -> Option<Token> {
        if self.pos < self.tokens.len() {
            let token = self.tokens[self.pos].clone();
            self.pos += 1;
            Some(token)
        } else {
            None
        }
    }

    // expression:
    //      expression + term
    //      expression - term
    //      term
    fn expr(&mut self, token: Option<Token>) -> Result<f64, String> {
        let mut left = self.term(token)?;
        loop {
            match self.current_token {
                Some(Token::Plus) => {
                    left += self.term(None)?;
                }
                Some(Token::Minus) => {
                    left -= self.term(None)?;
                }
                _ => {
                    return Ok(left);
                }
            }
        }
    }

    // term:
    //      term * power
    //      term / power
    //      term % power
    //      power
    fn term(&mut self, token: Option<Token>) -> Result<f64, String> {
        let mut left = self.power(token)?;
        loop {
            match self.current_token {
                Some(Token::Mul) => {
                    left *= self.power(None)?;
                }
                Some(Token::Div) => {
                    let p = self.power(None)?;
                    left /= p;
                }
                Some(Token::Mod) => {
                    let p = self.power(None)?;
                    left %= p;
                }
                _ => {
                    return Ok(left);
                }
            }
        }
    }

    // power:
    //      primary ^ power  (right associative)
    //      primary
    fn power(&mut self, token: Option<Token>) -> Result<f64, String> {
        let base = self.prim(token)?;
        if let Some(Token::Pow) = self.current_token {
            let exp = self.power(None)?; // right associative
            Ok(base.powf(exp))
        } else {
            Ok(base)
        }
    }

    // primary:
    //      number
    //      name
    //      name = expression
    //      -primary
    //      (expression)
    fn prim(&mut self, token: Option<Token>) -> Result<f64, String> {
        let tok = token.or_else(|| self.next_token());
        match tok {
            Some(Token::Name(name)) => {
                let mut value = self.symbols.get(&name).copied().unwrap_or_default();
                self.current_token = self.next_token();
                if let Some(Token::Assign) = self.current_token {
                    value = self.expr(None)?;
                    self.symbols.insert(name, value);
                }
                Ok(value)
            }
            Some(Token::Number(value)) => {
                self.current_token = self.next_token();
                Ok(value)
            }
            Some(Token::Minus) => Ok(-self.prim(None)?),
            Some(Token::LP) => {
                let e = self.expr(None)?;
                if let Some(Token::RP) = self.current_token {
                    self.current_token = self.next_token();
                    Ok(e)
                } else {
                    Err("unmatched parenthesis".to_owned())
                }
            }
            Some(Token::Print) => {
                // Handle semicolon - just continue to next expression
                let next = self.next_token();
                self.prim(next)
            }
            _ => Err("primary expected".to_owned()),
        }
    }
}

impl Default for Calculator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_arithmetic() {
        let mut calc = Calculator::new();
        assert_eq!(calc.evaluate("2 + 3").unwrap(), 5.0);
        assert_eq!(calc.evaluate("10 - 4").unwrap(), 6.0);
        assert_eq!(calc.evaluate("3 * 4").unwrap(), 12.0);
        assert_eq!(calc.evaluate("15 / 3").unwrap(), 5.0);
    }

    #[test]
    fn test_operator_precedence() {
        let mut calc = Calculator::new();
        assert_eq!(calc.evaluate("2 + 3 * 4").unwrap(), 14.0);
        assert_eq!(calc.evaluate("(2 + 3) * 4").unwrap(), 20.0);
    }

    #[test]
    fn test_variables() {
        let mut calc = Calculator::new();
        assert_eq!(calc.evaluate("x = 5").unwrap(), 5.0);
        assert_eq!(calc.evaluate("x * 2").unwrap(), 10.0);
        assert_eq!(calc.evaluate("y = x + 3").unwrap(), 8.0);
        assert_eq!(calc.evaluate("y").unwrap(), 8.0);
    }

    #[test]
    fn test_constants() {
        let mut calc = Calculator::new();
        let pi_result = calc.evaluate("pi").unwrap();
        assert!((pi_result - std::f64::consts::PI).abs() < 1e-10);

        let e_result = calc.evaluate("e").unwrap();
        assert!((e_result - std::f64::consts::E).abs() < 1e-10);
    }

    #[test]
    fn test_unary_minus() {
        let mut calc = Calculator::new();
        assert_eq!(calc.evaluate("-5").unwrap(), -5.0);
        assert_eq!(calc.evaluate("3 + -2").unwrap(), 1.0);
    }

    #[test]
    fn test_division_by_zero() {
        let mut calc = Calculator::new();
        let result = calc.evaluate("1/0").unwrap();
        assert!(result.is_infinite());
    }

    #[test]
    fn test_exponentiation() {
        let mut calc = Calculator::new();
        assert_eq!(calc.evaluate("2^3").unwrap(), 8.0);
        assert_eq!(calc.evaluate("2^3^2").unwrap(), 512.0); // right associative: 2^(3^2) = 2^9
        assert_eq!(calc.evaluate("2 + 3^2").unwrap(), 11.0); // precedence: 2 + 9
        assert_eq!(calc.evaluate("2 * 3^2").unwrap(), 18.0); // precedence: 2 * 9
    }

    #[test]
    fn test_modulo() {
        let mut calc = Calculator::new();
        assert_eq!(calc.evaluate("10 % 3").unwrap(), 1.0);
        assert_eq!(calc.evaluate("17 % 5").unwrap(), 2.0);
    }
}
