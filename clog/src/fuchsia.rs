use lazy_static::lazy_static;
use regex::Regex;
use thiserror::Error;

use crate::Field;

lazy_static! {
    static ref RE_LOG: Regex = Regex::new(concat!(
        r"\[(?P<time0>\d{5}\.\d{3})]\s+",
        r"(?P<time1>\d{5}:\d{5})>\s*",
        r"(?P<content>",
        r"\[(?P<tag>\w+):(?P<source>.*)]\s*",
        r"(?P<text>.*)",
        r")",
    ))
    .unwrap();
    static ref RE_KERNEL_LOG: Regex = Regex::new(concat!(
        r"\[(?P<time0>\d{5}\.\d{3})]\s+",
        r"(?P<time1>\d{5}:\d{5})>\s*",
        r"(?P<content>",
        r"(((?P<tag>[A-Z]+):\s+)?((?P<source>[a-zA-Z0-9_\-\.\(\)]+):\s+)?)?",
        r"(?P<text>.*)",
        r")?",
    ))
    .unwrap();
}

fn tag_to_class(t: &str) -> Option<&'static str> {
    match t {
        "ERROR" => Some(".error"),
        "WARNING" => Some(".warning"),
        "INFO" => Some(".info"),
        _ => None,
    }
}

#[derive(Error, Debug)]
pub enum ParseError {
    #[error("unmatched line")]
    Unmatched,
    #[error("missing field")]
    MissingField,
}

/// To get a named field
trait FieldStr<'t> {
    fn field(&self, name: &str) -> Result<&'t str, ParseError>;
}

/// Extend Captures to get named field with Result
impl<'t> FieldStr<'t> for regex::Captures<'t> {
    fn field(&self, name: &str) -> Result<&'t str, ParseError> {
        Ok(self.name(name).ok_or(ParseError::MissingField)?.as_str())
    }
}

pub fn parse_line(line: &str) -> Result<Vec<Field<'_>>, ParseError> {
    if let Some(cap) = RE_LOG.captures(line) {
        match tag_to_class(cap.field("tag")?) {
            Some(class) => Ok(vec![
                Field::new("[", ".time", cap.field("time0")?, "]"),
                Field::new(" ", ".time", cap.field("time1")?, ">"),
                Field::new(" [", class, cap.field("tag")?, ":"),
                Field::pos(".source", cap.field("source")?, "]"),
                Field::pre(" ", class, cap.field("text")?),
            ]),
            _ => Err(ParseError::Unmatched),
        }
    } else if let Some(cap) = RE_KERNEL_LOG.captures(line) {
        let mut ret = vec![
            Field::new("[", ".time", cap.field("time0")?, "]"),
            Field::new(" ", ".time", cap.field("time1")?, ">"),
        ];
        if cap.name("content").is_some() {
            if cap.name("tag").is_some() {
                let tag = cap.field("tag")?;
                if let Some(class) = tag_to_class(tag) {
                    ret.extend(vec![Field::new(" ", class, tag, ":")]);
                } else {
                    // treat it as source
                    ret.extend(vec![Field::new(" ", ".source", tag, ":")]);
                }
                if cap.name("source").is_some() {
                    ret.extend(vec![Field::new(" ", ".source", cap.field("source")?, ":")]);
                }
                if let Some(class) = tag_to_class(tag) {
                    ret.extend(vec![Field::pre(" ", class, cap.field("text")?)]);
                } else {
                    ret.extend(vec![Field::pre(" ", ".text", cap.field("text")?)]);
                }
            } else if cap.name("source").is_some() {
                ret.extend(vec![
                    Field::new(" ", ".source", cap.field("source")?, ":"),
                    Field::pre(" ", ".text", cap.field("text")?),
                ]);
            } else {
                ret.extend(vec![Field::pre(" ", ".text", cap.field("text")?)]);
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
        assert_eq!(
            r,
            vec![
                Field::new("[", ".time", "00050.844", "]"),
                Field::new(" ", ".time", "14025:14037", ">"),
                Field::pre(" ", ".text", ""),
            ]
        );

        let r = parse_line("[00050.844] 14025:14037> INIT: cpu 0, calling hook").unwrap();
        assert_eq!(
            r,
            vec![
                Field::new("[", ".time", "00050.844", "]"),
                Field::new(" ", ".time", "14025:14037", ">"),
                Field::new(" ", ".source", "INIT", ":"),
                Field::pre(" ", ".text", "cpu 0, calling hook"),
            ]
        );

        let r = parse_line("[00050.844] 14025:14037> WARNING: unable to find any cache levels.")
            .unwrap();
        assert_eq!(
            r,
            vec![
                Field::new("[", ".time", "00050.844", "]"),
                Field::new(" ", ".time", "14025:14037", ">"),
                Field::new(" ", ".warning", "WARNING", ":"),
                Field::pre(" ", ".warning", "unable to find any cache levels."),
            ]
        );

        let r = parse_line("[00050.844] 14025:14037> ERROR: setupLoaderTermPhysDevs: Failed to detect any valid GPUs in the current config").unwrap();
        assert_eq!(
            r,
            vec![
                Field::new("[", ".time", "00050.844", "]"),
                Field::new(" ", ".time", "14025:14037", ">"),
                Field::new(" ", ".error", "ERROR", ":"),
                Field::new(" ", ".source", "setupLoaderTermPhysDevs", ":"),
                Field::pre(
                    " ",
                    ".error",
                    "Failed to detect any valid GPUs in the current config"
                ),
            ]
        );

        let r = parse_line(
            "[00050.844] 14025:14037> [INFO:namespace_builder.cc(44)] config-data for fonts",
        )
        .unwrap();
        assert_eq!(
            r,
            vec![
                Field::new("[", ".time", "00050.844", "]"),
                Field::new(" ", ".time", "14025:14037", ">"),
                Field::new(" [", ".info", "INFO", ":"),
                Field::pos(".source", "namespace_builder.cc(44)", "]"),
                Field::pre(" ", ".info", "config-data for fonts"),
            ]
        );
    }
}
