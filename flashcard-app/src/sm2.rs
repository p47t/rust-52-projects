use chrono::Utc;

use crate::card::{Card, ReviewRating};

pub struct ReviewResult {
    pub new_ease_factor: f64,
    pub new_interval: u32,
    pub new_repetition: u32,
    pub next_review: chrono::DateTime<Utc>,
}

/// Apply the SM-2 algorithm to compute the next review schedule.
///
/// Algorithm (Wozniak):
/// 1. quality < 3: reset repetition to 0, interval to 1
/// 2. quality >= 3: n=1 → 1 day, n=2 → 6 days, n>2 → prev * EF
/// 3. EF' = EF + (0.1 - (5-q) * (0.08 + (5-q) * 0.02))
/// 4. EF minimum 1.3
pub fn review(card: &Card, rating: ReviewRating) -> ReviewResult {
    let q = rating.quality() as f64;

    let new_ef = (card.ease_factor + (0.1 - (5.0 - q) * (0.08 + (5.0 - q) * 0.02))).max(1.3);

    let (new_interval, new_repetition) = if rating == ReviewRating::Again {
        (1, 0)
    } else {
        let new_rep = card.repetition + 1;
        let interval = match new_rep {
            1 => 1,
            2 => 6,
            _ => (card.interval as f64 * new_ef).ceil() as u32,
        };
        (interval, new_rep)
    };

    let next_review = Utc::now() + chrono::Duration::days(new_interval as i64);

    ReviewResult {
        new_ease_factor: new_ef,
        new_interval,
        new_repetition,
        next_review,
    }
}

/// Returns indices of cards that are due for review.
pub fn due_cards(cards: &[Card]) -> Vec<usize> {
    let now = Utc::now();
    cards
        .iter()
        .enumerate()
        .filter(|(_, card)| card.next_review <= now)
        .map(|(i, _)| i)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::card::Card;

    fn test_card() -> Card {
        Card {
            id: uuid::Uuid::new_v4(),
            front: "Q".into(),
            back: "A".into(),
            ease_factor: 2.5,
            interval: 0,
            repetition: 0,
            next_review: Utc::now(),
            created_at: Utc::now(),
            last_reviewed: None,
            total_reviews: 0,
            correct_reviews: 0,
        }
    }

    #[test]
    fn again_resets_repetition() {
        let card = test_card();
        let result = review(&card, ReviewRating::Again);
        assert_eq!(result.new_repetition, 0);
        assert_eq!(result.new_interval, 1);
    }

    #[test]
    fn first_good_review_gives_interval_1() {
        let card = test_card();
        let result = review(&card, ReviewRating::Good);
        assert_eq!(result.new_repetition, 1);
        assert_eq!(result.new_interval, 1);
    }

    #[test]
    fn second_good_review_gives_interval_6() {
        let mut card = test_card();
        card.repetition = 1;
        card.interval = 1;
        let result = review(&card, ReviewRating::Good);
        assert_eq!(result.new_repetition, 2);
        assert_eq!(result.new_interval, 6);
    }

    #[test]
    fn third_good_review_uses_ease_factor() {
        let mut card = test_card();
        card.repetition = 2;
        card.interval = 6;
        card.ease_factor = 2.5;
        let result = review(&card, ReviewRating::Good);
        assert_eq!(result.new_repetition, 3);
        // interval = ceil(6 * 2.5) = 15 (EF changes slightly with Good rating)
        assert!(result.new_interval >= 14);
    }

    #[test]
    fn ease_factor_floor_at_1_3() {
        let mut card = test_card();
        card.ease_factor = 1.3;
        let result = review(&card, ReviewRating::Again);
        assert!(result.new_ease_factor >= 1.3);
    }

    #[test]
    fn easy_increases_ease_factor() {
        let card = test_card();
        let result = review(&card, ReviewRating::Easy);
        assert!(result.new_ease_factor > card.ease_factor);
    }

    #[test]
    fn due_cards_filters_correctly() {
        let mut cards = vec![test_card(), test_card(), test_card()];
        // Make the third card not due yet
        cards[2].next_review = Utc::now() + chrono::Duration::days(10);
        let due = due_cards(&cards);
        assert_eq!(due.len(), 2);
        assert_eq!(due, vec![0, 1]);
    }
}
