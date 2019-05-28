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

fn to_tag(t: &str) -> Option<&'static str> {
    match t {
        "ERROR" => Some("error"),
        "WARNING" => Some("warning"),
        "INFO" => Some("info"),
        _ => None,
    }
}

pub fn parse_line(line: &str) -> Result<Vec<Field>, std::option::NoneError> {
    use Field::*;

    if let Some(cap) = RE_LOG.captures(line) {
        let tag = to_tag(cap.name("tag")?.as_str())?;
        Ok(vec![
            Plain("text", "["),
            Plain("time", cap.name("time0")?.as_str()),
            Plain("text", "]"),
            Space("time", cap.name("time1")?.as_str()),
            Plain("text", ">"),
            Space("text", "["),
            Plain(tag, cap.name("tag")?.as_str()),
            Plain("text", ":"),
            Plain("source", cap.name("source")?.as_str()),
            Plain("text", "]"),
            Space(tag, cap.name("text")?.as_str()),
        ])
    } else if let Some(cap) = RE_KERNEL_LOG.captures(line) {
        let mut ret = vec![
            Plain("text", "["),
            Plain("time", cap.name("time0")?.as_str()),
            Plain("text", "]"),
            Space("time", cap.name("time1")?.as_str()),
            Plain("text", ">"),
        ];
        if let Some(_) = cap.name("content") {
            if let Some(_) = cap.name("tag") {
                let raw_tag = cap.name("tag")?.as_str();
                if let Some(tag) = to_tag(raw_tag) {
                    ret.extend(vec![
                        Space(tag, raw_tag),
                        Plain("text", ":"),
                    ]);
                } else {
                    // treat it as source
                    ret.extend(vec![
                        Space("source", raw_tag),
                        Plain("text", ":"),
                    ]);
                }
                if let Some(_) = cap.name("source") {
                    ret.extend(vec![
                        Space("source", cap.name("source")?.as_str()),
                        Plain("text", ":"),
                    ]);
                }
                if let Some(tag) = to_tag(raw_tag) {
                    ret.extend(vec![
                        Space(tag, cap.name("text")?.as_str()),
                    ]);
                } else {
                    ret.extend(vec![
                        Space("text", cap.name("text")?.as_str()),
                    ]);
                }
            } else if let Some(_) = cap.name("source") {
                ret.extend(vec![
                    Space("source", cap.name("source")?.as_str()),
                    Plain("text", ":"),
                    Space("text", cap.name("text")?.as_str()),
                ]);
            } else {
                ret.extend(vec![
                    Space("text", cap.name("text")?.as_str()),
                ]);
            }
        }
        Ok(ret)
    } else {
        Ok(vec![Plain("text", line.trim())])
    }
}

#[cfg(test)]
mod tests {
    use Field::*;

    use super::*;

    #[test]
    fn test_log() {
        let r = parse_line("[00050.844] 14025:14037>").unwrap();
        assert_eq!(r, vec![
            Plain("text", "["),
            Plain("time", "00050.844"),
            Plain("text", "]"),
            Space("time", "14025:14037"),
            Plain("text", ">"),
            Space("text", ""),
        ]);

        let r = parse_line("[00050.844] 14025:14037> INIT: cpu 0, calling hook").unwrap();
        assert_eq!(r, vec![
            Plain("text", "["),
            Plain("time", "00050.844"),
            Plain("text", "]"),
            Space("time", "14025:14037"),
            Plain("text", ">"),
            Space("source", "INIT"),
            Plain("text", ":"),
            Space("text", "cpu 0, calling hook"),
        ]);

        let r = parse_line("[00050.844] 14025:14037> WARNING: unable to find any cache levels.").unwrap();
        assert_eq!(r, vec![
            Plain("text", "["),
            Plain("time", "00050.844"),
            Plain("text", "]"),
            Space("time", "14025:14037"),
            Plain("text", ">"),
            Space("warning", "WARNING"),
            Plain("text", ":"),
            Space("warning", "unable to find any cache levels."),
        ]);

        let r = parse_line("[00050.844] 14025:14037> ERROR: setupLoaderTermPhysDevs: Failed to detect any valid GPUs in the current config").unwrap();
        assert_eq!(r, vec![
            Plain("text", "["),
            Plain("time", "00050.844"),
            Plain("text", "]"),
            Space("time", "14025:14037"),
            Plain("text", ">"),
            Space("error", "ERROR"),
            Plain("text", ":"),
            Space("source", "setupLoaderTermPhysDevs"),
            Plain("text", ":"),
            Space("error", "Failed to detect any valid GPUs in the current config"),
        ]);

        let r = parse_line("[00050.844] 14025:14037> ERROR: setupLoaderTermPhysDevs:  Failed to detect any valid GPUs in the current config").unwrap();
        assert_eq!(r, vec![
            Plain("text", "["),
            Plain("time", "00050.844"),
            Plain("text", "]"),
            Space("time", "14025:14037"),
            Plain("text", ">"),
            Space("error", "ERROR"),
            Plain("text", ":"),
            Space("source", "setupLoaderTermPhysDevs"),
            Plain("text", ":"),
            Space("error", "Failed to detect any valid GPUs in the current config"),
        ]);
    }
}