use nom::{IResult, Needed};

pub fn vint(input: &[u8]) -> IResult<&[u8], u64> {
    if input.is_empty() {
        return Err(nom::Err::Incomplete(Needed::Size(1)));
    }

    let v = input[0];
    let lz = v.leading_zeros();
    if lz == 8 || input.len() <= (lz as usize) {
        return Err(nom::Err::Incomplete(Needed::Size(1)));
    }

    // erase the leading 1
    let mut val = (v ^ (1 << (7 - lz))) as u64;

    // concat the following bytes
    for i in 0..lz as usize {
        val = (val << 8) | input[i + 1] as u64;
    }

    Ok((&input[lz as usize + 1..], val))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vint() {
        let bytes = vec![0x0u8];
        let i = vint(&bytes);
        assert!(i.is_err());

        let bytes = vec![0x40u8];
        let i = vint(&bytes);
        assert!(i.is_err());

        let bytes = vec![0x80u8];
        let i = vint(&bytes).unwrap().1;
        assert_eq!(i, 0u64);

        let bytes = vec![0x81u8];
        let i = vint(&bytes).unwrap().1;
        assert_eq!(i, 1u64);

        let bytes = vec![0xc0u8];
        let i = vint(&bytes).unwrap().1;
        assert_eq!(i, 0x40u64);

        let bytes = vec![0x40u8, 0x00];
        let i = vint(&bytes).unwrap().1;
        assert_eq!(i, 0u64);
    }
}
