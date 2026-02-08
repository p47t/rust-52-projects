use super::token::Token;

pub struct TokenStream {
    input: Vec<char>,
    offset: usize,
}

impl TokenStream {
    pub fn new(input: &str) -> Self {
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
                '%' => return Some(Token::Mod),
                '^' => return Some(Token::Pow),
                '+' => return Some(Token::Plus),
                '-' => return Some(Token::Minus),
                '(' => return Some(Token::LP),
                ')' => return Some(Token::RP),
                '=' => return Some(Token::Assign),
                '0'..='9' | '.' => {
                    while self.offset < self.input.len() {
                        let c = self.input[self.offset];
                        if c.is_ascii_digit() || c == '.' {
                            self.offset += 1;
                        } else {
                            break;
                        }
                    }
                    let number: String = self.input[begin..self.offset].iter().collect();
                    return if let Ok(number) = number.parse::<f64>() {
                        Some(Token::Number(number))
                    } else {
                        None
                    };
                }
                x if x.is_alphabetic() => {
                    while self.offset < self.input.len() {
                        let c = self.input[self.offset];
                        if c.is_alphabetic() || c == '_' {
                            self.offset += 1;
                        } else {
                            break;
                        }
                    }
                    let name = self.input[begin..self.offset].iter().collect();
                    return Some(Token::Name(name));
                }
                x if x.is_whitespace() => continue,
                _ => return None,
            }
        }
    }
}
