#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Name(String),
    Number(f64),
    Plus,
    Minus,
    Mul,
    Div,
    Mod,
    Pow,
    Print,
    Assign,
    LP,
    RP,
}
