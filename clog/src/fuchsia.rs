use anyhow::anyhow;
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
                r"(((?P<tag>[A-Z]+):\s+)?((?P<source>[a-zA-Z0-9_\-\.\(\)]+):\s+)?)?",
                r"(?P<text>.*)",
            r")?",
        )
    ).unwrap();
}

fn tag_to_class(t: &str) -> Option<&'static str> {
    match t {
        "ERROR" => Some(".error"),
        "WARNING" => Some(".warning"),
        "INFO" => Some(".info"),
        _ => None,
    }
}

pub fn parse_line(line: &str) -> Result<Vec<Field>, anyhow::Error> {
    if let Some(cap) = RE_LOG.captures(line) {
        match tag_to_class(cap.name("tag").unwrap().as_str()) {
            Some(class) => Ok(vec![
                Field::new("[", ".time", cap.name("time0").unwrap().as_str(), "]"),
                Field::new(" ", ".time", cap.name("time1").unwrap().as_str(), ">"),
                Field::new(" [", class, cap.name("tag").unwrap().as_str(), ":"),
                Field::pos(".source", cap.name("source").unwrap().as_str(), "]"),
                Field::pre(" ", class, cap.name("text").unwrap().as_str()),
            ]),
            _ => Err(anyhow!("class not found")),
        }
    } else if let Some(cap) = RE_KERNEL_LOG.captures(line) {
        let mut ret = vec![
            Field::new("[", ".time", cap.name("time0").unwrap().as_str(), "]"),
            Field::new(" ", ".time", cap.name("time1").unwrap().as_str(), ">"),
        ];
        if let Some(_) = cap.name("content") {
            if let Some(_) = cap.name("tag") {
                let tag = cap.name("tag").unwrap().as_str();
                if let Some(class) = tag_to_class(tag) {
                    ret.extend(vec![
                        Field::new(" ", class, tag, ":"),
                    ]);
                } else {
                    // treat it as source
                    ret.extend(vec![
                        Field::new(" ", ".source", tag, ":"),
                    ]);
                }
                if let Some(_) = cap.name("source") {
                    ret.extend(vec![
                        Field::new(" ", ".source", cap.name("source").unwrap().as_str(), ":"),
                    ]);
                }
                if let Some(class) = tag_to_class(tag) {
                    ret.extend(vec![
                        Field::pre(" ", class, cap.name("text").unwrap().as_str()),
                    ]);
                } else {
                    ret.extend(vec![
                        Field::pre(" ", ".text", cap.name("text").unwrap().as_str()),
                    ]);
                }
            } else if let Some(_) = cap.name("source") {
                ret.extend(vec![
                    Field::new(" ", ".source", cap.name("source").unwrap().as_str(), ":"),
                    Field::pre(" ", ".text", cap.name("text").unwrap().as_str()),
                ]);
            } else {
                ret.extend(vec![
                    Field::pre(" ", ".text", cap.name("text").unwrap().as_str()),
                ]);
            }
        }
        Ok(ret)
    } else {
        Ok(vec![Field::new("", ".text", line.trim(), "")])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log() {
        let r = parse_line("[00050.844] 14025:14037>").unwrap();
        assert_eq!(r, vec![
            Field::new("[", ".time", "00050.844", "]"),
            Field::new(" ", ".time", "14025:14037", ">"),
            Field::pre(" ", ".text", ""),
        ]);

        let r = parse_line("[00050.844] 14025:14037> INIT: cpu 0, calling hook").unwrap();
        assert_eq!(r, vec![
            Field::new("[", ".time", "00050.844", "]"),
            Field::new(" ", ".time", "14025:14037", ">"),
            Field::new(" ", ".source", "INIT", ":"),
            Field::pre(" ", ".text", "cpu 0, calling hook"),
        ]);

        let r = parse_line("[00050.844] 14025:14037> WARNING: unable to find any cache levels.").unwrap();
        assert_eq!(r, vec![
            Field::new("[", ".time", "00050.844", "]"),
            Field::new(" ", ".time", "14025:14037", ">"),
            Field::new(" ", ".warning", "WARNING", ":"),
            Field::pre(" ", ".warning", "unable to find any cache levels."),
        ]);

        let r = parse_line("[00050.844] 14025:14037> ERROR: setupLoaderTermPhysDevs: Failed to detect any valid GPUs in the current config").unwrap();
        assert_eq!(r, vec![
            Field::new("[", ".time", "00050.844", "]"),
            Field::new(" ", ".time", "14025:14037", ">"),
            Field::new(" ", ".error", "ERROR", ":"),
            Field::new(" ", ".source", "setupLoaderTermPhysDevs", ":"),
            Field::pre(" ", ".error", "Failed to detect any valid GPUs in the current config"),
        ]);

        let r = parse_line("[00050.844] 14025:14037> [INFO:namespace_builder.cc(44)] config-data for fonts").unwrap();
        assert_eq!(r, vec![
            Field::new("[", ".time", "00050.844", "]"),
            Field::new(" ", ".time", "14025:14037", ">"),
            Field::new(" [", ".info", "INFO", ":"),
            Field::pos(".source", "namespace_builder.cc(44)", "]"),
            Field::pre(" ", ".info", "config-data for fonts"),
        ]);
    }
}