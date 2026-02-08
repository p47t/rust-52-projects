use std::cell::RefCell;
use std::rc::Rc;

use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};

use crate::card::{Card, Deck, ReviewRating};
use crate::{sm2, storage, DeckInfo, DeckStats, MainWindow};

struct StudySession {
    deck_index: usize,
    due_cards: Vec<usize>,
    current_position: usize,
    cards_reviewed: u32,
}

struct AppState {
    decks: Vec<Deck>,
    current_session: Option<StudySession>,
    /// Which deck the editor is targeting
    editor_deck_index: Option<usize>,
    data_path: std::path::PathBuf,
}

fn make_deck_model(decks: &[Deck]) -> Rc<VecModel<DeckInfo>> {
    Rc::new(VecModel::from(
        decks
            .iter()
            .map(|d| {
                let due = sm2::due_cards(&d.cards).len() as i32;
                DeckInfo {
                    name: SharedString::from(&d.name),
                    description: SharedString::from(&d.description),
                    card_count: d.cards.len() as i32,
                    due_count: due,
                }
            })
            .collect::<Vec<_>>(),
    ))
}

fn refresh_deck_model(decks: &[Deck], model: &Rc<VecModel<DeckInfo>>) {
    // Replace all rows
    let new_items: Vec<DeckInfo> = decks
        .iter()
        .map(|d| {
            let due = sm2::due_cards(&d.cards).len() as i32;
            DeckInfo {
                name: SharedString::from(&d.name),
                description: SharedString::from(&d.description),
                card_count: d.cards.len() as i32,
                due_count: due,
            }
        })
        .collect();

    while model.row_count() > 0 {
        model.remove(0);
    }
    for item in new_items {
        model.push(item);
    }
}

fn compute_stats(deck: &Deck) -> DeckStats {
    let due = sm2::due_cards(&deck.cards).len() as i32;
    let total_reviews: u32 = deck.cards.iter().map(|c| c.total_reviews).sum();
    let correct_reviews: u32 = deck.cards.iter().map(|c| c.correct_reviews).sum();
    let avg_ease = if deck.cards.is_empty() {
        2.5
    } else {
        deck.cards.iter().map(|c| c.ease_factor).sum::<f64>() / deck.cards.len() as f64
    };

    DeckStats {
        total_cards: deck.cards.len() as i32,
        due_today: due,
        total_reviews: total_reviews as i32,
        correct_reviews: correct_reviews as i32,
        average_ease: avg_ease as f32,
    }
}

pub fn run() {
    let window = MainWindow::new().unwrap();

    let path = storage::data_path();
    let decks = storage::load_or_default(&path);

    let deck_model = make_deck_model(&decks);
    window.set_decks(ModelRc::from(deck_model.clone()));

    let state = Rc::new(RefCell::new(AppState {
        decks,
        current_session: None,
        editor_deck_index: None,
        data_path: path,
    }));

    // study-deck: start studying a deck
    {
        let window_weak = window.as_weak();
        let state = Rc::clone(&state);
        window.on_study_deck(move |deck_index| {
            let window = window_weak.unwrap();
            let mut st = state.borrow_mut();
            let idx = deck_index as usize;
            let due = sm2::due_cards(&st.decks[idx].cards);

            if due.is_empty() {
                return;
            }

            let first_card_idx = due[0];
            let card = &st.decks[idx].cards[first_card_idx];

            window.set_study_deck_name(SharedString::from(&st.decks[idx].name));
            window.set_card_front(SharedString::from(&card.front));
            window.set_card_back(SharedString::from(&card.back));
            window.set_card_revealed(false);
            window.set_cards_remaining(due.len() as i32 - 1);
            window.set_cards_reviewed(0);

            st.current_session = Some(StudySession {
                deck_index: idx,
                due_cards: due,
                current_position: 0,
                cards_reviewed: 0,
            });

            window.set_current_page(1);
        });
    }

    // reveal-card
    {
        let window_weak = window.as_weak();
        window.on_reveal_card(move || {
            let window = window_weak.unwrap();
            window.set_card_revealed(true);
        });
    }

    // rate-card: apply SM-2 and advance
    {
        let window_weak = window.as_weak();
        let state = Rc::clone(&state);
        let deck_model = deck_model.clone();
        window.on_rate_card(move |rating_int| {
            let window = window_weak.unwrap();
            let mut st = state.borrow_mut();

            let rating = ReviewRating::from_int(rating_int);

            // Extract session info to avoid borrow conflicts
            let (deck_idx, card_idx, _position, due_len) = {
                let session = match st.current_session.as_ref() {
                    Some(s) => s,
                    None => return,
                };
                (
                    session.deck_index,
                    session.due_cards[session.current_position],
                    session.current_position,
                    session.due_cards.len(),
                )
            };

            // Apply SM-2
            let result = sm2::review(&st.decks[deck_idx].cards[card_idx], rating);

            // Update card
            let card = &mut st.decks[deck_idx].cards[card_idx];
            card.ease_factor = result.new_ease_factor;
            card.interval = result.new_interval;
            card.repetition = result.new_repetition;
            card.next_review = result.next_review;
            card.last_reviewed = Some(chrono::Utc::now());
            card.total_reviews += 1;
            if rating != ReviewRating::Again {
                card.correct_reviews += 1;
            }

            // Advance session
            let session = st.current_session.as_mut().unwrap();
            session.cards_reviewed += 1;
            session.current_position += 1;
            let new_position = session.current_position;
            let cards_reviewed = session.cards_reviewed;

            if new_position < due_len {
                let next_idx = session.due_cards[new_position];
                let next_card = &st.decks[deck_idx].cards[next_idx];
                window.set_card_front(SharedString::from(&next_card.front));
                window.set_card_back(SharedString::from(&next_card.back));
                window.set_card_revealed(false);
                window.set_cards_remaining((due_len - new_position - 1) as i32);
                window.set_cards_reviewed(cards_reviewed as i32);
            } else {
                // Session complete
                storage::save(&st.decks, &st.data_path).ok();
                refresh_deck_model(&st.decks, &deck_model);
                st.current_session = None;
                window.set_current_page(0);
            }
        });
    }

    // add-card-to-deck: open editor for a new card
    {
        let window_weak = window.as_weak();
        let state = Rc::clone(&state);
        window.on_add_card_to_deck(move |deck_index| {
            let window = window_weak.unwrap();
            let mut st = state.borrow_mut();
            st.editor_deck_index = Some(deck_index as usize);
            window.set_editor_front(SharedString::default());
            window.set_editor_back(SharedString::default());
            window.set_editor_is_edit(false);
            window.set_current_page(2);
        });
    }

    // save-card
    {
        let window_weak = window.as_weak();
        let state = Rc::clone(&state);
        let deck_model = deck_model.clone();
        window.on_save_card(move |front, back| {
            let window = window_weak.unwrap();
            let mut st = state.borrow_mut();

            if front.is_empty() || back.is_empty() {
                return;
            }

            if let Some(deck_idx) = st.editor_deck_index {
                let card = Card::new(front.to_string(), back.to_string());
                st.decks[deck_idx].cards.push(card);
                storage::save(&st.decks, &st.data_path).ok();
                refresh_deck_model(&st.decks, &deck_model);
            }

            window.set_current_page(0);
        });
    }

    // delete-card (no-op for now since we only add new cards)
    {
        let window_weak = window.as_weak();
        window.on_delete_card(move || {
            let window = window_weak.unwrap();
            window.set_current_page(0);
        });
    }

    // show-deck-stats
    {
        let window_weak = window.as_weak();
        let state = Rc::clone(&state);
        window.on_show_deck_stats(move |deck_index| {
            let window = window_weak.unwrap();
            let st = state.borrow();
            let idx = deck_index as usize;
            let deck = &st.decks[idx];

            window.set_stats_deck_name(SharedString::from(&deck.name));
            window.set_stats(compute_stats(deck));
            window.set_current_page(3);
        });
    }

    // navigate-back
    {
        let window_weak = window.as_weak();
        let state = Rc::clone(&state);
        let deck_model = deck_model.clone();
        window.on_navigate_back(move || {
            let window = window_weak.unwrap();
            let mut st = state.borrow_mut();

            if st.current_session.is_some() {
                storage::save(&st.decks, &st.data_path).ok();
                refresh_deck_model(&st.decks, &deck_model);
                st.current_session = None;
            }

            window.set_current_page(0);
        });
    }

    window.run().unwrap();
}
