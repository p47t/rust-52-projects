use regex::Regex;

use lazy_static::lazy_static;

use crate::Field;

lazy_static! {
    static ref RE_LOG: Regex = Regex::new(
        concat!(
            r"\[(?P<time0>\d{5}\.\d{3})]\s+",
            r"(?P<time1>\d{5}:\d{5})>\s*",
            r"(?P<content>",
                r"\[(?P<tag>\w+):(?P<source>.*)]\s*",
                r"(?P<text>.*)",
            r")",
        )
    ).unwrap();
    static ref RE_KERNEL_LOG: Regex = Regex::new(
        concat!(
            r"\[(?P<time0>\d{5}\.\d{3})]\s+",
            r"(?P<time1>\d{5}:\d{5})>\s*",
            r"(?P<content>",
                r"(((?P<tag>[A-Z]+):\s+)?((?P<source>[a-zA-Z0-9_]+):\s+)?)?",
                r"(?P<text>.*)",
            r")?",
        )
    ).unwrap();
}

fn to_tag(t: &str) -> Option<&str> {
    match t {
        "ERROR" => Some("error"),
        "WARNING" => Some("warning"),
        "INFO" => Some("info"),
        _ => None,
    }
}

pub fn parse_line(line: &str) -> Result<Vec<Field>, ()> {
    use Field::*;

    if let Some(cap) = RE_LOG.captures(line) {
        let tag = to_tag(cap.name("tag").unwrap().as_str()).unwrap();
        Ok(vec![
            Plain("text".into(), "[".into()),
            Plain("time".into(), cap["time0"].into()),
            Plain("text".into(), "]".into()),
            Space("time".into(), cap["time1"].into()),
            Plain("text".into(), ">".into()),
            Space("text".into(), "[".into()),
            Plain(tag.into(), cap["tag"].into()),
            Plain("text".into(), ":".into()),
            Plain("source".into(), cap["source"].into()),
            Plain("text".into(), "]".into()),
            Space(tag.into(), cap["text"].into()),
        ])
    } else if let Some(cap) = RE_KERNEL_LOG.captures(line) {
        let mut ret = vec![
            Plain("text".into(), "[".into()),
            Plain("time".into(), cap["time0"].into()),
            Plain("text".into(), "]".into()),
            Space("time".into(), cap["time1"].into()),
            Plain("text".into(), ">".into()),
        ];
        if let Some(_) = cap.name("content") {
            if let Some(_) = cap.name("tag") {
                let raw_tag = cap.name("tag").unwrap().as_str();
                if let Some(tag) = to_tag(raw_tag) {
                    ret.extend(vec![
                        Space(tag.into(), raw_tag.into()),
                        Plain("text".into(), ":".into()),
                    ]);
                } else {
                    // treat it as source
                    ret.extend(vec![
                        Space("source".into(), raw_tag.into()),
                        Plain("text".into(), ":".into()),
                    ]);
                }
                if let Some(_) = cap.name("source") {
                    ret.extend(vec![
                        Space("source".into(), cap["source"].into()),
                        Plain("text".into(), ":".into()),
                    ]);
                }
                if let Some(tag) = to_tag(raw_tag) {
                    ret.extend(vec![
                        Space(tag.into(), cap["text"].into()),
                    ]);
                } else {
                    ret.extend(vec![
                        Space("text".into(), cap["text"].into()),
                    ]);
                }
            } else if let Some(_) = cap.name("source") {
                ret.extend(vec![
                    Space("source".into(), cap["source"].into()),
                    Plain("text".into(), ":".into()),
                    Space("text".into(), cap["text"].into()),
                ]);
            } else {
                ret.extend(vec![
                    Space("text".into(), cap["text"].into()),
                ]);
            }
        }
        Ok(ret)
    } else {
        Ok(vec![Plain("text".into(), line.trim().into())])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Field::*;

    #[test]
    fn test_log() {
        let r = parse_line("[00050.844] 14025:14037>").unwrap();
        assert_eq!(r, vec![
            Plain("text".into(), "[".into()),
            Plain("time".into(), "00050.844".into()),
            Plain("text".into(), "]".into()),
            Space("time".into(), "14025:14037".into()),
            Plain("text".into(), ">".into()),
            Space("text".into(), "".into()),
        ]);

        let r = parse_line("[00050.844] 14025:14037> INIT: cpu 0, calling hook").unwrap();
        assert_eq!(r, vec![
            Plain("text".into(), "[".into()),
            Plain("time".into(), "00050.844".into()),
            Plain("text".into(), "]".into()),
            Space("time".into(), "14025:14037".into()),
            Plain("text".into(), ">".into()),
            Space("source".into(), "INIT".into()),
            Plain("text".into(), ":".into()),
            Space("text".into(), "cpu 0, calling hook".into()),
        ]);

        let r = parse_line("[00050.844] 14025:14037> WARNING: unable to find any cache levels.").unwrap();
        assert_eq!(r, vec![
            Plain("text".into(), "[".into()),
            Plain("time".into(), "00050.844".into()),
            Plain("text".into(), "]".into()),
            Space("time".into(), "14025:14037".into()),
            Plain("text".into(), ">".into()),
            Space("warning".into(), "WARNING".into()),
            Plain("text".into(), ":".into()),
            Space("warning".into(), "unable to find any cache levels.".into()),
        ]);

        let r = parse_line("[00050.844] 14025:14037> ERROR: setupLoaderTermPhysDevs: Failed to detect any valid GPUs in the current config").unwrap();
        assert_eq!(r, vec![
            Plain("text".into(), "[".into()),
            Plain("time".into(), "00050.844".into()),
            Plain("text".into(), "]".into()),
            Space("time".into(), "14025:14037".into()),
            Plain("text".into(), ">".into()),
            Space("error".into(), "ERROR".into()),
            Plain("text".into(), ":".into()),
            Space("source".into(), "setupLoaderTermPhysDevs".into()),
            Plain("text".into(), ":".into()),
            Space("error".into(), "Failed to detect any valid GPUs in the current config".into()),
        ]);

        let r = parse_line("[00050.844] 14025:14037> ERROR: setupLoaderTermPhysDevs:  Failed to detect any valid GPUs in the current config").unwrap();
        assert_eq!(r, vec![
            Plain("text".into(), "[".into()),
            Plain("time".into(), "00050.844".into()),
            Plain("text".into(), "]".into()),
            Space("time".into(), "14025:14037".into()),
            Plain("text".into(), ">".into()),
            Space("error".into(), "ERROR".into()),
            Plain("text".into(), ":".into()),
            Space("source".into(), "setupLoaderTermPhysDevs".into()),
            Plain("text".into(), ":".into()),
            Space("error".into(), "Failed to detect any valid GPUs in the current config".into()),
        ]);
    }
}