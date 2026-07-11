use std::io::Write;

/// Triggers Windows PowerShell native TTS (`System.Speech.Synthesis.SpeechSynthesizer`)
/// to speak the response back to the user without any compilation dependencies.
pub fn speak(text: &str) {
    if text.trim().is_empty() {
        return;
    }

    let script = format!(
        "Add-Type -AssemblyName System.Speech; \
         $synth = New-Object System.Speech.Synthesis.SpeechSynthesizer; \
         $synth.Speak([Console]::In.ReadToEnd())"
    );

    let child = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", &script])
        .stdin(std::process::Stdio::piped())
        .spawn()
        .ok();

    if let Some(mut c) = child {
        if let Some(mut stdin) = c.stdin.take() {
            let _ = stdin.write_all(text.as_bytes());
        }
        let _ = c.wait();
    }
}

/// Plays a quick audio beep through powershell.
pub fn play_notification_beep(frequency: f32, duration_ms: u64) {
    let script = format!(
        "[System.Console]::Beep({}, {})",
        frequency as u32, duration_ms
    );
    let _ = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", &script])
        .spawn();
}
