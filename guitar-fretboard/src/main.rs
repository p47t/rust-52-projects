use iced::widget::{button, canvas, column, container, row, stack, text, Canvas, Column, Row};
use iced::{color, Element, Length, Point, Rectangle, Renderer, Size, Theme};
use rand::Rng;
use rodio::{OutputStream, Sink, Source};
use std::time::Duration;

fn main() -> iced::Result {
    iced::application("Guitar Fretboard - C Major Scale", App::update, App::view)
        .theme(|_| Theme::TokyoNightStorm)
        .window_size((1400.0, 480.0))
        .run()
}

// Music theory constants
const CHROMATIC_NOTES: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];
const C_MAJOR_SCALE: [&str; 7] = ["C", "D", "E", "F", "G", "A", "B"];

// Standard tuning MIDI notes for open strings (string 6 to string 1)
// E2=40, A2=45, D3=50, G3=55, B3=59, E4=64
const OPEN_STRING_MIDI: [u8; 6] = [40, 45, 50, 55, 59, 64];

const NUM_FRETS: usize = 23; // Frets 0-22

// Layout constants
const STRING_LABEL_WIDTH: f32 = 0.0; // No string labels
const FRET_WIDTH: f32 = 59.0; // 55 button + 4 spacing
const STRING_HEIGHT: f32 = 50.0; // Increased for spacing between notes and markers
const HEADER_HEIGHT: f32 = 24.0;
const ROW_SPACING: f32 = 4.0;

struct App {
    _output_stream: Option<OutputStream>,
    stream_handle: Option<rodio::OutputStreamHandle>,
}

#[derive(Debug, Clone)]
enum Message {
    NoteClicked(usize, usize), // (string_index, fret)
}

impl Default for App {
    fn default() -> Self {
        let (stream, handle) = OutputStream::try_default().ok().unzip();
        Self {
            _output_stream: stream,
            stream_handle: handle,
        }
    }
}

impl App {
    fn update(&mut self, message: Message) {
        match message {
            Message::NoteClicked(string_idx, fret) => {
                self.play_note(string_idx, fret);
            }
        }
    }

    fn play_note(&self, string_idx: usize, fret: usize) {
        if let Some(handle) = &self.stream_handle {
            let midi_note = OPEN_STRING_MIDI[string_idx] + fret as u8;
            let frequency = midi_to_frequency(midi_note);

            if let Ok(sink) = Sink::try_new(handle) {
                // Use Karplus-Strong for realistic plucked string sound
                let source = KarplusStrong::new(frequency, 1500).amplify(0.5);
                sink.append(source);
                sink.detach();
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let legend = self.view_legend();
        let fretboard = self.view_fretboard();

        container(
            column![legend, fretboard]
                .spacing(16)
                .padding(20)
                .width(Length::Fill),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|_theme| container::Style {
            background: Some(color!(0x1a1b26).into()),
            ..Default::default()
        })
        .into()
    }

    fn view_legend(&self) -> Element<'_, Message> {
        let root_sample = container(text("C").size(12).color(color!(0x1a1b26)))
            .padding(4)
            .style(|_| container::Style {
                background: Some(color!(0xff9e64).into()),
                border: iced::Border {
                    radius: 4.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            });

        let scale_sample = container(text("D").size(12).color(color!(0x1a1b26)))
            .padding(4)
            .style(|_| container::Style {
                background: Some(color!(0x7dcfff).into()),
                border: iced::Border {
                    radius: 4.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            });

        let other_sample = container(text("C#").size(12).color(color!(0xa9b1d6)))
            .padding(4)
            .style(|_| container::Style {
                background: Some(color!(0x414868).into()),
                border: iced::Border {
                    radius: 4.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            });

        row![
            root_sample,
            text("= Root (C)").size(14).color(color!(0xa9b1d6)),
            text("  ").size(14),
            scale_sample,
            text("= C Major Scale").size(14).color(color!(0xa9b1d6)),
            text("  ").size(14),
            other_sample,
            text("= Other notes").size(14).color(color!(0xa9b1d6)),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center)
        .into()
    }

    fn view_fretboard(&self) -> Element<'_, Message> {
        // Fret numbers header
        let mut fret_header: Vec<Element<Message>> = vec![];

        for fret in 0..NUM_FRETS {
            let fret_text = if fret == 0 {
                "Nut".to_string()
            } else {
                fret.to_string()
            };

            fret_header.push(
                container(text(fret_text).size(12).color(color!(0x7dcfff)))
                    .width(55)
                    .align_x(iced::Alignment::Center)
                    .into(),
            );
        }

        let header_row = Row::with_children(fret_header)
            .spacing(4)
            .height(HEADER_HEIGHT as u16)
            .align_y(iced::Alignment::Center);

        // String rows (from high E to low E for visual representation)
        let mut string_rows: Vec<Element<Message>> = vec![header_row.into()];

        for string_idx in (0..6).rev() {
            let string_row = self.view_string_row(string_idx);
            string_rows.push(string_row);
        }

        let buttons_layer = Column::with_children(string_rows)
            .spacing(4)
            .width(Length::Fill);

        // Calculate canvas size (account for spacing)
        let canvas_width = STRING_LABEL_WIDTH + FRET_WIDTH * NUM_FRETS as f32;
        let canvas_height = HEADER_HEIGHT + ROW_SPACING + STRING_HEIGHT * 6.0 + ROW_SPACING * 5.0;

        let fretboard_canvas: Canvas<FretboardCanvas, Message, Theme, Renderer> =
            canvas(FretboardCanvas)
                .width(canvas_width as u16)
                .height(canvas_height as u16);

        // Stack canvas behind buttons
        stack![fretboard_canvas, buttons_layer]
            .width(Length::Fill)
            .height(canvas_height as u16)
            .into()
    }

    fn view_string_row(&self, string_idx: usize) -> Element<'_, Message> {
        let mut fret_buttons: Vec<Element<Message>> = vec![];

        for fret in 0..NUM_FRETS {
            let note_btn = self.view_note_button(string_idx, fret);
            fret_buttons.push(note_btn);
        }

        Row::with_children(fret_buttons)
            .spacing(4)
            .height(STRING_HEIGHT as u16)
            .align_y(iced::Alignment::Center)
            .into()
    }

    fn view_note_button(&self, string_idx: usize, fret: usize) -> Element<'_, Message> {
        let note_name = get_note_name(string_idx, fret);
        let is_c_major = C_MAJOR_SCALE.contains(&note_name.as_str());
        let is_root = note_name == "C";
        let has_sharp = note_name.contains('#');

        // Circle size - larger for sharps to fit "F#" etc.
        let circle_size: f32 = if has_sharp { 36.0 } else { 32.0 };

        // Use translucent backgrounds (RGBA with alpha as f32 0.0-1.0)
        let (bg_color, text_color) = if is_root {
            (
                iced::Color::from_rgba8(0xff, 0x9e, 0x64, 0.85),
                color!(0x1a1b26),
            ) // Orange 85%
        } else if is_c_major {
            (
                iced::Color::from_rgba8(0x7d, 0xcf, 0xff, 0.60),
                color!(0x1a1b26),
            ) // Cyan 60% - more transparent
        } else {
            (
                iced::Color::from_rgba8(0x41, 0x48, 0x68, 0.70),
                color!(0xa9b1d6),
            ) // Dim 70%
        };

        let style = move |_theme: &Theme, status: button::Status| {
            let bg = match status {
                button::Status::Hovered | button::Status::Pressed => {
                    if is_root {
                        iced::Color::from_rgba8(0xff, 0xb3, 0x80, 0.95)
                    } else if is_c_major {
                        iced::Color::from_rgba8(0x9d, 0xd6, 0xff, 0.80) // More visible on hover
                    } else {
                        iced::Color::from_rgba8(0x56, 0x5f, 0x89, 0.85)
                    }
                }
                _ => bg_color,
            };

            button::Style {
                background: Some(bg.into()),
                text_color,
                border: iced::Border {
                    radius: (circle_size / 2.0).into(), // Circular
                    width: if fret == 0 { 2.0 } else { 0.0 },
                    color: color!(0x565f89),
                },
                ..button::Style::default()
            }
        };

        let circle_button = button(
            container(text(note_name).size(12).font(iced::Font {
                weight: iced::font::Weight::Bold,
                ..iced::Font::default()
            }))
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(iced::Alignment::Center)
            .align_y(iced::Alignment::Center),
        )
        .width(circle_size as u16)
        .height(circle_size as u16)
        .style(style)
        .on_press(Message::NoteClicked(string_idx, fret));

        // Center the circle within the fret width
        container(circle_button)
            .width(55)
            .align_x(iced::Alignment::Center)
            .into()
    }
}

/// Get the note name for a given string and fret
fn get_note_name(string_idx: usize, fret: usize) -> String {
    let midi_note = OPEN_STRING_MIDI[string_idx] + fret as u8;
    let note_idx = (midi_note % 12) as usize;
    CHROMATIC_NOTES[note_idx].to_string()
}

/// Convert MIDI note number to frequency in Hz
fn midi_to_frequency(midi_note: u8) -> f32 {
    440.0 * 2.0_f32.powf((midi_note as f32 - 69.0) / 12.0)
}

/// Karplus-Strong plucked string synthesis for realistic guitar sound
struct KarplusStrong {
    buffer: Vec<f32>, // Circular delay buffer
    index: usize,     // Current position in buffer
    sample_rate: u32,
    samples_remaining: usize, // For duration control
    decay: f32,               // Controls sustain length
}

impl KarplusStrong {
    fn new(frequency: f32, duration_ms: u64) -> Self {
        let sample_rate = 44100u32;
        let delay_samples = (sample_rate as f32 / frequency).round() as usize;
        let total_samples = (sample_rate as u64 * duration_ms / 1000) as usize;

        // Fill buffer with white noise (-1.0 to 1.0)
        let mut rng = rand::thread_rng();
        let buffer: Vec<f32> = (0..delay_samples)
            .map(|_| rng.gen::<f32>() * 2.0 - 1.0)
            .collect();

        Self {
            buffer,
            index: 0,
            sample_rate,
            samples_remaining: total_samples,
            decay: 0.999, // Good guitar-like sustain
        }
    }
}

impl Iterator for KarplusStrong {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        if self.samples_remaining == 0 {
            return None;
        }
        self.samples_remaining -= 1;

        // Get current sample from buffer
        let current = self.buffer[self.index];

        // Low-pass filter: average with next sample
        let next_idx = (self.index + 1) % self.buffer.len();
        let filtered = (current + self.buffer[next_idx]) * 0.5 * self.decay;

        // Feed filtered sample back into buffer
        self.buffer[self.index] = filtered;

        // Advance index
        self.index = (self.index + 1) % self.buffer.len();

        Some(current)
    }
}

impl Source for KarplusStrong {
    fn current_frame_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> u16 {
        1
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

/// Canvas for drawing fretboard strings and frets
struct FretboardCanvas;

impl canvas::Program<Message> for FretboardCanvas {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<canvas::Geometry<Renderer>> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());

        let fretboard_x = STRING_LABEL_WIDTH;
        let fretboard_y = HEADER_HEIGHT + ROW_SPACING; // Account for spacing after header
        let fretboard_width = FRET_WIDTH * NUM_FRETS as f32;
        let fretboard_height = STRING_HEIGHT * 6.0 + ROW_SPACING * 5.0; // Include spacing between rows

        // Draw fretboard background (wood color)
        frame.fill_rectangle(
            Point::new(fretboard_x, fretboard_y),
            Size::new(fretboard_width, fretboard_height),
            canvas::Fill::from(iced::Color::from_rgb8(0x3d, 0x2b, 0x1f)), // Dark wood
        );

        // Draw nut (thick bar at fret 0)
        frame.fill_rectangle(
            Point::new(fretboard_x + FRET_WIDTH - 4.0, fretboard_y),
            Size::new(6.0, fretboard_height),
            canvas::Fill::from(iced::Color::from_rgb8(0xf5, 0xf5, 0xdc)), // Bone/ivory color
        );

        // Draw frets (vertical lines)
        for fret in 1..NUM_FRETS {
            let x = fretboard_x + (fret as f32 + 1.0) * FRET_WIDTH - 2.0;
            frame.fill_rectangle(
                Point::new(x, fretboard_y),
                Size::new(3.0, fretboard_height),
                canvas::Fill::from(iced::Color::from_rgb8(0xc0, 0xc0, 0xc0)), // Silver frets
            );
        }

        // Draw fret markers (single dots at 3, 5, 7, 9, 15, 17, 19, 21 and double at 12)
        let marker_color = iced::Color::from_rgb8(0xf5, 0xf5, 0xdc);
        let marker_y = fretboard_y + fretboard_height / 2.0;

        for &fret in &[3, 5, 7, 9, 15, 17, 19, 21] {
            let x = fretboard_x + (fret as f32 + 0.5) * FRET_WIDTH;
            frame.fill(
                &canvas::Path::circle(Point::new(x, marker_y), 6.0),
                canvas::Fill::from(marker_color),
            );
        }

        // Double dot at 12th fret - positioned between string rows (B-G and D-A gaps)
        let x12 = fretboard_x + 12.5 * FRET_WIDTH;
        // Gap between row 1 (B) and row 2 (G): at the ROW_SPACING boundary
        let dot1_y =
            fretboard_y + 1.0 * (STRING_HEIGHT + ROW_SPACING) + STRING_HEIGHT + ROW_SPACING / 2.0;
        // Gap between row 3 (D) and row 4 (A)
        let dot2_y =
            fretboard_y + 3.0 * (STRING_HEIGHT + ROW_SPACING) + STRING_HEIGHT + ROW_SPACING / 2.0;
        frame.fill(
            &canvas::Path::circle(Point::new(x12, dot1_y), 5.0),
            canvas::Fill::from(marker_color),
        );
        frame.fill(
            &canvas::Path::circle(Point::new(x12, dot2_y), 5.0),
            canvas::Fill::from(marker_color),
        );

        // Draw strings (horizontal lines) - thicker for bass strings
        let string_thicknesses = [3.0, 2.5, 2.0, 1.5, 1.2, 1.0]; // E A D G B e
        let string_colors = [
            iced::Color::from_rgb8(0xcd, 0x7f, 0x32), // Bronze for wound strings
            iced::Color::from_rgb8(0xcd, 0x7f, 0x32),
            iced::Color::from_rgb8(0xcd, 0x7f, 0x32),
            iced::Color::from_rgb8(0xcd, 0x7f, 0x32),
            iced::Color::from_rgb8(0xe8, 0xe8, 0xe8), // Steel for plain strings
            iced::Color::from_rgb8(0xe8, 0xe8, 0xe8),
        ];

        for string_idx in 0..6 {
            // Strings are displayed high to low (reversed)
            let display_idx = 5 - string_idx;
            // Account for spacing between rows
            let y = fretboard_y
                + display_idx as f32 * (STRING_HEIGHT + ROW_SPACING)
                + STRING_HEIGHT / 2.0;
            let thickness = string_thicknesses[string_idx];
            let color = string_colors[string_idx];

            frame.fill_rectangle(
                Point::new(fretboard_x, y - thickness / 2.0),
                Size::new(fretboard_width, thickness),
                canvas::Fill::from(color),
            );
        }

        vec![frame.into_geometry()]
    }
}
