use nom::{IResult, Needed};

#[derive(Debug)]
pub enum ElementData {
    Signed(i64),
    Unsigned(u64),
    Float(f64),
    PlainString(String),
    UTF8String(String),
    Date(u64),
    Master(Vec<Element>),
    Binary(Vec<u8>),
    Unknown(u64),
}

#[derive(Debug)]
pub struct Element {
    id: u64,
    size: u64,
    data: ElementData,
}

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
    let end = lz as usize + 1;

    // concat the following bytes
    val = input[1..end].iter().fold(val, |acc, &b| {
        (acc << 8) | (b as u64)
    });

    Ok((&input[end..], val))
}

pub fn uint(input: &[u8], size: u64) -> IResult<&[u8], ElementData> {
    let val = input[..size as usize].iter().fold(0u64, |acc, &b| {
        (acc << 8) | (b as u64)
    });
    Ok((input, ElementData::Unsigned(val)))
}

pub fn element(input: &[u8]) -> IResult<&[u8], Element> {
    let (input, id) = vint(input)?;
    let (input, size) = vint(input)?;
    match id {
        0x4286 => {
            let (input, data) = uint(input, size)?;
            Ok((input, Element{id, size, data}))
        },
        _ => Ok((input, Element{id, size, data: ElementData::Unknown(id)})),
    }
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

        let bytes = vec![0x40u8, 0x00];
        let i = vint(&bytes).unwrap().1;
        assert_eq!(i, 0u64);

        let bytes = vec![0x4fu8, 0x88];
        let i = vint(&bytes).unwrap().1;
        assert_eq!(i, 0xf88u64);

        let bytes = vec![0x21u8, 0x32, 0x23];
        let i = vint(&bytes).unwrap().1;
        assert_eq!(i, 0x13223u64);

        let bytes = vec![0x11u8, 0x23, 0x45, 0x67];
        let i = vint(&bytes).unwrap().1;
        assert_eq!(i, 0x1234567);

        let bytes = vec![0x09u8, 0x23, 0x45, 0x67, 0x89];
        let i = vint(&bytes).unwrap().1;
        assert_eq!(i, 0x123456789);

        let bytes = vec![0x05u8, 0x23, 0x45, 0x67, 0x89, 0xab];
        let i = vint(&bytes).unwrap().1;
        assert_eq!(i, 0x123456789ab);
    }
}
