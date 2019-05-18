use nom::IResult;
use crate::ebml::{vid, vint, skip, binary, float, uint, string};

pub enum Level1Element {
    SeekHead(SeekHead),
    Info(Info),
    Tracks(Tracks),
    Chapters,
    Cluster,
    Cues,
    Attachments,
    Tags,
    Void(u64),
    Unknown(u64),
}

#[derive(Default)]
pub struct SeekHead {
    pub positions: Vec<Seek>,
}

#[derive(Default)]
pub struct Seek {
    pub id: Vec<u8>,
    pub position: u64,
}

#[derive(Default)]
pub struct Info {
    pub uid: Vec<u8>,
    pub filename: String,
    pub prev_uid: Vec<u8>,
    pub prev_filename: String,
    pub next_uid: Vec<u8>,
    pub next_filename: String,
    pub timecode_scale: u64,
    pub duration: f64,
    pub title: String,
    pub date_utc: u64,
    pub muxing_app: String,
    pub writing_app: String,
}

#[derive(Default)]
pub struct Tracks {
    pub tracks: Vec<Track>
}

#[derive(Default)]
pub struct Track {
    pub number: u64,
    pub uid: Vec<u8>,
    pub track_type: u64,
    pub enabled: bool,
    pub default: bool,
    pub forced: bool,
    pub lacing: bool,
    pub min_cache: u64,
    pub max_cache: u64,
    pub default_duration: u64,
    pub track_timecode_scale: f64,
    pub name: String,
    pub language: String,
    pub codec_id: String,
    pub codec_private: Vec<u8>,
    pub codec_name: String,
    pub attachment_link: u64,
    pub video: Video,
    pub audio: Audio,
    pub content_encodings: ContentEncodings,
}

#[derive(Default)]
pub struct Video {
    pub pixel_width: u64,
    pub pixel_height: u64,
    pub pixel_crop_bottom: u64,
    pub pixel_crop_top: u64,
    pub pixel_crop_left: u64,
    pub pixel_crop_right: u64,
    pub display_width: u64,
    pub display_height: u64,
    pub display_uint: u64,
}

#[derive(Default)]
pub struct Audio {
    pub sampling_frequency: u64,
    pub output_sampling_frequency: u64,
    pub channels: u64,
    pub bit_depth: u64,
}

#[derive(Default)]
pub struct ContentEncodings {}

pub fn seek_head(input: &[u8]) -> IResult<&[u8], Level1Element> {
    let (input, size) = vint(input)?;
    let (input, mut data) = nom::take!(input, size)?;

    let mut seek_head = SeekHead::default();
    while !data.is_empty() {
        let id;
        element!(data, id, vid);
        match id {
            0x4DBB => {
                let val;
                element!(data, val, seek);
                seek_head.positions.push(val);
            }
            _ => skip!(data, id),
        }
    }

    Ok((input, Level1Element::SeekHead(seek_head)))
}

pub fn seek(input: &[u8]) -> IResult<&[u8], Seek> {
    let (input, size) = vint(input)?;
    let (input, mut data) = nom::take!(input, size)?;

    let mut seek = Seek::default();
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

pub fn info(input: &[u8]) -> IResult<&[u8], Level1Element> {
    let (input, size) = vint(input)?;
    let (input, mut data) = nom::take!(input, size)?;

    let mut info = Info::default();
    while !data.is_empty() {
        let id;
        element!(data, id, vid);
        match id {
            0x73A4 => element!(data, info.uid, binary),
            0x7384 => element!(data, info.filename, string),
            0x3CB923 => element!(data, info.prev_uid, binary),
            0x3C83AB => element!(data, info.prev_filename, string),
            0x3EB923 => element!(data, info.next_uid, binary),
            0x3C83BB => element!(data, info.next_filename, string),
            0x2AD7B1 => element!(data, info.timecode_scale, uint),
            0x4489 => element!(data, info.duration, float),
            0x7BA9 => element!(data, info.title, string),
            0x4D80 => element!(data, info.muxing_app, string),
            0x5741 => element!(data, info.writing_app, string),
            0x4461 => element!(data, info.date_utc, uint),
            _ => skip!(data, id),
        }
    }

    Ok((input, Level1Element::Info(info)))
}

pub fn cluster(input: &[u8]) -> IResult<&[u8], Level1Element> {
    let (input, size) = vint(input)?;
    let (input, _) = nom::take!(input, size)?;
    Ok((input, Level1Element::Cluster))
}

pub fn chapters(input: &[u8]) -> IResult<&[u8], Level1Element> {
    let (input, size) = vint(input)?;
    let (input, _) = nom::take!(input, size)?;
    Ok((input, Level1Element::Chapters))
}

pub fn tags(input: &[u8]) -> IResult<&[u8], Level1Element> {
    let (input, size) = vint(input)?;
    let (input, _) = nom::take!(input, size)?;
    Ok((input, Level1Element::Tags))
}

pub fn attachments(input: &[u8]) -> IResult<&[u8], Level1Element> {
    let (input, size) = vint(input)?;
    let (input, _) = nom::take!(input, size)?;
    Ok((input, Level1Element::Attachments))
}

pub fn tracks(input: &[u8]) -> IResult<&[u8], Level1Element> {
    let (input, size) = vint(input)?;
    let (input, mut data) = nom::take!(input, size)?;

    let mut tracks = Tracks::default();
    while !data.is_empty() {
        let id;
        element!(data, id, vid);
        match id {
            0xAE => {
                let t;
                element!(data, t, track);
                tracks.tracks.push(t);
            }
            _ => skip!(data, id),
        }
    }

    Ok((input, Level1Element::Tracks(tracks)))
}

pub fn track(input: &[u8]) -> IResult<&[u8], Track> {
    let (input, size) = vint(input)?;
    let (input, mut data) = nom::take!(input, size)?;

    let mut track = Track::default();
    while !data.is_empty() {
        let id;
        element!(data, id, vid);
        match id {
            0xD7 => element!(data, track.number, uint),
            _ => skip!(data, id),
        }
    }

    Ok((input, track))
}

pub fn cues(input: &[u8]) -> IResult<&[u8], Level1Element> {
    let (input, size) = vint(input)?;
    let (input, _) = nom::take!(input, size)?;
    Ok((input, Level1Element::Cues))
}

pub fn level1_element(input: &[u8]) -> IResult<&[u8], Level1Element> {
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
            Ok((input, Level1Element::Void(size as u64)))
        }
        _ => {
            let (input, size) = skip(input)?;
            Ok((input, Level1Element::Unknown(size as u64)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ebml;

    const WEBM: &'static [u8] = include_bytes!("../assets/big-buck-bunny_trailer.webm");

    #[test]
    fn test_webm_segment() {
        let res = ebml::parse(&WEBM[..]);
        assert!(res.is_ok());
        let (_, (_, segment)) = res.unwrap();

        let input = segment.content;

        let res = level1_element(input);
        assert!(res.is_ok());
        let (input, element) = res.unwrap();
        match element {
            Level1Element::SeekHead(_) => (),
            _ => panic!()
        }

        let res = level1_element(input);
        assert!(res.is_ok());
        let (input, element) = res.unwrap();
        match element {
            Level1Element::Info(_) => (),
            _ => panic!()
        }

        let res = level1_element(input);
        assert!(res.is_ok());
        let (input, element) = res.unwrap();
        match element {
            Level1Element::Tracks(_) => (),
            _ => panic!()
        }

        let res = level1_element(input);
        assert!(res.is_ok());
        let (input, element) = res.unwrap();
        match element {
            Level1Element::Cues => (),
            _ => panic!()
        }

        let res = level1_element(input);
        assert!(res.is_ok());
        let (_input, element) = res.unwrap();
        match element {
            Level1Element::Cluster => (),
            _ => panic!()
        }
    }
}