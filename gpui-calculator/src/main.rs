use anyhow::Result;
use gpui::*;

mod calculator;

use calculator::Calculator;

fn main() -> Result<()> {
    let app = Application::new();

    app.run(move |cx| {
        let bounds = Bounds::centered(None, size(px(340.), px(480.)), cx);

        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_window, cx| cx.new(|_cx| CalculatorApp::new()),
        )
        .expect("Failed to open window");
    });

    Ok(())
}

// Colors
const BG_DARK: u32 = 0x1e1e1e;
const BG_DISPLAY: u32 = 0x2d2d2d;
const BG_BUTTON: u32 = 0x3c3c3c;
const BG_BUTTON_HOVER: u32 = 0x4a4a4a;
const BG_BUTTON_OP: u32 = 0x505050;
const BG_BUTTON_ENTER: u32 = 0x4fc3f7;
const TEXT_PRIMARY: u32 = 0xffffff;
const TEXT_SECONDARY: u32 = 0xcccccc;
const TEXT_RESULT: u32 = 0x4fc3f7;
const TEXT_ERROR: u32 = 0xff5252;

struct CalculatorApp {
    input: String,
    result: Option<String>,
    error: Option<String>,
    engine: Calculator,
}

impl CalculatorApp {
    fn new() -> Self {
        Self {
            input: String::new(),
            result: None,
            error: None,
            engine: Calculator::new(),
        }
    }

    fn append(&mut self, text: &str) {
        self.input.push_str(text);
        self.error = None;
    }

    fn backspace(&mut self) {
        self.input.pop();
        self.error = None;
    }

    fn clear(&mut self) {
        self.input.clear();
        self.result = None;
        self.error = None;
    }

    fn toggle_sign(&mut self) {
        if self.input.starts_with('-') {
            self.input.remove(0);
        } else if !self.input.is_empty() {
            self.input.insert(0, '-');
        }
    }

    fn evaluate(&mut self) {
        if self.input.is_empty() {
            return;
        }

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

fn format_result(value: f64) -> String {
    if value.is_infinite() {
        "Infinity".to_string()
    } else if value.is_nan() {
        "NaN".to_string()
    } else if value.fract() == 0.0 && value.abs() < 1e15 {
        format!("{}", value as i64)
    } else {
        // Limit decimal places but remove trailing zeros
        let s = format!("{:.10}", value);
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

impl Render for CalculatorApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(rgb(BG_DARK))
            .p_4()
            .gap_3()
            .child(self.render_display())
            .child(self.render_keypad(cx))
    }
}

impl CalculatorApp {
    fn render_display(&self) -> impl IntoElement {
        let display_text = if self.input.is_empty() {
            "0".to_string()
        } else {
            self.input.clone()
        };

        let result_text = self.error.clone().or_else(|| self.result.clone());
        let result_color = if self.error.is_some() {
            TEXT_ERROR
        } else {
            TEXT_RESULT
        };

        div()
            .flex()
            .flex_col()
            .p_4()
            .bg(rgb(BG_DISPLAY))
            .rounded_lg()
            .min_h(px(80.))
            .justify_end()
            .child(
                div()
                    .text_color(rgb(TEXT_SECONDARY))
                    .text_xl()
                    .text_right()
                    .overflow_x_hidden()
                    .child(display_text),
            )
            .child(
                div()
                    .text_color(rgb(result_color))
                    .text_2xl()
                    .font_weight(FontWeight::BOLD)
                    .text_right()
                    .min_h(px(32.))
                    .child(result_text.unwrap_or_default()),
            )
    }

    fn render_keypad(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let buttons = vec![
            vec![
                ("C", ButtonType::Special),
                ("(", ButtonType::Normal),
                (")", ButtonType::Normal),
                ("%", ButtonType::Operator),
                ("⌫", ButtonType::Special),
            ],
            vec![
                ("7", ButtonType::Normal),
                ("8", ButtonType::Normal),
                ("9", ButtonType::Normal),
                ("/", ButtonType::Operator),
                ("π", ButtonType::Operator),
            ],
            vec![
                ("4", ButtonType::Normal),
                ("5", ButtonType::Normal),
                ("6", ButtonType::Normal),
                ("*", ButtonType::Operator),
                ("e", ButtonType::Operator),
            ],
            vec![
                ("1", ButtonType::Normal),
                ("2", ButtonType::Normal),
                ("3", ButtonType::Normal),
                ("-", ButtonType::Operator),
                ("^", ButtonType::Operator),
            ],
            vec![
                ("0", ButtonType::Normal),
                (".", ButtonType::Normal),
                ("±", ButtonType::Special),
                ("+", ButtonType::Operator),
                ("=", ButtonType::Enter),
            ],
        ];

        div()
            .flex()
            .flex_col()
            .gap_2()
            .flex_1()
            .children(buttons.into_iter().map(|row| {
                div().flex().gap_2().flex_1().children(
                    row.into_iter()
                        .map(|(label, btn_type)| self.render_button(label, btn_type, cx)),
                )
            }))
    }

    fn render_button(
        &mut self,
        label: &str,
        btn_type: ButtonType,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let label_owned = label.to_string();
        let bg_color = match btn_type {
            ButtonType::Normal => BG_BUTTON,
            ButtonType::Operator => BG_BUTTON_OP,
            ButtonType::Special => BG_BUTTON_OP,
            ButtonType::Enter => BG_BUTTON_ENTER,
        };

        let text_color = if btn_type == ButtonType::Enter {
            BG_DARK
        } else {
            TEXT_PRIMARY
        };

        if label.is_empty() {
            // Empty placeholder
            return div().flex_1().min_h(px(48.)).into_any_element();
        }

        let display_label = match label {
            "π" => "pi".to_string(),
            other => other.to_string(),
        };

        div()
            .id(SharedString::from(format!("btn_{}", label)))
            .flex_1()
            .min_h(px(48.))
            .bg(rgb(bg_color))
            .hover(|style| style.bg(rgb(BG_BUTTON_HOVER)))
            .rounded_md()
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .text_lg()
            .text_color(rgb(text_color))
            .font_weight(FontWeight::MEDIUM)
            .child(display_label)
            .on_click(cx.listener(move |this, _event, _window, cx| {
                match label_owned.as_str() {
                    "C" => this.clear(),
                    "⌫" => this.backspace(),
                    "=" => this.evaluate(),
                    "±" => this.toggle_sign(),
                    "π" => this.append("pi"),
                    other => this.append(other),
                }
                cx.notify();
            }))
            .into_any_element()
    }
}

#[derive(Clone, Copy, PartialEq)]
enum ButtonType {
    Normal,
    Operator,
    Special,
    Enter,
}
