use std::fmt;

/// State of a connected ADB device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeviceState {
    /// Fully operational device.
    Device,
    /// Device is offline.
    Offline,
    /// Device requires USB debugging authorization.
    Unauthorized,
    /// Device is in the process of being authorized.
    Authorizing,
    /// Insufficient permissions to communicate with device.
    NoPermissions,
    /// Unknown state from the ADB server.
    Unknown(String),
}

impl DeviceState {
    pub fn parse(s: &str) -> Self {
        match s {
            "device" => DeviceState::Device,
            "offline" => DeviceState::Offline,
            "unauthorized" => DeviceState::Unauthorized,
            "authorizing" => DeviceState::Authorizing,
            "no permissions" => DeviceState::NoPermissions,
            other => DeviceState::Unknown(other.to_string()),
        }
    }
}

impl fmt::Display for DeviceState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DeviceState::Device => write!(f, "device"),
            DeviceState::Offline => write!(f, "offline"),
            DeviceState::Unauthorized => write!(f, "unauthorized"),
            DeviceState::Authorizing => write!(f, "authorizing"),
            DeviceState::NoPermissions => write!(f, "no permissions"),
            DeviceState::Unknown(s) => write!(f, "{}", s),
        }
    }
}

/// Information about a connected Android device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceInfo {
    /// Device serial number (e.g., "emulator-5554", "R5CT200XXXX").
    pub serial: String,
    /// Current device state.
    pub state: DeviceState,
}

impl DeviceInfo {
    /// Parse the `serial\tstate\n` format returned by `host:devices`.
    ///
    /// Example input: `"emulator-5554\tdevice\nR5CT200XXXX\tunauthorized\n"`
    pub fn parse_device_list(data: &str) -> Vec<DeviceInfo> {
        data.lines()
            .filter(|line| !line.is_empty())
            .filter_map(|line| {
                let mut parts = line.split('\t');
                let serial = parts.next()?.to_string();
                let state_str = parts.next()?;
                Some(DeviceInfo {
                    serial,
                    state: DeviceState::parse(state_str),
                })
            })
            .collect()
    }
}

impl fmt::Display for DeviceInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}\t{}", self.serial, self.state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_device_list_single() {
        let data = "emulator-5554\tdevice\n";
        let devices = DeviceInfo::parse_device_list(data);
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].serial, "emulator-5554");
        assert_eq!(devices[0].state, DeviceState::Device);
    }

    #[test]
    fn test_parse_device_list_multiple() {
        let data = "emulator-5554\tdevice\nR5CT200XXXX\tunauthorized\n";
        let devices = DeviceInfo::parse_device_list(data);
        assert_eq!(devices.len(), 2);
        assert_eq!(devices[0].serial, "emulator-5554");
        assert_eq!(devices[0].state, DeviceState::Device);
        assert_eq!(devices[1].serial, "R5CT200XXXX");
        assert_eq!(devices[1].state, DeviceState::Unauthorized);
    }

    #[test]
    fn test_parse_device_list_empty() {
        let devices = DeviceInfo::parse_device_list("");
        assert!(devices.is_empty());
    }

    #[test]
    fn test_parse_device_list_blank_lines() {
        let data = "\nemulator-5554\tdevice\n\n";
        let devices = DeviceInfo::parse_device_list(data);
        assert_eq!(devices.len(), 1);
    }

    #[test]
    fn test_device_state_display_round_trip() {
        let states = ["device", "offline", "unauthorized", "authorizing"];
        for s in &states {
            let state = DeviceState::parse(s);
            assert_eq!(state.to_string(), *s);
        }
    }

    #[test]
    fn test_device_state_unknown() {
        let state = DeviceState::parse("recovery");
        assert_eq!(state, DeviceState::Unknown("recovery".into()));
        assert_eq!(state.to_string(), "recovery");
    }

    #[test]
    fn test_device_info_display() {
        let info = DeviceInfo {
            serial: "emulator-5554".into(),
            state: DeviceState::Device,
        };
        assert_eq!(info.to_string(), "emulator-5554\tdevice");
    }
}
