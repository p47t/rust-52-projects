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
        return Err(nom::Err::Incomplete(::nom::Needed::Size(size)))
    }
    let val = input[..size].iter().fold(0u64, |acc, &b| {
        (acc << 8) | (b as u64)
    });
    Ok((&input[size..], val))
}

pub fn string(input: &[u8]) -> IResult<&[u8], String> {
    let (input, size) = vsize(input)?;
    if input.len() < size {
        return Err(nom::Err::Incomplete(::nom::Needed::Size(size)))
    }
    let r = nom::take!(input, size)?;
    Ok((r.0, String::from_utf8(r.1.to_vec()).unwrap()))
}

pub fn skip_element(input: &[u8]) -> IResult<&[u8], usize> {
    let (input, size) = vsize(input)?;
    let r = nom::take!(input, size)?;
    Ok((r.0, size))
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

pub fn ebml_header(input: &[u8]) -> IResult<&[u8], EBMLHeader> {
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
        let (i, id) = vid(input)?;
        input = i;
        match id {
            0x4286 => {
                let (i, val) = uint(input)?;
                input = i;
                header.version = val;
            },
            0x42F7 => {
                let (i, val) = uint(input)?;
                input = i;
                header.read_version = val;
            }
            0x42F2 => {
                let (i, val) = uint(input)?;
                input = i;
                header.max_id_length = val;
            }
            0x42F3 => {
                let (i, val) = uint(input)?;
                input = i;
                header.max_size_length = val;
            }
            0x4282 => {
                let (i, val) = string(input)?;
                input = i;
                header.doc_type = val;
            }
            0x4287 => {
                let (i, val) = uint(input)?;
                input = i;
                header.doc_type_version = val;
            }
            0x4285 => {
                let (i, val) = uint(input)?;
                input = i;
                header.doc_type_read_version = val;
            }
            _ => {
                let (i, size) = skip_element(input)?;
                input = i;
                eprintln!("Ignore element {:x} of {:x} bytes", id, size);
            }
        }
    }

    Ok((rest, header))
}

pub struct EBMLSegment {
    pub size: u64,
}

impl EBMLSegment {
    pub fn next_element<'a>(&self, input: &'a [u8]) -> IResult<&'a [u8], SegmentElement> {
        ebml_segment_element(input)
    }
}

pub fn ebml_segment(input: &[u8]) -> IResult<&[u8], EBMLSegment> {
    let mut segment = EBMLSegment {
        size: 0
    };

    const SEGMENT_ID: [u8; 4] = [0x18u8, 0x53, 0x80, 0x67];
    let (input, _) = nom::take_until_and_consume!(input, &SEGMENT_ID[..])?;
    let (input, size) = vint(input)?;
    segment.size = size;

    Ok((input, segment))
}

pub enum SegmentElement {
    SeekHead,
    Info,
    Tracks,
    Chapters,
    Cluster,
    Cues,
    Attachments,
    Tags,
    Void(u64),
    Unknown(u64),
}

pub struct SeekHead {
    pub positions: Vec<Seek>,
}

pub struct Seek {
    pub id: Vec<u8>,
    pub position: u64,
}

pub fn seek_head(input: &[u8]) -> IResult<&[u8], SegmentElement> {
    let (input, size) = vint(input)?;
    let (input, _) = nom::take!(input, size)?;
    Ok((input, SegmentElement::SeekHead))
}

pub fn info(input: &[u8]) -> IResult<&[u8], SegmentElement> {
    let (input, size) = vint(input)?;
    let (input, _) = nom::take!(input, size)?;
    Ok((input, SegmentElement::Info))
}

pub fn cluster(input: &[u8]) -> IResult<&[u8], SegmentElement> {
    let (input, size) = vint(input)?;
    let (input, _) = nom::take!(input, size)?;
    Ok((input, SegmentElement::Cluster))
}

pub fn chapters(input: &[u8]) -> IResult<&[u8], SegmentElement> {
    let (input, size) = vint(input)?;
    let (input, _) = nom::take!(input, size)?;
    Ok((input, SegmentElement::Chapters))
}

pub fn tags(input: &[u8]) -> IResult<&[u8], SegmentElement> {
    let (input, size) = vint(input)?;
    let (input, _) = nom::take!(input, size)?;
    Ok((input, SegmentElement::Tags))
}

pub fn attachments(input: &[u8]) -> IResult<&[u8], SegmentElement> {
    let (input, size) = vint(input)?;
    let (input, _) = nom::take!(input, size)?;
    Ok((input, SegmentElement::Attachments))
}

pub fn tracks(input: &[u8]) -> IResult<&[u8], SegmentElement> {
    let (input, size) = vint(input)?;
    let (input, _) = nom::take!(input, size)?;
    Ok((input, SegmentElement::Tracks))
}

pub fn cues(input: &[u8]) -> IResult<&[u8], SegmentElement> {
    let (input, size) = vint(input)?;
    let (input, _) = nom::take!(input, size)?;
    Ok((input, SegmentElement::Cues))
}

pub fn ebml_segment_element(input: &[u8]) -> IResult<&[u8], SegmentElement> {
    let (input, id) = vid(input)?;
    match id {
        0x114D9B74 => seek_head(input),
        0x1549A966 => info(input),
        0x1F43B675 => cluster(input),
        0x1043A770 => chapters(input),
        0x1254C367 => tags(input),
        0x1941A469 => attachments(input),
        0x1654AE6B => tracks(input),
        0x1C53BB6B => cues(input),
        0xEC => {
            let (input, size) = skip_element(input)?;
            Ok((input, SegmentElement::Void(size as u64)))
        }
        _ => {
            let (input, size) = skip_element(input)?;
            Ok((input, SegmentElement::Unknown(size as u64)))
        }
    }
}

const EBML_ID: [u8; 4] = [0x1Au8, 0x45, 0xDF, 0xA3];

pub fn ebml_file(input: &[u8]) -> IResult<&[u8], (EBMLHeader, EBMLSegment)> {
    let (input, _) = nom::take_until_and_consume!(input, &EBML_ID[..])?;
    let (input, header) = ebml_header(input)?;
    let (input, segment) = ebml_segment(input)?;
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

        let id = vid(&EBML_ID).unwrap().1;
        assert_eq!(id, 0x1a45dfa3);
    }

    #[test]
    fn test_ebml_header() {
        let res = ebml_file(&WEBM[..100]);
        assert!(res.is_ok());
        let (_, (header, _)) = res.unwrap();
        assert_eq!(header.doc_type, "webm");

        let res = ebml_file(&SINGLE_STREAM[..100]);
        assert!(res.is_ok());
        let (_, (header, _)) = res.unwrap();
        assert_eq!(header.doc_type, "matroska");
    }

    #[test]
    fn test_webm_segment() {
        let res = ebml_file(&WEBM[..]);
        assert!(res.is_ok());
        let (input, (_, segment)) = res.unwrap();

        let res = segment.next_element(input);
        assert!(res.is_ok());
        let (input, element) = res.unwrap();
        match element {
            SegmentElement::SeekHead => (),
            _ => panic!()
        }

        let res = segment.next_element(input);
        assert!(res.is_ok());
        let (input, element) = res.unwrap();
        match element {
            SegmentElement::Info => (),
            _ => panic!()
        }

        let res = segment.next_element(input);
        assert!(res.is_ok());
        let (input, element) = res.unwrap();
        match element {
            SegmentElement::Tracks => (),
            _ => panic!()
        }

        let res = segment.next_element(input);
        assert!(res.is_ok());
        let (input, element) = res.unwrap();
        match element {
            SegmentElement::Cues => (),
            _ => panic!()
        }

        let res = segment.next_element(input);
        assert!(res.is_ok());
        let (input, element) = res.unwrap();
        match element {
            SegmentElement::Cluster => (),
            _ => panic!()
        }
    }
}
