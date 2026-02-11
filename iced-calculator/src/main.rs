use iced::widget::{button, column, container, text, Column, Row};
use iced::{color, Element, Length, Theme};

mod calculator;

use calculator::Calculator;

fn main() -> iced::Result {
    iced::application("Iced Calculator", App::update, App::view)
        .theme(|_| Theme::TokyoNightStorm)
        .window_size((340.0, 480.0))
        .run()
}

#[derive(Default)]
struct App {
    input: String,
    result: Option<String>,
    error: Option<String>,
    engine: Calculator,
}

#[derive(Debug, Clone)]
enum Message {
    ButtonPressed(String),
    Clear,
    Backspace,
    Evaluate,
    ToggleSign,
}

impl App {
    fn update(&mut self, message: Message) {
        match message {
            Message::ButtonPressed(s) => {
                if s == "pi" {
                    self.input.push_str("pi");
                } else {
                    self.input.push_str(&s);
                }
                self.error = None;
            }
            Message::Clear => {
                self.input.clear();
                self.result = None;
                self.error = None;
            }
            Message::Backspace => {
                self.input.pop();
                self.error = None;
            }
            Message::Evaluate => {
                if !self.input.is_empty() {
                    match self.engine.evaluate(&self.input) {
                        Ok(value) => {
                            self.result = Some(format_result(value));
                            self.error = None;
                        }
                        Err(msg) => {
                            self.error = Some(msg);
                            self.result = None;
                        }
                    }
                }
            }
            Message::ToggleSign => {
                if self.input.starts_with('-') {
                    self.input.remove(0);
                } else if !self.input.is_empty() {
                    self.input.insert(0, '-');
                }
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let display = self.view_display();
        let keypad = self.view_keypad();

        container(
            column![display, keypad]
                .spacing(12)
                .padding(16)
                .width(Length::Fill),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }

    fn view_display(&self) -> Element<'_, Message> {
        let display_text = if self.input.is_empty() {
            "0".to_string()
        } else {
            self.input.clone()
        };

        let result_text = self
            .error
            .clone()
            .or_else(|| self.result.clone())
            .unwrap_or_default();

        let result_color = if self.error.is_some() {
            color!(0xff5252)
        } else {
            color!(0x4fc3f7)
        };

        container(
            column![
                text(display_text).size(24).color(color!(0xcccccc)),
                text(result_text).size(32).color(result_color),
            ]
            .spacing(4)
            .width(Length::Fill)
            .align_x(iced::Alignment::End),
        )
        .padding(16)
        .width(Length::Fill)
        .style(|_theme| container::Style {
            background: Some(color!(0x2d2d2d).into()),
            border: iced::Border {
                radius: 8.0.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .into()
    }

    fn view_keypad(&self) -> Element<'_, Message> {
        let buttons: Vec<Vec<(&str, ButtonKind)>> = vec![
            vec![
                ("C", ButtonKind::Special),
                ("(", ButtonKind::Normal),
                (")", ButtonKind::Normal),
                ("%", ButtonKind::Operator),
                ("⌫", ButtonKind::Special),
            ],
            vec![
                ("7", ButtonKind::Normal),
                ("8", ButtonKind::Normal),
                ("9", ButtonKind::Normal),
                ("/", ButtonKind::Operator),
                ("π", ButtonKind::Operator),
            ],
            vec![
                ("4", ButtonKind::Normal),
                ("5", ButtonKind::Normal),
                ("6", ButtonKind::Normal),
                ("*", ButtonKind::Operator),
                ("e", ButtonKind::Operator),
            ],
            vec![
                ("1", ButtonKind::Normal),
                ("2", ButtonKind::Normal),
                ("3", ButtonKind::Normal),
                ("-", ButtonKind::Operator),
                ("^", ButtonKind::Operator),
            ],
            vec![
                ("0", ButtonKind::Normal),
                (".", ButtonKind::Normal),
                ("±", ButtonKind::Special),
                ("+", ButtonKind::Operator),
                ("=", ButtonKind::Enter),
            ],
        ];

        let rows: Vec<Element<Message>> = buttons
            .into_iter()
            .map(|row_buttons| {
                let btns: Vec<Element<Message>> = row_buttons
                    .into_iter()
                    .map(|(label, kind)| self.view_button(label, kind))
                    .collect();
                Row::with_children(btns)
                    .spacing(8)
                    .width(Length::Fill)
                    .into()
            })
            .collect();

        Column::with_children(rows)
            .spacing(8)
            .width(Length::Fill)
            .into()
    }

    fn view_button(&self, label: &str, kind: ButtonKind) -> Element<'_, Message> {
        let display_label = match label {
            "π" => "pi".to_string(),
            other => other.to_string(),
        };

        let message = match label {
            "C" => Message::Clear,
            "⌫" => Message::Backspace,
            "=" => Message::Evaluate,
            "±" => Message::ToggleSign,
            "π" => Message::ButtonPressed("pi".to_string()),
            other => Message::ButtonPressed(other.to_string()),
        };

        let style = move |_theme: &Theme, status: button::Status| {
            let base_bg = match kind {
                ButtonKind::Normal => color!(0x3c3c3c),
                ButtonKind::Operator => color!(0x505050),
                ButtonKind::Special => color!(0x505050),
                ButtonKind::Enter => color!(0x4fc3f7),
            };

            let bg = match status {
                button::Status::Hovered | button::Status::Pressed => color!(0x5a5a5a),
                _ => base_bg,
            };

            let text_color = if kind == ButtonKind::Enter {
                color!(0x1e1e1e)
            } else {
                color!(0xffffff)
            };

            button::Style {
                background: Some(bg.into()),
                text_color,
                border: iced::Border {
                    radius: 6.0.into(),
                    ..Default::default()
                },
                ..button::Style::default()
            }
        };

        button(
            text(display_label)
                .size(20)
                .width(Length::Fill)
                .align_x(iced::Alignment::Center),
        )
        .width(Length::Fill)
        .height(48)
        .style(style)
        .on_press(message)
        .into()
    }
}

#[derive(Clone, Copy, PartialEq)]
enum ButtonKind {
    Normal,
    Operator,
    Special,
    Enter,
}

fn format_result(value: f64) -> String {
    if value.is_infinite() {
        "Infinity".to_string()
    } else if value.is_nan() {
        "NaN".to_string()
    } else if value.fract() == 0.0 && value.abs() < 1e15 {
        format!("{}", value as i64)
    } else {
        let s = format!("{:.10}", value);
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}
