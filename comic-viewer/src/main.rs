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
/// Single-page mode: 1 visible + 2 ahead + 2 behind + 2 buffer.
const CACHE_CAPACITY_SINGLE: usize = 7;
/// Double-page mode: 2 visible + 4 ahead + 4 behind + 2 buffer.
const CACHE_CAPACITY_DOUBLE: usize = 12;
/// Pages to preload in each direction in single-page mode.
const PRELOAD_LOOKAHEAD_SINGLE: usize = 2;
/// Pages to preload in each direction in double-page mode (covers 2 full spreads).
const PRELOAD_LOOKAHEAD_DOUBLE: usize = 4;
/// Minimum logical-pixel width required for each page in double-page mode.
/// Double mode activates when the window can provide at least this much width
/// to both pages simultaneously (i.e. window_width ≥ 2 × MIN_PAGE_WIDTH).
const MIN_PAGE_WIDTH: f32 = 400.0;

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
    /// `current_page` if the cache is already full (up to `capacity` entries).
    fn insert(&mut self, index: usize, handle: image::Handle, current_page: usize, capacity: usize) {
        if !self.entries.contains_key(&index) && self.entries.len() >= capacity {
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
// Layout mode / page flow
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
enum LayoutMode {
    Single,
    /// Show current page and current+1 side by side.
    Double,
}

impl LayoutMode {
    fn is_double(self) -> bool {
        matches!(self, Self::Double)
    }

    /// Pages advanced/retreated per navigation action.
    fn nav_step(self) -> usize {
        match self {
            Self::Single => 1,
            Self::Double => 2,
        }
    }

    /// Pages to preload in each direction from the current position.
    fn preload_lookahead(self) -> usize {
        match self {
            Self::Single => PRELOAD_LOOKAHEAD_SINGLE,
            Self::Double => PRELOAD_LOOKAHEAD_DOUBLE,
        }
    }

    /// Maximum number of decoded page handles to keep resident in memory.
    fn cache_capacity(self) -> usize {
        match self {
            Self::Single => CACHE_CAPACITY_SINGLE,
            Self::Double => CACHE_CAPACITY_DOUBLE,
        }
    }
}

/// Reading direction.
///
/// - `LeftToRight`: western comics — earlier page on the left.
/// - `RightToLeft`: manga — earlier page on the right.
///   Also reverses the Left/Right arrow key semantics so that pressing
///   Left (the natural "forward" direction in RTL) advances to the next page.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
enum PageFlow {
    LeftToRight,
    #[default]
    RightToLeft,
}

impl PageFlow {
    /// Label shown on the toggle button in the header.
    fn flow_label(self) -> &'static str {
        match self {
            Self::RightToLeft => "◁ RTL",
            Self::LeftToRight => "▷ LTR",
        }
    }

    /// `(prev_label, next_label)` for the navigation buttons.
    fn nav_labels(self) -> (&'static str, &'static str) {
        match self {
            Self::LeftToRight => ("◄ Previous", "Next ►"),
            Self::RightToLeft => ("Previous ►", "◄ Next"),
        }
    }

    /// Whether pressing the Left arrow / clicking the leftmost nav button
    /// should advance to the *next* page (true in RTL/manga).
    fn left_is_next(self) -> bool {
        matches!(self, Self::RightToLeft)
    }

    /// Whether the earlier (lower-index) page should appear on the *right*
    /// in double-page mode (true in RTL/manga).
    fn earlier_page_on_right(self) -> bool {
        matches!(self, Self::RightToLeft)
    }
}

// ---------------------------------------------------------------------------
// Application state
// ---------------------------------------------------------------------------

struct App {
    comic: Option<Arc<dyn reader::ComicReader>>,
    current_page: usize,
    current_handle: Option<image::Handle>,
    page_cache: PageCache,
    loading: bool,
    error: Option<String>,
    window_width: f32,
    window_height: f32,
    page_flow: PageFlow,
}

impl Default for App {
    fn default() -> Self {
        Self {
            comic: None,
            current_page: 0,
            current_handle: None,
            page_cache: PageCache::default(),
            loading: false,
            error: None,
            window_width: 900.0,
            window_height: 700.0,
            page_flow: PageFlow::default(),
        }
    }
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
    WindowResized(f32, f32),
    /// Raw horizontal key — resolved to Next/Prev based on `page_flow`.
    LeftKey,
    RightKey,
    ToggleFlow,
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
                let page_count = self.comic.as_ref().map_or(0, |c| c.page_count());
                let step = self.layout_mode().nav_step();
                if self.current_page + step < page_count {
                    self.navigate_to(self.current_page + step)
                } else {
                    Task::none()
                }
            }
            Message::PrevPage => {
                let step = self.layout_mode().nav_step();
                let target = if self.current_page > step {
                    self.current_page - step
                } else if self.current_page > 0 {
                    0
                } else {
                    return Task::none();
                };
                self.navigate_to(target)
            }
            Message::WindowResized(w, h) => {
                self.window_width = w;
                self.window_height = h;
                Task::none()
            }
            Message::LeftKey => self.update(if self.page_flow.left_is_next() {
                Message::NextPage
            } else {
                Message::PrevPage
            }),
            Message::RightKey => self.update(if self.page_flow.left_is_next() {
                Message::PrevPage
            } else {
                Message::NextPage
            }),
            Message::ToggleFlow => {
                self.page_flow = match self.page_flow {
                    PageFlow::LeftToRight => PageFlow::RightToLeft,
                    PageFlow::RightToLeft => PageFlow::LeftToRight,
                };
                Task::none()
            }
            Message::PagePreloaded(index, Some(handle)) => {
                self.page_cache.insert(index, handle, self.current_page, self.cache_capacity());
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
            self.page_cache.insert(page, h.clone(), page, self.cache_capacity());
            self.current_handle = Some(h);
        }

        // In Double mode, page+1 must be ready before the first paint or the
        // view briefly flashes single-page while the background preload lands.
        // Fetch it synchronously here (cache hit = free; extract = fast).
        if self.layout_mode().is_double() {
            let next = page + 1;
            if !self.page_cache.contains(next) {
                let maybe = self.comic.as_ref().and_then(|c| c.extract_page(next).ok());
                if let Some(h) = maybe {
                    self.page_cache.insert(next, h, page, self.cache_capacity());
                }
            }
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

        let lookahead = self.preload_lookahead();
        let start = around.saturating_sub(lookahead);
        let end = (around + lookahead).min(page_count - 1);

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

    /// Choose between single and dual-page layout based on window dimensions.
    ///
    /// Rules:
    /// - Cover page (index 0) is always single.
    /// - Last page when total count is odd is always single (no next page).
    /// - Double only when the window is wide enough to give each page at least
    ///   `MIN_PAGE_WIDTH` logical pixels (window_width ≥ 2 × MIN_PAGE_WIDTH).
    fn layout_mode(&self) -> LayoutMode {
        let Some(comic) = &self.comic else {
            return LayoutMode::Single;
        };
        let is_cover = self.current_page == 0;
        let has_next = self.current_page + 1 < comic.page_count();
        if !is_cover && has_next && self.window_width >= 2.0 * MIN_PAGE_WIDTH {
            LayoutMode::Double
        } else {
            LayoutMode::Single
        }
    }

    fn preload_lookahead(&self) -> usize {
        self.layout_mode().preload_lookahead()
    }

    fn cache_capacity(&self) -> usize {
        self.layout_mode().cache_capacity()
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
            if self.layout_mode().is_double() {
                format!(
                    "{}-{} / {}",
                    self.current_page + 1,
                    (self.current_page + 2).min(c.page_count()),
                    c.page_count()
                )
            } else {
                format!("{} / {}", self.current_page + 1, c.page_count())
            }
        });

        let flow_label = self.page_flow.flow_label();
        let flow_btn = button(text(flow_label).size(14))
            .on_press(Message::ToggleFlow)
            .style(nav_button_style);

        container(
            row![
                open_btn,
                flow_btn,
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
            if self.layout_mode().is_double() {
                // Try to serve page N+1 from cache (preloaded in background).
                // Falls back to single if the preload hasn't arrived yet.
                if let Some(next) = self.page_cache.get(self.current_page + 1) {
                    let page_a = image(handle.clone())
                        .content_fit(ContentFit::Contain)
                        .width(Length::Fill)
                        .height(Length::Fill);
                    let page_b = image(next)
                        .content_fit(ContentFit::Contain)
                        .width(Length::Fill)
                        .height(Length::Fill);
                    let pages: Element<'_, Message> = if self.page_flow.earlier_page_on_right() {
                        row![page_b, page_a]
                    } else {
                        row![page_a, page_b]
                    }
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .into();
                    return container(pages)
                        .width(Length::Fill)
                        .height(Length::Fill)
                        .into();
                }
            }
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

        let (prev_label, next_label) = self.page_flow.nav_labels();

        let prev_btn = button(text(prev_label).size(14))
            .on_press_maybe((has_comic && can_prev).then_some(Message::PrevPage))
            .style(nav_button_style);

        let next_btn = button(text(next_label).size(14))
            .on_press_maybe((has_comic && can_next).then_some(Message::NextPage))
            .style(nav_button_style);

        let nav_row = if self.page_flow.left_is_next() {
            row![next_btn, prev_btn]
        } else {
            row![prev_btn, next_btn]
        }
        .spacing(16)
        .align_y(iced::Alignment::Center);

        container(nav_row)
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
        Subscription::batch([
            iced::keyboard::on_key_press(|key, _modifiers| match key.as_ref() {
                iced::keyboard::Key::Named(Named::ArrowLeft) => Some(Message::LeftKey),
                iced::keyboard::Key::Named(Named::ArrowRight) => Some(Message::RightKey),
                iced::keyboard::Key::Named(Named::ArrowDown)
                | iced::keyboard::Key::Named(Named::PageDown) => Some(Message::NextPage),
                iced::keyboard::Key::Named(Named::ArrowUp)
                | iced::keyboard::Key::Named(Named::PageUp) => Some(Message::PrevPage),
                _ => None,
            }),
            iced::event::listen_with(|event, _status, _window| {
                if let iced::Event::Window(iced::window::Event::Resized(size)) = event {
                    Some(Message::WindowResized(size.width, size.height))
                } else {
                    None
                }
            }),
        ])
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
