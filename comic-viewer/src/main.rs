mod reader;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use iced::keyboard::key::Named;
use iced::widget::{button, column, container, image, row, text};
use iced::{ContentFit, Element, Length, Subscription, Task, Theme};

fn main() -> iced::Result {
    iced::application("Comic Viewer", App::update, App::view)
        .theme(|_| Theme::TokyoNightStorm)
        .window_size((900.0, 700.0))
        .subscription(App::subscription)
        .run()
}

// ---------------------------------------------------------------------------
// Page cache
// ---------------------------------------------------------------------------

/// LRU-by-distance cache for decoded page handles.
///
/// Stores up to `CAPACITY` entries. On overflow the entry whose page index is
/// furthest from the current page is evicted, keeping recently-visited and
/// soon-to-be-visited pages hot.
///
/// Holding a `Handle` also keeps iced's internal GPU texture alive, so cached
/// pages re-render instantly without a re-decode.
const CACHE_CAPACITY: usize = 7;
/// Pages to preload in each direction from the current page.
const PRELOAD_LOOKAHEAD: usize = 2;

#[derive(Default)]
struct PageCache {
    entries: HashMap<usize, image::Handle>,
}

impl PageCache {
    /// Return a clone of the cached handle for `index`, if present.
    fn get(&self, index: usize) -> Option<image::Handle> {
        self.entries.get(&index).cloned()
    }

    fn contains(&self, index: usize) -> bool {
        self.entries.contains_key(&index)
    }

    /// Insert `handle` for `index`, evicting the entry furthest from
    /// `current_page` if the cache is already full.
    fn insert(&mut self, index: usize, handle: image::Handle, current_page: usize) {
        if !self.entries.contains_key(&index) && self.entries.len() >= CACHE_CAPACITY {
            // usize is Copy, so we can copy the key out before the mutable borrow.
            let evict = *self
                .entries
                .keys()
                .max_by_key(|&&k| k.abs_diff(current_page))
                .expect("cache is non-empty");
            self.entries.remove(&evict);
        }
        self.entries.insert(index, handle);
    }

    fn clear(&mut self) {
        self.entries.clear();
    }
}

// ---------------------------------------------------------------------------
// Application state
// ---------------------------------------------------------------------------

#[derive(Default)]
struct App {
    comic: Option<Arc<dyn reader::ComicReader>>,
    current_page: usize,
    current_handle: Option<image::Handle>,
    page_cache: PageCache,
    loading: bool,
    error: Option<String>,
}

#[derive(Debug, Clone)]
enum Message {
    OpenFile,
    FileSelected(Option<PathBuf>),
    ComicLoaded(Result<Arc<dyn reader::ComicReader>, String>),
    NextPage,
    PrevPage,
    /// A background preload completed. `None` means extraction failed (ignored).
    PagePreloaded(usize, Option<image::Handle>),
}

// ---------------------------------------------------------------------------
// Update
// ---------------------------------------------------------------------------

impl App {
    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::OpenFile => {
                self.loading = true;
                self.error = None;
                Task::perform(
                    async {
                        rfd::AsyncFileDialog::new()
                            .add_filter("Comic Book Archive", &["cbz", "cbr", "cb7"])
                            .pick_file()
                            .await
                            .map(|f| f.path().to_owned())
                    },
                    Message::FileSelected,
                )
            }
            Message::FileSelected(Some(path)) => Task::perform(
                async move { smol::unblock(move || reader::open(&path).map(Arc::from)).await },
                Message::ComicLoaded,
            ),
            Message::FileSelected(None) => {
                self.loading = false;
                Task::none()
            }
            Message::ComicLoaded(Ok(comic)) => {
                self.loading = false;
                self.error = None;
                self.page_cache.clear();
                self.comic = Some(comic);
                // navigate_to sets current_page, fills current_handle, and
                // spawns a background preload for page 1.
                self.navigate_to(0)
            }
            Message::ComicLoaded(Err(e)) => {
                self.loading = false;
                self.error = Some(e);
                Task::none()
            }
            Message::NextPage => {
                let can_advance = self
                    .comic
                    .as_ref()
                    .is_some_and(|c| self.current_page + 1 < c.page_count());
                if can_advance {
                    self.navigate_to(self.current_page + 1)
                } else {
                    Task::none()
                }
            }
            Message::PrevPage => {
                if self.current_page > 0 {
                    self.navigate_to(self.current_page - 1)
                } else {
                    Task::none()
                }
            }
            Message::PagePreloaded(index, Some(handle)) => {
                self.page_cache.insert(index, handle, self.current_page);
                Task::none()
            }
            Message::PagePreloaded(_, None) => Task::none(),
        }
    }

    /// Switch to `page`, serving from the cache when possible, then launch
    /// background preloads for the immediately adjacent pages.
    fn navigate_to(&mut self, page: usize) -> Task<Message> {
        self.current_page = page;

        // Cache hit → clone the handle (cheap: `Bytes` is Arc-backed).
        // Cache miss → extract synchronously (in-memory, fast).
        let handle = if let Some(h) = self.page_cache.get(page) {
            Some(h)
        } else if let Some(comic) = &self.comic {
            comic.extract_page(page).ok()
        } else {
            None
        };

        if let Some(h) = handle {
            self.page_cache.insert(page, h.clone(), page);
            self.current_handle = Some(h);
        }

        self.preload_adjacent(page)
    }

    /// Spawn background `Task`s to extract up to `PRELOAD_LOOKAHEAD` pages in
    /// each direction from `around`, skipping any already in the cache.
    /// Each task offloads its synchronous extraction to smol's blocking thread
    /// pool via `smol::unblock`, keeping iced's async executor threads free.
    fn preload_adjacent(&self, around: usize) -> Task<Message> {
        let Some(comic) = &self.comic else {
            return Task::none();
        };
        let page_count = comic.page_count();
        if page_count == 0 {
            return Task::none();
        }

        let start = around.saturating_sub(PRELOAD_LOOKAHEAD);
        let end = (around + PRELOAD_LOOKAHEAD).min(page_count - 1);

        let candidates: Vec<usize> = (start..=end)
            .filter(|&i| i != around && !self.page_cache.contains(i))
            .collect();

        if candidates.is_empty() {
            return Task::none();
        }

        let tasks: Vec<Task<Message>> = candidates
            .into_iter()
            .map(|idx| {
                let comic = Arc::clone(comic);
                Task::perform(
                    async move {
                        let handle = smol::unblock(move || comic.extract_page(idx).ok()).await;
                        (idx, handle)
                    },
                    |(idx, handle)| Message::PagePreloaded(idx, handle),
                )
            })
            .collect();

        Task::batch(tasks)
    }

    // -----------------------------------------------------------------------
    // View
    // -----------------------------------------------------------------------

    fn view(&self) -> Element<'_, Message> {
        let header = self.view_header();
        let content = self.view_content();
        let nav = self.view_nav();

        container(
            column![header, content, nav]
                .width(Length::Fill)
                .height(Length::Fill),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|_theme| container::Style {
            background: Some(iced::color!(0x1a1b26).into()),
            ..Default::default()
        })
        .into()
    }

    fn view_header(&self) -> Element<'_, Message> {
        let open_btn = button(text("Open Comic").size(14))
            .on_press(Message::OpenFile)
            .style(|_theme, status| {
                let bg = match status {
                    button::Status::Hovered | button::Status::Pressed => iced::color!(0x565f89),
                    _ => iced::color!(0x414868),
                };
                button::Style {
                    background: Some(bg.into()),
                    text_color: iced::color!(0xc0caf5),
                    border: iced::Border {
                        radius: 4.0.into(),
                        ..Default::default()
                    },
                    ..button::Style::default()
                }
            });

        let title_str = self
            .comic
            .as_ref()
            .map_or_else(|| "No Comic Loaded".to_string(), |c| c.title().to_string());

        let page_str = self.comic.as_ref().map_or_else(String::new, |c| {
            format!("{} / {}", self.current_page + 1, c.page_count())
        });

        container(
            row![
                open_btn,
                container(text(title_str).size(16).color(iced::color!(0xc0caf5)))
                    .width(Length::Fill)
                    .align_x(iced::Alignment::Center),
                text(page_str).size(14).color(iced::color!(0x7dcfff)),
            ]
            .spacing(12)
            .align_y(iced::Alignment::Center)
            .padding(10),
        )
        .width(Length::Fill)
        .style(|_theme| container::Style {
            background: Some(iced::color!(0x16161e).into()),
            ..Default::default()
        })
        .into()
    }

    fn view_content(&self) -> Element<'_, Message> {
        let inner: Element<'_, Message> = if self.loading {
            container(text("Loading...").size(24).color(iced::color!(0xa9b1d6)))
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(iced::Alignment::Center)
                .align_y(iced::Alignment::Center)
                .into()
        } else if let Some(err) = &self.error {
            container(
                text(format!("Error: {err}"))
                    .size(16)
                    .color(iced::color!(0xf7768e)),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(iced::Alignment::Center)
            .align_y(iced::Alignment::Center)
            .into()
        } else if let Some(handle) = &self.current_handle {
            image(handle.clone())
                .content_fit(ContentFit::Contain)
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
        } else {
            container(
                text("Click \"Open Comic\" to begin")
                    .size(20)
                    .color(iced::color!(0x565f89)),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(iced::Alignment::Center)
            .align_y(iced::Alignment::Center)
            .into()
        };

        container(inner)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn view_nav(&self) -> Element<'_, Message> {
        let has_comic = self.comic.is_some();
        let can_prev = self.current_page > 0;
        let can_next = self
            .comic
            .as_ref()
            .is_some_and(|c| self.current_page + 1 < c.page_count());

        let prev_btn = button(text("◄ Previous").size(14))
            .on_press_maybe((has_comic && can_prev).then_some(Message::PrevPage))
            .style(nav_button_style);

        let next_btn = button(text("Next ►").size(14))
            .on_press_maybe((has_comic && can_next).then_some(Message::NextPage))
            .style(nav_button_style);

        container(
            row![prev_btn, next_btn]
                .spacing(16)
                .align_y(iced::Alignment::Center),
        )
        .width(Length::Fill)
        .align_x(iced::Alignment::Center)
        .padding(10)
        .style(|_theme| container::Style {
            background: Some(iced::color!(0x16161e).into()),
            ..Default::default()
        })
        .into()
    }

    fn subscription(&self) -> Subscription<Message> {
        iced::keyboard::on_key_press(|key, _modifiers| match key.as_ref() {
            iced::keyboard::Key::Named(Named::ArrowLeft)
            | iced::keyboard::Key::Named(Named::ArrowDown)
            | iced::keyboard::Key::Named(Named::PageDown) => Some(Message::NextPage),
            iced::keyboard::Key::Named(Named::ArrowRight)
            | iced::keyboard::Key::Named(Named::ArrowUp)
            | iced::keyboard::Key::Named(Named::PageUp) => Some(Message::PrevPage),
            _ => None,
        })
    }
}

fn nav_button_style(_theme: &Theme, status: button::Status) -> button::Style {
    let bg = match status {
        button::Status::Hovered | button::Status::Pressed => iced::color!(0x565f89),
        button::Status::Disabled => iced::color!(0x292e42),
        _ => iced::color!(0x414868),
    };
    let text_color = match status {
        button::Status::Disabled => iced::color!(0x3d4168),
        _ => iced::color!(0xc0caf5),
    };
    button::Style {
        background: Some(bg.into()),
        text_color,
        border: iced::Border {
            radius: 4.0.into(),
            ..Default::default()
        },
        ..button::Style::default()
    }
}
