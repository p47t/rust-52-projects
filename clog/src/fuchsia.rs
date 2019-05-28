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
                r"(((?P<tag>[A-Z]+):\s+)?((?P<source>[a-zA-Z_]+):\s+)?)?",
                r"(?P<text>.*)",
            r")?",
        )
    ).unwrap();
}

fn to_tag(t: &str) -> &str {
    match t {
        "ERROR" => "error",
        "WARNING" => "warning",
        "INFO" => "info",
        _ => "text",
    }
}

pub fn parse_line(line: &str) -> Result<Vec<Field>, ()> {
    use Field::*;

    if let Some(cap) = RE_LOG.captures(line) {
        let tag = to_tag(cap.name("tag").unwrap().as_str());
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
                let tag = to_tag(cap.name("tag").unwrap().as_str());
                ret.extend(vec![
                    Space(tag.into(), cap["tag"].into()),
                    Plain("text".into(), ":".into()),
                ]);
                if let Some(_) = cap.name("source") {
                    ret.extend(vec![
                        Space("source".into(), cap["source"].into()),
                        Plain("text".into(), ":".into()),
                    ]);
                }
                ret.extend(vec![
                    Space(tag.into(), cap["text"].into()),
                ]);
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

    #[test]
    fn test_log() {
        let r = parse_line("[00050.844] 14025:14037> ERROR: setupLoaderTermPhysDevs:  Failed to detect any valid GPUs in the current config").unwrap();
        assert_eq!(r, vec![
            Field::Plain("text".into(), "[".into()),
            Field::Plain("time".into(), "00050.844".into()),
            Field::Plain("text".into(), "]".into()),
            Field::Space("time".into(), "14025:14037".into()),
            Field::Plain("text".into(), ">".into()),
            Field::Space("error".into(), "ERROR".into()),
            Field::Plain("text".into(), ":".into()),
            Field::Space("source".into(), "setupLoaderTermPhysDevs".into()),
            Field::Plain("text".into(), ":".into()),
            Field::Space("error".into(), "Failed to detect any valid GPUs in the current config".into()),
        ]);
    }
}