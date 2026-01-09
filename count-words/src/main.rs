use anyhow::Context;
use std::env;
use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum WordCountError {
    #[error("Source contains no data")]
    EmptySource,
    // #[error("Read error")]
    // ReadError { source: std::io::Error },
    #[error(transparent)]
    IOError(#[from] std::io::Error),
}

fn count_words<R: Read>(input: &mut R) -> Result<u32, WordCountError> {
    let reader = BufReader::new(input);
    let mut word_count = 0;
    for line in reader.lines() {
        // let line = line.map_err(|source| WordCountError::ReadError { source })?;
        for _word in line?.split_whitespace() {
            word_count += 1;
        }
    }
    if word_count == 0 {
        Err(WordCountError::EmptySource)
    } else {
        Ok(word_count)
    }
}

fn main() -> anyhow::Result<()> {
    for filename in env::args().skip(1).collect::<Vec<String>>() {
        let mut reader = File::open(&filename).context(format!("unable to open '{filename}'"))?;
        let word_count =
            count_words(&mut reader).context(format!("unable to count words in '{filename}'"))?;
        println!("{word_count} {filename}");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{self, ErrorKind};

    pub struct ErrReader<'a> {
        pub kind: ErrorKind,
        pub msg: &'a str,
    }

    impl<'a> ErrReader<'a> {
        pub fn new(kind: ErrorKind, msg: &'a str) -> Self {
            Self { kind, msg }
        }
    }

    impl<'a> io::Read for ErrReader<'a> {
        fn read(&mut self, _buf: &mut [u8]) -> io::Result<usize> {
            Err(io::Error::new(self.kind, self.msg))
        }
    }

    #[test]
    fn read_broken_pipe() {
        let mut f = ErrReader::new(ErrorKind::BrokenPipe, "read: broken pipe");
        let _err = count_words(&mut f).unwrap_err();
    }
}
