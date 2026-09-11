use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VisibleCard {
    pub id: String,
    pub text: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GuessOption {
    pub id: String,
    pub text: String,
    #[serde(default)]
    pub correct: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HiddenCard {
    pub id: String,
    pub text: String,
    pub category: String,
    pub hints: Vec<String>,
    pub guess_options: Vec<GuessOption>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PuzzlePlayer {
    pub slot: u8,
    pub visible_cards: Vec<VisibleCard>,
    pub hidden_card: HiddenCard,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Puzzle {
    pub id: String,
    pub title: String,
    pub context: String,
    pub clue_categories: Vec<String>,
    pub players: Vec<PuzzlePlayer>,
    /// Each entry is a pair of card ids that the Connect phase considers a
    /// valid, meaningful link. Order within a pair doesn't matter.
    pub connections: Vec<(String, String)>,
}

impl Puzzle {
    pub fn player_count(&self) -> usize {
        self.players.len()
    }

    pub fn player_at(&self, slot: u8) -> Option<&PuzzlePlayer> {
        self.players.iter().find(|p| p.slot == slot)
    }

    /// True if (a, b) — in either order — is one of this puzzle's valid links.
    pub fn is_valid_connection(&self, a: &str, b: &str) -> bool {
        self.connections
            .iter()
            .any(|(x, y)| (x == a && y == b) || (x == b && y == a))
    }

    /// Not called yet in this MVP — reserved for the Contradiction phase,
    /// where knowing who holds a given hidden card becomes a game action.
    #[allow(dead_code)]
    pub fn find_hidden_card_owner(&self, card_id: &str) -> Option<u8> {
        self.players
            .iter()
            .find(|p| p.hidden_card.id == card_id)
            .map(|p| p.slot)
    }
}

/// Loads every `*.json` file in the puzzles directory at startup. A bad file
/// is logged and skipped rather than crashing the whole server — one broken
/// puzzle shouldn't take down a live game night.
pub fn load_puzzles(dir: &str) -> HashMap<String, Puzzle> {
    let mut puzzles = HashMap::new();
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => {
            tracing::warn!(dir, error = %e, "could not read puzzles directory");
            return puzzles;
        }
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        match load_one(&path) {
            Ok(puzzle) => {
                tracing::info!(id = %puzzle.id, title = %puzzle.title, "loaded puzzle");
                puzzles.insert(puzzle.id.clone(), puzzle);
            }
            Err(e) => tracing::warn!(path = %path.display(), error = %e, "skipping invalid puzzle file"),
        }
    }
    puzzles
}

fn load_one(path: &Path) -> anyhow::Result<Puzzle> {
    let raw = fs::read_to_string(path)?;
    let puzzle: Puzzle = serde_json::from_str(&raw)?;
    Ok(puzzle)
}
