use crate::card::{Card, Deck};
use serde::Deserialize;

#[derive(Deserialize)]
struct SampleCard {
    front: String,
    back: String,
}

#[derive(Deserialize)]
struct SampleDeck {
    name: String,
    description: String,
    cards: Vec<SampleCard>,
}

pub fn sample_decks() -> Vec<Deck> {
    let data = include_str!("../data/sample_decks.json");
    let samples: Vec<SampleDeck> = serde_json::from_str(data).expect("invalid sample data");

    samples
        .into_iter()
        .map(|sd| {
            let mut deck = Deck::new(sd.name, sd.description);
            deck.cards = sd
                .cards
                .into_iter()
                .map(|c| Card::new(c.front, c.back))
                .collect();
            deck
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_sample_decks() {
        let decks = sample_decks();
        assert_eq!(decks.len(), 2);
        assert_eq!(decks[0].name, "Rust Basics");
        assert_eq!(decks[1].name, "World Capitals");
        assert_eq!(decks[0].cards.len(), 10);
        assert_eq!(decks[1].cards.len(), 10);
    }

    #[test]
    fn sample_cards_have_defaults() {
        let decks = sample_decks();
        let card = &decks[0].cards[0];
        assert_eq!(card.ease_factor, 2.5);
        assert_eq!(card.interval, 0);
        assert_eq!(card.repetition, 0);
    }
}
