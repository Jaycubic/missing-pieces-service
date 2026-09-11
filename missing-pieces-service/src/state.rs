use chrono::{DateTime, Utc};
use rand::Rng;
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Mutex;
use tokio::sync::{mpsc, oneshot};
use tokio::time::{sleep_until, Duration, Instant};

use crate::db::{self, EventLogEntry};
use crate::protocol::*;
use crate::puzzle::Puzzle;

const DISCOVER_SECONDS: u64 = 120;
const CONNECT_SECONDS: u64 = 120;
const TOKENS_PER_PLAYER: u8 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PhaseKind {
    Lobby,
    Discover,
    Connect,
    Finished,
}

impl PhaseKind {
    fn as_protocol(&self) -> Phase {
        match self {
            PhaseKind::Lobby => Phase::Lobby,
            PhaseKind::Discover => Phase::Discover,
            PhaseKind::Connect => Phase::Connect,
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

/// Everything the outside world can ask a room to do. The room task is the
/// only thing that ever touches its own state — REST handlers and WebSocket
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
                // next tick moves straight to Finished instead of waiting
                // out the rest of the Connect timer.
                self.phase_deadline = Some(Instant::now());
            }
        } else {
            self.log(slot, "propose_connection", None, false);
        }
    }

    async fn advance_phase(&mut self) {
        match self.phase {
            PhaseKind::Discover => {
                self.phase = PhaseKind::Connect;
                self.phase_deadline = Some(Instant::now() + Duration::from_secs(CONNECT_SECONDS));
                self.broadcast();
            }
            PhaseKind::Connect => self.finish().await,
            PhaseKind::Lobby | PhaseKind::Finished => {}
        }
    }

    async fn finish(&mut self) {
        self.phase = PhaseKind::Finished;
        self.phase_deadline = None;
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
