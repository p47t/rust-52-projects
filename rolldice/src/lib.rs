use std::fmt::{Display, Error, Formatter};

use rand::distributions::{Distribution, Standard};
use rand::Rng;

#[derive(Debug, PartialEq)]
pub enum RollResult {
    One,
    Two,
    Three,
    Four,
    Five,
    Six,
}

impl RollResult {
    fn from(i: u32) -> RollResult {
        match i {
            1 => RollResult::One,
            2 => RollResult::Two,
            3 => RollResult::Three,
            4 => RollResult::Four,
            5 => RollResult::Five,
            _ => RollResult::Six,
        }
    }
}

impl Distribution<RollResult> for Standard {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> RollResult {
        RollResult::from(rng.gen_range(1, 7))
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

pub struct MultiZip<I> {
    iters: Vec<I>,
}

pub fn multizip<I: Iterator>(iters: Vec<I>) -> MultiZip<I> {
    MultiZip { iters }
}

impl<I: Iterator> Iterator for MultiZip<I> {
    type Item = Vec<I::Item>;

    fn next(&mut self) -> Option<Self::Item> {
        self.iters.iter_mut().map(|iter| iter.next()).collect()
    }
}

#[cfg(test)]
mod tests {
    use crate::{multizip, RollResult};

    #[test]
    fn dice_from_number() {
        assert_eq!(RollResult::from(1), RollResult::One);
        assert_eq!(RollResult::from(2), RollResult::Two);
        assert_eq!(RollResult::from(3), RollResult::Three);
        assert_eq!(RollResult::from(4), RollResult::Four);
        assert_eq!(RollResult::from(5), RollResult::Five);
        assert_eq!(RollResult::from(6), RollResult::Six);
    }

    #[test]
    fn format_die_one() {
        let expected = "\
+---+
|   |
| * |
|   |
+---+";
        assert_eq!(format!("{}", RollResult::One), expected);
        let expected = "\
+---+
|  *|
|   |
|*  |
+---+";
        assert_eq!(format!("{}", RollResult::Two), expected);
        let expected = "\
+---+
|  *|
| * |
|*  |
+---+";
        assert_eq!(format!("{}", RollResult::Three), expected);
        let expected = "\
+---+
|* *|
|   |
|* *|
+---+";
        assert_eq!(format!("{}", RollResult::Four), expected);
        let expected = "\
+---+
|* *|
| * |
|* *|
+---+";
        assert_eq!(format!("{}", RollResult::Five), expected);
        let expected = "\
+---+
|* *|
|* *|
|* *|
+---+";
        assert_eq!(format!("{}", RollResult::Six), expected);
    }

    #[test]
    fn test_multizip() {
        let vecs = vec![
            vec![1, 2, 3],
            vec![4, 5, 6],
            vec![7, 8, 9],
        ];
        let iters = vecs.iter().map(|v| v.iter()).collect();
        let mut zipped = multizip(iters).into_iter();
        let line = zipped.next().unwrap();
        assert_eq!(line.as_slice(), &[&1, &4, &7]);
        let line = zipped.next().unwrap();
        assert_eq!(line.as_slice(), &[&2, &5, &8]);
        let line = zipped.next().unwrap();
        assert_eq!(line.as_slice(), &[&3, &6, &9]);
    }
}