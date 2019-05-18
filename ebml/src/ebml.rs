#![macro_use]

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
    let end = lz as usize + 1;

    // concat the following bytes
    val = input[1..end].iter().fold(val, |acc, &b| {
        (acc << 8) | (b as u64)
    });

    Ok((&input[end..], val))
}

pub fn vid(input: &[u8]) -> IResult<&[u8], u64> {
    if input.is_empty() {
        return Err(nom::Err::Incomplete(Needed::Size(1)));
    }

    let v = input[0];
    let lz = v.leading_zeros();
    if lz == 8 || input.len() <= (lz as usize) {
        return Err(nom::Err::Incomplete(Needed::Size(1)));
    }

    let mut val = v as u64; // keep leading 1
    let end = lz as usize + 1;

    // concat the following bytes
    val = input[1..end].iter().fold(val, |acc, &b| {
        (acc << 8) | (b as u64)
    });

    Ok((&input[end..], val))
}

pub fn vsize(input: &[u8]) -> IResult<&[u8], usize> {
    let (rest, val) = vint(input)?;
    Ok((rest, val as usize))
}

pub fn uint(input: &[u8]) -> IResult<&[u8], u64> {
    let (input, size) = vsize(input)?;
    if input.len() < size {
        return Err(nom::Err::Incomplete(::nom::Needed::Size(size)));
    }
    let val = input[..size].iter().fold(0u64, |acc, &b| {
        (acc << 8) | (b as u64)
    });
    Ok((&input[size..], val))
}

pub fn bool(input: &[u8]) ->IResult<&[u8], bool> {
    let (i, val) = uint(input)?;
    Ok((i, val != 0))
}

pub fn float(input: &[u8]) -> IResult<&[u8], f64> {
    let (input, size) = vsize(input)?;

    if size == 4 {
        let (input, val) = nom::be_f32(input)?;
        Ok((input, val as f64))
    } else if size == 8 {
        let (input, val) = nom::be_f64(input)?;
        Ok((input, val))
    } else {
        Ok((input, 0f64))
    }
}

pub fn string(input: &[u8]) -> IResult<&[u8], String> {
    let (input, size) = vsize(input)?;
    if input.len() < size {
        return Err(nom::Err::Incomplete(::nom::Needed::Size(size)));
    }
    let r = nom::take!(input, size)?;
    Ok((r.0, String::from_utf8(r.1.to_vec()).unwrap()))
}

pub fn binary(input: &[u8]) -> IResult<&[u8], Vec<u8>> {
    let (input, size) = vsize(input)?;
    if input.len() < size {
        return Err(nom::Err::Incomplete(::nom::Needed::Size(size)));
    }
    let r = nom::take!(input, size)?;
    Ok((r.0, r.1.to_vec()))
}

pub fn skip(input: &[u8]) -> IResult<&[u8], usize> {
    let (input, size) = vsize(input)?;
    let r = nom::take!(input, size)?;
    Ok((r.0, size))
}

// parse an element from the mutable input and move the result to specified output
macro_rules! element {
    ($input: expr, $output: expr, $func: expr) => {{
        let _res = $func($input)?;
        $input = _res.0;
        $output = _res.1;
    }};
}

// skip the rest of an element after its ID field
macro_rules! skip {
    ($input: expr, $id: expr) => {{
        let _res = skip($input)?;
        $input = _res.0;
        eprintln!("Ignore element {:x} of {:x} bytes", $id, _res.1);
    }};
}

pub struct EBMLHeader {
    pub version: u64,
    pub read_version: u64,
    pub max_id_length: u64,
    pub max_size_length: u64,
    pub doc_type: String,
    pub doc_type_version: u64,
    pub doc_type_read_version: u64,
}

impl EBMLHeader {
    const ID: &'static [u8] = &[0x1Au8, 0x45, 0xDF, 0xA3];

    pub fn parse(input: &[u8]) -> IResult<&[u8], EBMLHeader> {
        let (input, size) = vsize(input)?;
        if input.len() < size {
            return Err(nom::Err::Incomplete(::nom::Needed::Size(size)));
        }

        let rest = &input[size..];
        let mut input = &input[0..size];

        let mut header = EBMLHeader {
            version: 1,
            read_version: 1,
            max_id_length: 4,
            max_size_length: 8,
            doc_type: "matroska".into(),
            doc_type_version: 1,
            doc_type_read_version: 1,
        };

        while !input.is_empty() {
            let id;
            element!(input, id, vid);
            match id {
                0x4286 => element!(input, header.version, uint),
                0x42F7 => element!(input, header.read_version, uint),
                0x42F2 => element!(input, header.max_id_length, uint),
                0x42F3 => element!(input, header.max_size_length, uint),
                0x4282 => element!(input, header.doc_type, string),
                0x4287 => element!(input, header.doc_type_version, uint),
                0x4285 => element!(input, header.doc_type_read_version, uint),
                _ => skip!(input, id),
            }
        }

        Ok((rest, header))
    }
}

pub struct EBMLSegment<'a> {
    pub content: &'a [u8],
}

impl<'a> EBMLSegment<'a> {
    const ID: &'static [u8] = &[0x18u8, 0x53, 0x80, 0x67];

    pub fn parse(input: &[u8]) -> IResult<&[u8], EBMLSegment> {
        let (input, _) = nom::take_until_and_consume!(input, EBMLSegment::ID)?;
        let (input, size) = vint(input)?;
        let (input, content) = nom::take!(input, size)?;

        Ok((input, EBMLSegment { content }))
    }
}

pub fn parse(input: &[u8]) -> IResult<&[u8], (EBMLHeader, EBMLSegment)> {
    let (input, _) = nom::take_until_and_consume!(input, EBMLHeader::ID)?;
    let (input, header) = EBMLHeader::parse(input)?;
    let (input, segment) = EBMLSegment::parse(input)?;
    Ok((input, (header, segment)))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SINGLE_STREAM: &'static [u8] = include_bytes!("../assets/single_stream.mkv");
    const WEBM: &'static [u8] = include_bytes!("../assets/big-buck-bunny_trailer.webm");

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

    #[test]
    fn test_vid() {
        let bytes = vec![0xecu8];
        let id = vid(&bytes).unwrap().1;
        assert_eq!(id, 0xec);

        let bytes = vec![0x42u8, 0x86];
        let id = vid(&bytes).unwrap().1;
        assert_eq!(id, 0x4286);

        let id = vid(&EBMLHeader::ID).unwrap().1;
        assert_eq!(id, 0x1a45dfa3);
    }

    #[test]
    fn test_ebml_header() {
        let res = parse(&WEBM[..100]);
        assert!(res.is_ok());
        let (_, (header, _)) = res.unwrap();
        assert_eq!(header.doc_type, "webm");

        let res = parse(&SINGLE_STREAM[..100]);
        assert!(res.is_ok());
        let (_, (header, _)) = res.unwrap();
        assert_eq!(header.doc_type, "matroska");
    }
}
