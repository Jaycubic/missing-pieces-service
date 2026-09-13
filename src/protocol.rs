use serde::{Deserialize, Serialize};

// ---------- Client → Server ----------

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PlayerAction {
    /// Ask a specific other player for the next available hint about MY
    /// hidden card. The server decides which hint (if any are left) — the
    /// client cannot request a specific one.
    AskClue { target_slot: u8 },

    /// Submit a guess at my own hidden card's content, from the fixed
    /// multiple-choice options. Right or wrong, this consumes the attempt.
    GuessHiddenCard { option_id: String },

    /// Propose that two cards (by id) are meaningfully connected.
    ProposeConnection { card_a: String, card_b: String },

    /// Submit the team's current best guess at the event order. Any player
    /// can resubmit as many times as time allows — wrong guesses just show
    /// the team what was tried, they don't cost anything.
    SubmitSequence { ordered_card_ids: Vec<String> },

    /// Propose that two cards (by id) are the contradicting pair.
    ProposeContradiction { card_a: String, card_b: String },

    /// Toggle a shared, team-visible marker that a candidate solution looks
    /// ruled out by the evidence so far. Purely a discussion aid — it does
    /// not affect scoring.
    FlagSolution { solution_id: String },

    /// Cast (or change) this player's vote for the final solution.
    CastVote { solution_id: String },

    /// Signal readiness to leave the lobby, or to move on early once
    /// everyone's done in a phase that doesn't strictly need the full clock.
    Ready,
}

// ---------- Server → Client ----------
// Everything from Reconstruct onward is intentionally NOT redacted per
// player — by this point in the design, all information is meant to be
// commonly known to the team. Only Discover has per-player asymmetric
// visibility; that's the one phase build_state_for() customizes per slot.

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Lobby,
    Discover,
    Connect,
    Reconstruct,
    Contradiction,
    Validate,
    Decide,
    Finished,
}

#[derive(Debug, Clone, Serialize)]
pub struct VisibleCardView {
    pub id: String,
    pub text: String,
}

/// What the payload contains for MY OWN hidden card — never the text,
/// only how far I've gotten toward figuring it out.
#[derive(Debug, Clone, Serialize)]
pub struct MyHiddenCardView {
    pub category_known: Option<String>,
    pub hints_received: Vec<String>,
    pub solved: bool,
    pub guess_options: Vec<GuessOptionView>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GuessOptionView {
    pub id: String,
    pub text: String,
    // `correct` is deliberately never included here.
}

/// What every OTHER player's hidden card looks like to me — full text,
/// exactly per the design's "other players can see it" rule.
#[derive(Debug, Clone, Serialize)]
pub struct OtherPlayerView {
    pub slot: u8,
    pub name: String,
    pub hidden_card_id: String,
    pub hidden_card_text: String,
    pub solved: bool,
    pub connected: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct MeView {
    pub slot: u8,
    pub name: String,
    pub visible_cards: Vec<VisibleCardView>,
    pub hidden_card: MyHiddenCardView,
    pub tokens_remaining: u8,
}

#[derive(Debug, Clone, Serialize)]
pub struct FoundConnection {
    pub card_a: String,
    pub card_b: String,
    pub found_by_slot: u8,
}

#[derive(Debug, Clone, Serialize)]
pub struct SequenceCardView {
    pub id: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ContradictionCardView {
    pub id: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SolutionView {
    pub id: String,
    pub text: String,
    // `correct` is deliberately never included here — the team must reason
    // it out, not read it off the wire.
}

#[derive(Debug, Clone, Serialize)]
pub struct VoteView {
    pub slot: u8,
    pub solution_id: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct FinalResult {
    pub winning_solution_id: Option<String>,
    pub team_correct: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct StatePush {
    pub phase: Phase,
    pub phase_ends_at: Option<chrono::DateTime<chrono::Utc>>,
    pub puzzle_title: String,
    pub puzzle_context: String,
    pub clue_categories: Vec<String>,
    pub me: MeView,
    pub others: Vec<OtherPlayerView>,
    pub connections_found: Vec<FoundConnection>,
    pub connections_total: usize,

    pub sequence_cards: Vec<SequenceCardView>,
    pub sequence_length_needed: usize,
    pub current_sequence: Vec<String>,
    pub sequence_solved: bool,

    pub contradiction_cards: Vec<ContradictionCardView>,
    pub contradiction_solved: bool,

    pub solutions: Vec<SolutionView>,
    pub flagged_solutions: Vec<String>,
    pub votes: Vec<VoteView>,

    pub final_result: Option<FinalResult>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    State(StatePush),
    /// Reserved for a future graceful-shutdown path (e.g. an admin ending a
    /// session early); not sent anywhere yet.
    #[allow(dead_code)]
    RoomClosed { reason: String },
}
