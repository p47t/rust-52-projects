use rand::distributions::{Distribution, Standard};
use rand::Rng;
use std::fmt::{Display, Formatter, Error};

#[derive(Debug)]
pub enum RollResult {
    One,
    Two,
    Three,
    Four,
    Five,
    Six,
}

impl Distribution<RollResult> for Standard {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> RollResult {
        match rng.gen_range(0, 6) {
            0 => RollResult::One,
            1 => RollResult::Two,
            2 => RollResult::Three,
            3 => RollResult::Four,
            4 => RollResult::Five,
            _ => RollResult::Six,
        }
    }
}

const CORNER: char = '+';
const HORIZ: char = '-';
const VERT: char = '|';
const PIP: char = '*';
const BLANK: char = ' ';

impl Display for RollResult {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        let pips = match self {
            RollResult::One => [
                [BLANK, BLANK, BLANK],
                [BLANK, (PIP), BLANK],
                [BLANK, BLANK, BLANK],
            ],
            RollResult::Two => [
                [BLANK, BLANK, (PIP)],
                [BLANK, BLANK, BLANK],
                [(PIP), BLANK, BLANK],
            ],
            RollResult::Three => [
                [BLANK, BLANK, (PIP)],
                [BLANK, (PIP), BLANK],
                [(PIP), BLANK, BLANK],
            ],
            RollResult::Four => [
                [(PIP), BLANK, (PIP)],
                [BLANK, BLANK, BLANK],
                [(PIP), BLANK, (PIP)],
            ],
            RollResult::Five => [
                [(PIP), BLANK, (PIP)],
                [BLANK, (PIP), BLANK],
                [(PIP), BLANK, (PIP)],
            ],
            RollResult::Six => [
                [(PIP), BLANK, (PIP)],
                [(PIP), BLANK, (PIP)],
                [(PIP), BLANK, (PIP)],
            ],
        };

        writeln!(f, "{}{}{}{}{}", CORNER, HORIZ, HORIZ, HORIZ, CORNER)?;
        for row in &pips {
            write!(f, "{}", VERT)?;
            for c in row {
                write!(f, "{}", c)?;
            }
            writeln!(f, "{}", VERT)?;
        }
        write!(f, "{}{}{}{}{}", CORNER, HORIZ, HORIZ, HORIZ, CORNER)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::RollResult;

    #[test]
    fn format_die_one() {
        let expected = "\
+---+
|   |
| * |
|   |
+---+";
        assert_eq!(format!("{}", RollResult::One), expected);
    }
}