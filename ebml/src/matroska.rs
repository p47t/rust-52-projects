use nom::IResult;
use crate::ebml::{vid, vint, skip, binary, float, uint, string};
use crate::ebml;

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

impl Level1Element {
    pub fn parse(input: &[u8]) -> IResult<&[u8], Level1Element> {
        let (input, id) = vid(input)?;
        match id {
            0x114D9B74 => {
                // Convert the result to the common Level1Element type
                SeekHead::parse(input).map(|(i, val)| {
                    (i, Level1Element::SeekHead(val))
                })
            }
            0x1549A966 => {
                Info::parse(input).map(|(i, val)| {
                    (i, Level1Element::Info(val))
                })
            }
            0x1F43B675 => cluster(input),
            0x1043A770 => chapters(input),
            0x1254C367 => tags(input),
            0x1941A469 => attachments(input),
            0x1654AE6B => {
                Tracks::parse(input).map(|(i, val)| {
                    (i, Level1Element::Tracks(val))
                })
            }
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
}

#[derive(Default)]
pub struct SeekHead {
    pub positions: Vec<Seek>,
}

impl SeekHead {
    pub fn parse(input: &[u8]) -> IResult<&[u8], SeekHead> {
        let (input, size) = vint(input)?;
        let (input, mut data) = nom::take!(input, size)?;

        let mut seek_head = SeekHead::default();
        while !data.is_empty() {
            let id;
            element!(data, id, vid);
            match id {
                0x4DBB => {
                    let val;
                    element!(data, val, Seek::parse);
                    seek_head.positions.push(val);
                }
                _ => skip!(data, id),
            }
        }

        Ok((input, seek_head))
    }
}

#[derive(Default)]
pub struct Seek {
    pub id: Vec<u8>,
    pub position: u64,
}

impl Seek {
    pub fn parse(input: &[u8]) -> IResult<&[u8], Seek> {
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

impl Info {
    pub fn parse(input: &[u8]) -> IResult<&[u8], Info> {
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

        Ok((input, info))
    }
}

#[derive(Default)]
pub struct Tracks {
    pub tracks: Vec<Track>
}

impl Tracks {
    pub fn parse(input: &[u8]) -> IResult<&[u8], Tracks> {
        let (input, size) = vint(input)?;
        let (input, mut data) = nom::take!(input, size)?;

        let mut tracks = Tracks::default();
        while !data.is_empty() {
            let id;
            element!(data, id, vid);
            match id {
                0xAE => {
                    let t;
                    element!(data, t, Track::parse);
                    tracks.tracks.push(t);
                }
                _ => skip!(data, id),
            }
        }

        Ok((input, tracks))
    }
}

#[derive(Default)]
pub struct Track {
    pub number: u64,
    pub uid: u64,
    pub typ3: u64,
    pub enabled: bool,
    pub default: bool,
    pub forced: bool,
    pub lacing: bool,
    pub min_cache: u64,
    pub max_cache: u64,
    pub default_duration: u64,
    pub timecode_scale: f64,
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

impl Track {
    pub fn new() -> Self {
        Track {
            enabled: true,
            default: true,
            ..Default::default()
        }
    }

    pub fn parse(input: &[u8]) -> IResult<&[u8], Track> {
        let (input, size) = vint(input)?;
        let (input, mut data) = nom::take!(input, size)?;

        let mut track = Track::new();
        while !data.is_empty() {
            let id;
            element!(data, id, vid);
            match id {
                0xD7 => element!(data, track.number, uint),
                0x73C5 => element!(data, track.uid, uint),
                0x83 => element!(data, track.typ3, uint),
                0xB9 => element!(data, track.enabled, ebml::bool),
                0x55AA => element!(data, track.forced, ebml::bool),
                0x9C => element!(data, track.lacing, ebml::bool),
                0x6DE7 => element!(data, track.min_cache, uint),
                0x6DF8 => element!(data, track.max_cache, uint),
                0x23E383 => element!(data, track.default_duration, uint),
                0x23314F => element!(data, track.timecode_scale, float),
                0x536E => element!(data, track.name, string),
                0x22B59C => element!(data, track.language, string),
                0x86 => element!(data, track.codec_id, string),
                0x63A2 => element!(data, track.codec_private, binary),
                0x258688 => element!(data, track.codec_name, string),
                0x7446 => element!(data, track.attachment_link, uint),
                0xE0 => element!(data, track.video, Video::parse),
                0xE1 => element!(data, track.audio, Audio::parse),
                0x6D80 => element!(data, track.content_encodings, ContentEncodings::parse),
                _ => skip!(data, id),
            }
        }

        Ok((input, track))
    }
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
    pub display_unit: u64,
}

impl Video {
    pub fn parse(input: &[u8]) -> IResult<&[u8], Self> {
        let (input, size) = vint(input)?;
        let (input, mut data) = nom::take!(input, size)?;

        let mut video = Video::default();
        while !data.is_empty() {
            let id;
            element!(data, id, vid);
            match id {
                0xB0 => element!(data, video.pixel_width, uint),
                0xBA => element!(data, video.pixel_height, uint),
                0x54AA => element!(data, video.pixel_crop_bottom, uint),
                0x54BB => element!(data, video.pixel_crop_top, uint),
                0x54CC => element!(data, video.pixel_crop_left, uint),
                0x54DD => element!(data, video.pixel_crop_right, uint),
                0x54B0 => element!(data, video.display_width, uint),
                0x54BA => element!(data, video.display_height, uint),
                0x54B2 => element!(data, video.display_unit, uint),
                _ => skip!(data, id),
            }
        }

        if video.display_width == 0 {
            video.display_width = video.pixel_width;
        }
        if video.display_height == 0 {
            video.display_height = video.pixel_height;
        }

        Ok((input, video))
    }
}

#[derive(Default)]
pub struct Audio {
    pub sampling_frequency: u64,
    pub output_sampling_frequency: u64,
    pub channels: u64,
    pub bit_depth: u64,
}

impl Audio {
    pub fn new() -> Audio {
        Audio {
            sampling_frequency: 8000,
            channels: 1,
            ..Default::default()
        }
    }

    pub fn parse(input: &[u8]) -> IResult<&[u8], Self> {
        let (input, size) = vint(input)?;
        let (input, mut data) = nom::take!(input, size)?;

        let mut audio = Audio::new();
        while !data.is_empty() {
            let id;
            element!(data, id, vid);
            match id {
                0xB5 => element!(data, audio.sampling_frequency, uint),
                0x78B5 => element!(data, audio.output_sampling_frequency, uint),
                0x9F => element!(data, audio.channels, uint),
                0x6264 => element!(data, audio.bit_depth, uint),
                _ => skip!(data, id),
            }
        }

        if audio.output_sampling_frequency == 0 {
            audio.output_sampling_frequency = audio.sampling_frequency;
        }

        Ok((input, audio))
    }
}

#[derive(Default)]
pub struct ContentEncodings {}

impl ContentEncodings {
    pub fn parse(input: &[u8]) -> IResult<&[u8], Self> {
        let (i, _) = ebml::skip(input)?;
        Ok((i, Self::default()))
    }
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

pub fn cues(input: &[u8]) -> IResult<&[u8], Level1Element> {
    let (input, size) = vint(input)?;
    let (input, _) = nom::take!(input, size)?;
    Ok((input, Level1Element::Cues))
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

        let res = Level1Element::parse(input);
        assert!(res.is_ok());
        let (input, element) = res.unwrap();
        match element {
            Level1Element::SeekHead(_) => (),
            _ => panic!()
        }

        let res = Level1Element::parse(input);
        assert!(res.is_ok());
        let (input, element) = res.unwrap();
        match element {
            Level1Element::Info(_) => (),
            _ => panic!()
        }

        let res = Level1Element::parse(input);
        assert!(res.is_ok());
        let (input, element) = res.unwrap();
        match element {
            Level1Element::Tracks(_) => (),
            _ => panic!()
        }

        let res = Level1Element::parse(input);
        assert!(res.is_ok());
        let (input, element) = res.unwrap();
        match element {
            Level1Element::Cues => (),
            _ => panic!()
        }

        let res = Level1Element::parse(input);
        assert!(res.is_ok());
        let (_input, element) = res.unwrap();
        match element {
            Level1Element::Cluster => (),
            _ => panic!()
        }
    }
}