use chrono::{DateTime, Utc};
use rand::Rng;
use sqlx::PgPool;
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use tokio::sync::{mpsc, oneshot};
use tokio::time::{sleep_until, Duration, Instant};

use crate::db::{self, EventLogEntry};
use crate::protocol::*;
use crate::puzzle::Puzzle;

const DISCOVER_SECONDS: u64 = 120;
const CONNECT_SECONDS: u64 = 120;
const RECONSTRUCT_SECONDS: u64 = 120;
const CONTRADICTION_SECONDS: u64 = 60;
const VALIDATE_SECONDS: u64 = 60;
const DECIDE_SECONDS: u64 = 30;
const TOKENS_PER_PLAYER: u8 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PhaseKind {
    Lobby,
    Discover,
    Connect,
    Reconstruct,
    Contradiction,
    Validate,
    Decide,
    Finished,
}

impl PhaseKind {
    fn as_protocol(&self) -> Phase {
        match self {
            PhaseKind::Lobby => Phase::Lobby,
            PhaseKind::Discover => Phase::Discover,
            PhaseKind::Connect => Phase::Connect,
            PhaseKind::Reconstruct => Phase::Reconstruct,
            PhaseKind::Contradiction => Phase::Contradiction,
            PhaseKind::Validate => Phase::Validate,
            PhaseKind::Decide => Phase::Decide,
            PhaseKind::Finished => Phase::Finished,
        }
    }
}

struct PlayerRuntime {
    device_token: String,
    name: String,
    tokens_remaining: u8,
    hints_revealed: usize,
    solved: bool,
    sender: Option<mpsc::UnboundedSender<ServerMessage>>,
    connected: bool,
}

struct FinalResultInternal {
    winning_solution_id: Option<String>,
    team_correct: bool,
}

/// Everything the outside world can ask a room to do. The room task is the
/// only thing that ever mutates its own state — REST handlers and WebSocket
/// connections only ever send messages in and wait for a reply.
pub enum RoomEvent {
    Join {
        device_token: String,
        name: String,
        reply: oneshot::Sender<Result<u8, String>>,
    },
    Reconnect {
        device_token: String,
        reply: oneshot::Sender<Option<u8>>,
    },
    PlayerConnected {
        slot: u8,
        sender: mpsc::UnboundedSender<ServerMessage>,
    },
    PlayerDisconnected {
        slot: u8,
    },
    PlayerAction {
        slot: u8,
        action: PlayerAction,
    },
}

struct RoomActor {
    code: String,
    puzzle: Puzzle,
    phase: PhaseKind,
    phase_deadline: Option<Instant>,
    started_at: DateTime<Utc>,
    players: HashMap<u8, PlayerRuntime>,

    connections_found: Vec<(String, String)>,

    current_sequence: Vec<String>,
    sequence_solved: bool,
    sequence_attempts: u32,

    contradiction_solved: bool,
    contradiction_attempts: u32,

    flagged_solutions: HashSet<String>,
    votes: HashMap<u8, String>,
    final_result: Option<FinalResultInternal>,

    events: Vec<EventLogEntry>,
    pool: PgPool,
    rx: mpsc::UnboundedReceiver<RoomEvent>,
}

impl RoomActor {
    fn new(code: String, puzzle: Puzzle, pool: PgPool, rx: mpsc::UnboundedReceiver<RoomEvent>) -> Self {
        Self {
            code,
            puzzle,
            phase: PhaseKind::Lobby,
            phase_deadline: None,
            started_at: Utc::now(),
            players: HashMap::new(),
            connections_found: Vec::new(),
            current_sequence: Vec::new(),
            sequence_solved: false,
            sequence_attempts: 0,
            contradiction_solved: false,
            contradiction_attempts: 0,
            flagged_solutions: HashSet::new(),
            votes: HashMap::new(),
            final_result: None,
            events: Vec::new(),
            pool,
            rx,
        }
    }

    async fn run(mut self) {
        loop {
            let deadline = self.phase_deadline;
            tokio::select! {
                event = self.rx.recv() => {
                    match event {
                        Some(ev) => self.handle_event(ev).await,
                        None => break, // every sender dropped — nobody can reach this room anymore
                    }
                }
                _ = async {
                    match deadline {
                        Some(d) => sleep_until(d).await,
                        None => std::future::pending::<()>().await,
                    }
                } => {
                    self.advance_phase().await;
                }
            }

            if self.phase == PhaseKind::Finished {
                // Keep the task alive briefly so a late reconnect still sees
                // the final result, then let it drop.
                tokio::time::sleep(Duration::from_secs(300)).await;
                break;
            }
        }
    }

    async fn handle_event(&mut self, event: RoomEvent) {
        match event {
            RoomEvent::Join { device_token, name, reply } => {
                let result = self.handle_join(device_token, name);
                let _ = reply.send(result);
                self.broadcast();
            }
            RoomEvent::Reconnect { device_token, reply } => {
                let slot = self
                    .players
                    .iter()
                    .find(|(_, p)| p.device_token == device_token)
                    .map(|(slot, _)| *slot);
                let _ = reply.send(slot);
            }
            RoomEvent::PlayerConnected { slot, sender } => {
                if let Some(p) = self.players.get_mut(&slot) {
                    p.sender = Some(sender);
                    p.connected = true;
                }
                self.broadcast();
            }
            RoomEvent::PlayerDisconnected { slot } => {
                if let Some(p) = self.players.get_mut(&slot) {
                    p.connected = false;
                }
                self.broadcast();
            }
            RoomEvent::PlayerAction { slot, action } => {
                self.handle_action(slot, action);
                self.broadcast();
            }
        }
    }

    fn handle_join(&mut self, device_token: String, name: String) -> Result<u8, String> {
        if let Some((slot, _)) = self.players.iter().find(|(_, p)| p.device_token == device_token) {
            return Ok(*slot);
        }
        if self.players.len() >= self.puzzle.player_count() {
            return Err("room is full".into());
        }
        if self.phase != PhaseKind::Lobby {
            return Err("this session has already started".into());
        }

        let slot = self.players.len() as u8;
        self.players.insert(
            slot,
            PlayerRuntime {
                device_token,
                name,
                tokens_remaining: TOKENS_PER_PLAYER,
                hints_revealed: 0,
                solved: false,
                sender: None,
                connected: false,
            },
        );

        if self.players.len() == self.puzzle.player_count() {
            self.phase = PhaseKind::Discover;
            self.phase_deadline = Some(Instant::now() + Duration::from_secs(DISCOVER_SECONDS));
            self.started_at = Utc::now();
        }

        Ok(slot)
    }

    fn handle_action(&mut self, slot: u8, action: PlayerAction) {
        match action {
            PlayerAction::AskClue { target_slot } => self.handle_ask_clue(slot, target_slot),
            PlayerAction::GuessHiddenCard { option_id } => self.handle_guess(slot, &option_id),
            PlayerAction::ProposeConnection { card_a, card_b } => {
                self.handle_propose_connection(slot, &card_a, &card_b)
            }
            PlayerAction::SubmitSequence { ordered_card_ids } => {
                self.handle_submit_sequence(slot, ordered_card_ids)
            }
            PlayerAction::ProposeContradiction { card_a, card_b } => {
                self.handle_propose_contradiction(slot, &card_a, &card_b)
            }
            PlayerAction::FlagSolution { solution_id } => self.handle_flag_solution(slot, &solution_id),
            PlayerAction::CastVote { solution_id } => self.handle_cast_vote(slot, &solution_id),
            PlayerAction::Ready => { /* reserved for a future early-advance vote */ }
        }
    }

    fn handle_ask_clue(&mut self, slot: u8, target_slot: u8) {
        if self.phase != PhaseKind::Discover || target_slot == slot || !self.players.contains_key(&target_slot) {
            return;
        }
        let Some(runtime) = self.players.get(&slot) else { return };
        if runtime.solved || runtime.tokens_remaining == 0 {
            self.log(slot, "ask_clue", Some(target_slot), false);
            return;
        }

        let next_hint = self
            .puzzle
            .player_at(slot)
            .and_then(|pp| pp.hidden_card.hints.get(runtime.hints_revealed).cloned());

        if next_hint.is_some() {
            if let Some(p) = self.players.get_mut(&slot) {
                p.tokens_remaining -= 1;
                p.hints_revealed += 1;
            }
            self.log(slot, "ask_clue", Some(target_slot), true);
        } else {
            self.log(slot, "ask_clue", Some(target_slot), false);
        }
    }

    fn handle_guess(&mut self, slot: u8, option_id: &str) {
        if self.phase != PhaseKind::Discover {
            return;
        }
        let Some(already_solved) = self.players.get(&slot).map(|p| p.solved) else { return };
        if already_solved {
            return;
        }
        let Some(pp) = self.puzzle.player_at(slot) else { return };

        let correct = pp.hidden_card.guess_options.iter().any(|g| g.id == option_id && g.correct);
        if correct {
            if let Some(p) = self.players.get_mut(&slot) {
                p.solved = true;
            }
        }
        self.log(slot, "guess_hidden_card", None, correct);
    }

    fn handle_propose_connection(&mut self, slot: u8, card_a: &str, card_b: &str) {
        if self.phase != PhaseKind::Connect {
            return;
        }
        let already_found = self
            .connections_found
            .iter()
            .any(|(a, b)| (a == card_a && b == card_b) || (a == card_b && b == card_a));
        let valid = self.puzzle.is_valid_connection(card_a, card_b);

        if valid && !already_found {
            self.connections_found.push((card_a.to_string(), card_b.to_string()));
            self.log(slot, "propose_connection", None, true);
            if self.connections_found.len() >= self.puzzle.connections.len() {
                // Team found everything early — collapse the clock so the
                // next tick moves straight on instead of waiting out the rest
                // of the Connect timer.
                self.phase_deadline = Some(Instant::now());
            }
        } else {
            self.log(slot, "propose_connection", None, false);
        }
    }

    fn handle_submit_sequence(&mut self, slot: u8, ordered_card_ids: Vec<String>) {
        if self.phase != PhaseKind::Reconstruct || self.sequence_solved {
            return;
        }
        self.sequence_attempts += 1;
        self.current_sequence = ordered_card_ids.clone();
        let correct = ordered_card_ids == self.puzzle.sequence.correct_order;
        self.log(slot, "submit_sequence", None, correct);
        if correct {
            self.sequence_solved = true;
            self.phase_deadline = Some(Instant::now());
        }
    }

    fn handle_propose_contradiction(&mut self, slot: u8, card_a: &str, card_b: &str) {
        if self.phase != PhaseKind::Contradiction || self.contradiction_solved {
            return;
        }
        self.contradiction_attempts += 1;
        let (pa, pb) = &self.puzzle.contradiction.pair;
        let correct = (card_a == pa && card_b == pb) || (card_a == pb && card_b == pa);
        self.log(slot, "propose_contradiction", None, correct);
        if correct {
            self.contradiction_solved = true;
            self.phase_deadline = Some(Instant::now());
        }
    }

    fn handle_flag_solution(&mut self, slot: u8, solution_id: &str) {
        if self.phase != PhaseKind::Validate {
            return;
        }
        let now_flagged = if self.flagged_solutions.contains(solution_id) {
            self.flagged_solutions.remove(solution_id);
            false
        } else {
            self.flagged_solutions.insert(solution_id.to_string());
            true
        };
        self.log(slot, "flag_solution", None, now_flagged);
    }

    fn handle_cast_vote(&mut self, slot: u8, solution_id: &str) {
        if self.phase != PhaseKind::Decide {
            return;
        }
        let changed = self.votes.get(&slot).map(|v| v != solution_id).unwrap_or(true);
        self.votes.insert(slot, solution_id.to_string());
        self.log(slot, "cast_vote", None, changed);
    }

    async fn advance_phase(&mut self) {
        match self.phase {
            PhaseKind::Discover => {
                self.phase = PhaseKind::Connect;
                self.phase_deadline = Some(Instant::now() + Duration::from_secs(CONNECT_SECONDS));
                self.broadcast();
            }
            PhaseKind::Connect => {
                self.phase = PhaseKind::Reconstruct;
                self.phase_deadline = Some(Instant::now() + Duration::from_secs(RECONSTRUCT_SECONDS));
                self.broadcast();
            }
            PhaseKind::Reconstruct => {
                self.phase = PhaseKind::Contradiction;
                self.phase_deadline = Some(Instant::now() + Duration::from_secs(CONTRADICTION_SECONDS));
                self.broadcast();
            }
            PhaseKind::Contradiction => {
                self.phase = PhaseKind::Validate;
                self.phase_deadline = Some(Instant::now() + Duration::from_secs(VALIDATE_SECONDS));
                self.broadcast();
            }
            PhaseKind::Validate => {
                self.phase = PhaseKind::Decide;
                self.phase_deadline = Some(Instant::now() + Duration::from_secs(DECIDE_SECONDS));
                self.broadcast();
            }
            PhaseKind::Decide => self.finish().await,
            PhaseKind::Lobby | PhaseKind::Finished => {}
        }
    }

    async fn finish(&mut self) {
        self.phase = PhaseKind::Finished;
        self.phase_deadline = None;

        let mut tally: HashMap<String, u32> = HashMap::new();
        for sol_id in self.votes.values() {
            *tally.entry(sol_id.clone()).or_insert(0) += 1;
        }
        let winning = tally.iter().max_by_key(|(_, count)| **count).map(|(id, _)| id.clone());
        let correct_id = self.puzzle.correct_solution_id().map(|s| s.to_string());
        let team_correct = winning.is_some() && winning == correct_id;

        self.final_result = Some(FinalResultInternal { winning_solution_id: winning, team_correct });
        self.broadcast();

        let result = db::persist_session(
            &self.pool,
            &self.code,
            &self.puzzle.id,
            self.started_at,
            self.connections_found.len(),
            self.puzzle.connections.len(),
            &self.events,
        )
        .await;

        if let Err(e) = result {
            tracing::error!(room = %self.code, error = %e, "failed to persist session analytics");
        }
    }

    fn log(&mut self, slot: u8, action_type: &str, target_slot: Option<u8>, successful: bool) {
        self.events.push(EventLogEntry {
            player_slot: slot as i32,
            action_type: action_type.to_string(),
            target_slot: target_slot.map(|s| s as i32),
            phase: format!("{:?}", self.phase).to_lowercase(),
            successful: Some(successful),
            occurred_at: Utc::now(),
        });
    }

    fn broadcast(&self) {
        for (&slot, player) in self.players.iter() {
            if let Some(sender) = &player.sender {
                let _ = sender.send(ServerMessage::State(self.build_state_for(slot)));
            }
        }
    }

    fn build_state_for(&self, viewer_slot: u8) -> StatePush {
        let phase_ends_at = self.phase_deadline.map(|deadline| {
            let remaining = deadline.saturating_duration_since(Instant::now());
            Utc::now() + chrono::Duration::from_std(remaining).unwrap_or_default()
        });

        let me_runtime = self.players.get(&viewer_slot);
        let me_puzzle = self.puzzle.player_at(viewer_slot);
        let hints_revealed = me_runtime.map(|p| p.hints_revealed).unwrap_or(0);

        let me = MeView {
            slot: viewer_slot,
            name: me_runtime.map(|p| p.name.clone()).unwrap_or_default(),
            visible_cards: me_puzzle
                .map(|pp| {
                    pp.visible_cards
                        .iter()
                        .map(|c| VisibleCardView { id: c.id.clone(), text: c.text.clone() })
                        .collect()
                })
                .unwrap_or_default(),
            hidden_card: MyHiddenCardView {
                category_known: me_puzzle.and_then(|pp| {
                    if hints_revealed > 0 { Some(pp.hidden_card.category.clone()) } else { None }
                }),
                hints_received: me_puzzle
                    .map(|pp| pp.hidden_card.hints.iter().take(hints_revealed).cloned().collect())
                    .unwrap_or_default(),
                solved: me_runtime.map(|p| p.solved).unwrap_or(false),
                guess_options: me_puzzle
                    .map(|pp| {
                        pp.hidden_card
                            .guess_options
                            .iter()
                            .map(|g| GuessOptionView { id: g.id.clone(), text: g.text.clone() })
                            .collect()
                    })
                    .unwrap_or_default(),
            },
            tokens_remaining: me_runtime.map(|p| p.tokens_remaining).unwrap_or(0),
        };

        let mut others: Vec<OtherPlayerView> = self
            .players
            .iter()
            .filter(|(&slot, _)| slot != viewer_slot)
            .filter_map(|(&slot, runtime)| {
                self.puzzle.player_at(slot).map(|pp| OtherPlayerView {
                    slot,
                    name: runtime.name.clone(),
                    hidden_card_id: pp.hidden_card.id.clone(),
                    hidden_card_text: pp.hidden_card.text.clone(),
                    solved: runtime.solved,
                    connected: runtime.connected,
                })
            })
            .collect();
        others.sort_by_key(|o| o.slot);

        let connections_found = self
            .connections_found
            .iter()
            .map(|(a, b)| FoundConnection { card_a: a.clone(), card_b: b.clone(), found_by_slot: 0 })
            .collect();

        let sequence_cards = self
            .puzzle
            .sequence
            .cards
            .iter()
            .map(|c| SequenceCardView { id: c.id.clone(), text: c.text.clone() })
            .collect();

        let contradiction_cards = self
            .puzzle
            .contradiction
            .cards
            .iter()
            .map(|c| ContradictionCardView { id: c.id.clone(), text: c.text.clone() })
            .collect();

        let solutions = self
            .puzzle
            .solutions
            .iter()
            .map(|s| SolutionView { id: s.id.clone(), text: s.text.clone() })
            .collect();

        let mut votes: Vec<VoteView> = self
            .votes
            .iter()
            .map(|(&slot, sol)| VoteView { slot, solution_id: sol.clone() })
            .collect();
        votes.sort_by_key(|v| v.slot);

        StatePush {
            phase: self.phase.as_protocol(),
            phase_ends_at,
            puzzle_title: self.puzzle.title.clone(),
            puzzle_context: self.puzzle.context.clone(),
            clue_categories: self.puzzle.clue_categories.clone(),
            me,
            others,
            connections_found,
            connections_total: self.puzzle.connections.len(),

            sequence_cards,
            sequence_length_needed: self.puzzle.sequence.correct_order.len(),
            current_sequence: self.current_sequence.clone(),
            sequence_solved: self.sequence_solved,

            contradiction_cards,
            contradiction_solved: self.contradiction_solved,

            solutions,
            flagged_solutions: self.flagged_solutions.iter().cloned().collect(),
            votes,

            final_result: self.final_result.as_ref().map(|f| FinalResult {
                winning_solution_id: f.winning_solution_id.clone(),
                team_correct: f.team_correct,
            }),
            last_error: None,
        }
    }
}

/// Shared across every request: the loaded puzzle library, the DB pool, and
/// a registry mapping active room codes to that room's event channel.
pub struct AppState {
    pub puzzles: HashMap<String, Puzzle>,
    pub pool: PgPool,
    rooms: Mutex<HashMap<String, mpsc::UnboundedSender<RoomEvent>>>,
}

impl AppState {
    pub fn new(puzzles: HashMap<String, Puzzle>, pool: PgPool) -> Self {
        Self { puzzles, pool, rooms: Mutex::new(HashMap::new()) }
    }

    pub fn create_room(&self, puzzle_id: Option<&str>) -> Result<String, String> {
        let puzzle = match puzzle_id {
            Some(id) => self.puzzles.get(id).cloned(),
            None => self.puzzles.values().next().cloned(),
        }
        .ok_or_else(|| "no puzzle available on this server".to_string())?;

        let mut rooms = self.rooms.lock().expect("room registry mutex poisoned");
        let mut code = generate_room_code();
        while rooms.contains_key(&code) {
            code = generate_room_code();
        }

        let (tx, rx) = mpsc::unbounded_channel();
        let actor = RoomActor::new(code.clone(), puzzle, self.pool.clone(), rx);
        tokio::spawn(actor.run());

        rooms.insert(code.clone(), tx);
        Ok(code)
    }

    pub fn room_sender(&self, code: &str) -> Option<mpsc::UnboundedSender<RoomEvent>> {
        let rooms = self.rooms.lock().expect("room registry mutex poisoned");
        rooms.get(code).cloned()
    }
}

fn generate_room_code() -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHJKMNPQRSTUVWXYZ23456789";
    let mut rng = rand::thread_rng();
    (0..6).map(|_| ALPHABET[rng.gen_range(0..ALPHABET.len())] as char).collect()
}
