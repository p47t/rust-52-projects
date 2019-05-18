use nom::IResult;
use crate::ebml::{vid, vint, skip, binary, uint};

pub enum SegmentElement {
    SeekHead(SeekHead),
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
    let (input, mut data) = nom::take!(input, size)?;

    let mut seek_head = SeekHead { positions: vec![] };
    while !data.is_empty() {
        let id;
        element!(data, id, vid);
        match id {
            0x4DBB => {
                let seek;
                element!(data, seek, seek_element);
                seek_head.positions.push(seek);
            }
            _ => skip!(data, id),
        }
    }

    Ok((input, SegmentElement::SeekHead(seek_head)))
}

pub fn seek_element(input: &[u8]) -> IResult<&[u8], Seek> {
    let (input, size) = vint(input)?;
    let (input, mut data) = nom::take!(input, size)?;

    let mut seek = Seek {
        id: vec![],
        position: 0,
    };
    while !data.is_empty() {
        let id;
        element!(data, id, vid);
        match id {
            0x53AB => element!(data, seek.id, binary),
            0x53AC => element!(data, seek.position, uint),
            _ => skip!(data, id),
        }
    }

    Ok((input, seek))
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

pub fn mkv_level1_element(input: &[u8]) -> IResult<&[u8], SegmentElement> {
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
            let (input, size) = skip(input)?;
            Ok((input, SegmentElement::Void(size as u64)))
        }
        _ => {
            let (input, size) = skip(input)?;
            Ok((input, SegmentElement::Unknown(size as u64)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ebml::ebml_file;

    const WEBM: &'static [u8] = include_bytes!("../assets/big-buck-bunny_trailer.webm");

    #[test]
    fn test_webm_segment() {
        let res = ebml_file(&WEBM[..]);
        assert!(res.is_ok());
        let (input, (_, _)) = res.unwrap();

        let res = mkv_level1_element(input);
        assert!(res.is_ok());
        let (input, element) = res.unwrap();
        match element {
            SegmentElement::SeekHead(_) => (),
            _ => panic!()
        }

        let res = mkv_level1_element(input);
        assert!(res.is_ok());
        let (input, element) = res.unwrap();
        match element {
            SegmentElement::Info => (),
            _ => panic!()
        }

        let res = mkv_level1_element(input);
        assert!(res.is_ok());
        let (input, element) = res.unwrap();
        match element {
            SegmentElement::Tracks => (),
            _ => panic!()
        }

        let res = mkv_level1_element(input);
        assert!(res.is_ok());
        let (input, element) = res.unwrap();
        match element {
            SegmentElement::Cues => (),
            _ => panic!()
        }

        let res = mkv_level1_element(input);
        assert!(res.is_ok());
        let (_input, element) = res.unwrap();
        match element {
            SegmentElement::Cluster => (),
            _ => panic!()
        }
    }
}