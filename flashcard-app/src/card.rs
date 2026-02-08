use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Card {
    pub id: Uuid,
    pub front: String,
    pub back: String,
    pub ease_factor: f64,
    pub interval: u32,
    pub repetition: u32,
    pub next_review: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub last_reviewed: Option<DateTime<Utc>>,
    pub total_reviews: u32,
    pub correct_reviews: u32,
}

impl Card {
    pub fn new(front: String, back: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            front,
            back,
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Deck {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    pub cards: Vec<Card>,
    pub created_at: DateTime<Utc>,
}

impl Deck {
    pub fn new(name: String, description: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            description,
            cards: Vec::new(),
            created_at: Utc::now(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewRating {
    Again,
    Hard,
    Good,
    Easy,
}

impl ReviewRating {
    pub fn quality(self) -> u8 {
        match self {
            ReviewRating::Again => 1,
            ReviewRating::Hard => 3,
            ReviewRating::Good => 4,
            ReviewRating::Easy => 5,
        }
    }

    pub fn from_int(value: i32) -> Self {
        match value {
            0 => ReviewRating::Again,
            1 => ReviewRating::Hard,
            2 => ReviewRating::Good,
            _ => ReviewRating::Easy,
        }
    }
}
