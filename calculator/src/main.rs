use std::collections::HashMap;

enum Token {
    Name(String),
    Number(f64),
    Plus,
    Minus,
    Mul,
    Div,
    Print,
    Assign,
    LP,
    RP,
}

struct Calculator<TS> {
    token_stream: TS,
    symbols: HashMap<String, f64>,

    current_token: Option<Token>,
}

impl<TS> Calculator<TS> {
    fn new(token_stream: TS) -> Calculator<TS> {
        let mut symbols = HashMap::new();
        symbols.insert("pi".to_owned(), std::f64::consts::PI);
        symbols.insert("e".to_owned(), std::f64::consts::E);

        Calculator {
            token_stream,
            current_token: None,
            symbols,
        }
    }
}

impl<TS: Iterator<Item = Token>> Calculator<TS> {
    // program:
    //      end
    //      expr_list end
    //
    // expr_list:
    //      expression print
    //      expression print expr_list
    //
    fn calculate(&mut self) -> Vec<String> {
        let mut result = vec![];
        loop {
            match self.token_stream.next() {
                None => break,
                Some(Token::Print) => continue,
                token => {
                    let line = match self.expr(token) {
                        Ok(value) => value.to_string(),
                        Err(msg) => msg.to_string(),
                    };
                    result.push(line)
                }
            }
        }
        result
    }

    // expression:
    //      expression + term
    //      expression - term
    //      term
    //
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
    //      term * primary
    //      term / primary
    //      primary
    //
    fn term(&mut self, token: Option<Token>) -> Result<f64, String> {
        let mut left = self.prim(token)?;
        loop {
            match self.current_token {
                Some(Token::Mul) => {
                    left *= self.prim(None)?;
                }
                Some(Token::Div) => {
                    let p = self.prim(None)?;
                    left /= p;
                }
                _ => {
                    return Ok(left);
                }
            }
        }
    }

    // primary
    //      number
    //      name
    //      name = expression
    //      -primary
    //      (expression)
    //
    fn prim(&mut self, token: Option<Token>) -> Result<f64, String> {
        match token.or_else(|| self.token_stream.next()) {
            Some(Token::Name(name)) => {
                let mut value = self.symbols.get(&name).map_or(Default::default(), |v| *v);
                self.current_token = self.token_stream.next();
                if let Some(Token::Assign) = self.current_token {
                    value = self.expr(None)?;
                    self.symbols.insert(name, value);
                }
                Ok(value)
            }
            Some(Token::Number(value)) => {
                self.current_token = self.token_stream.next();
                Ok(value)
            }
            Some(Token::Minus) => Ok(-self.prim(None)?),
            Some(Token::LP) => {
                let e = self.expr(None)?;
                if let Some(Token::RP) = self.current_token {
                    self.current_token = self.token_stream.next();
                    Ok(e)
                } else {
                    Err("unmatched parenthesis".to_owned())
                }
            }
            _ => Err("primary expected".to_owned()),
        }
    }
}

struct TokenStream {
    input: Vec<char>,
    offset: usize,
}

impl TokenStream {
    fn new(input: &str) -> Self {
        TokenStream {
            input: input.chars().collect(),
            offset: 0,
        }
    }
}

impl Iterator for TokenStream {
    type Item = Token;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.offset >= self.input.len() {
                return None;
            }

            let begin = self.offset;
            let ch = self.input[self.offset];
            self.offset += 1;

            match ch {
                ';' => return Some(Token::Print),
                '*' => return Some(Token::Mul),
                '/' => return Some(Token::Div),
                '+' => return Some(Token::Plus),
                '-' => return Some(Token::Minus),
                '(' => return Some(Token::LP),
                ')' => return Some(Token::RP),
                '=' => return Some(Token::Assign),
                '0'..='9' | '.' => {
                    self.offset += self.input[self.offset..]
                        .iter()
                        .position(|c| !c.is_ascii_digit() && *c != '.')
                        .unwrap_or(0);
                    let number: String = self.input[begin..self.offset].iter().collect();
                    return if let Ok(number) = number.parse::<f64>() {
                        Some(Token::Number(number))
                    } else {
                        None
                    };
                }
                x if x.is_alphabetic() => {
                    self.offset += self.input[self.offset..]
                        .iter()
                        .position(|c| !c.is_alphabetic() && *c != '_')
                        .unwrap_or(0);
                    let name = self.input[begin..self.offset].iter().collect();
                    return Some(Token::Name(name));
                }
                x if x.is_whitespace() => continue,
                _ => return None,
            }
        }
    }
}

fn main() {
    for p in std::env::args().skip(1) {
        println!("Calculating {}", p);
        let mut calc = Calculator::new(TokenStream::new(&p));
        for line in calc.calculate() {
            println!("{}", line);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic() {
        let mut calc = Calculator::new(
            // x = 1; y = (x + 2*3/2 - 1); x + y
            vec![
                Token::Name("x".to_owned()),
                Token::Assign,
                Token::Number(1.0f64),
                Token::Print,
                Token::Name("y".to_owned()),
                Token::Assign,
                Token::LP,
                Token::Name("x".to_owned()),
                Token::Plus,
                Token::Number(2.0f64),
                Token::Mul,
                Token::Number(3.0f64),
                Token::Div,
                Token::Number(2.0f64),
                Token::Minus,
                Token::Number(1.0f64),
                Token::RP,
                Token::Print,
                Token::Name("x".to_owned()),
                Token::Plus,
                Token::Name("y".to_owned()),
            ]
            .into_iter(),
        );
        let lines = calc.calculate();
        assert_eq!(lines, vec!["1", "3", "4"]);
    }

    #[test]
    fn test_program_1() {
        let mut calc = Calculator::new(TokenStream::new(
            "x = 1; y = (x + 2*3/2 - 1); z = 0.5; x + y * z",
        ));
        let lines = calc.calculate();
        assert_eq!(lines, vec!["1", "3", "0.5", "2.5"]);
    }

    #[test]
    fn test_program_2() {
        let mut calc = Calculator::new(TokenStream::new("r = 10; a = pi * r * r"));
        let lines = calc.calculate();
        assert_eq!(lines, vec!["10", "314.1592653589793"]);
    }

    #[test]
    fn test_program_3() {
        let mut calc = Calculator::new(TokenStream::new("1/0"));
        let lines = calc.calculate();
        assert_eq!(lines, vec!["inf"]);
    }
}
