#![no_std]
//! `no_std` business logic for tiny games.
//!
//! MicroGames is the part of a game that should not know whether it is being
//! displayed in a terminal, framebuffer, GPU window, serial shell, embedded
//! display, or custom OS UI. It owns rules and state transitions; your platform
//! owns events, input, timing, audio, and output.
//!
//! The crate currently includes engines for:
//!
//! - Tetris-like falling blocks via [`Game`]
//! - [`snake`]
//! - [`minesweeper`]
//! - [`bejewled`]
//! - [`chess`]
//! - a terminal-oriented Tetris shell adapter in [`shell`]
//!
//! # Basic Tetris Loop
//!
//! ```
//! use microgames::{Lcg32, NoopEvents, Rotation};
//!
//! let mut rng = Lcg32::new(0xC11C_7E75);
//! let mut events = NoopEvents;
//! let mut game = microgames::Game::<10, 24, 4>::new(&mut rng, &mut events);
//!
//! game.move_left();
//! game.rotate(Rotation::Cw);
//! game.soft_drop(&mut rng, &mut events);
//!
//! for y in game.hidden_rows()..game.height_total() {
//!     for x in 0..game.width() {
//!         let cell = game.cell_view_at(x, y, true);
//!         let _ = cell;
//!     }
//! }
//! ```
//!
//! # Events
//!
//! Game engines report meaningful side effects through event traits. A renderer,
//! audio system, logger, network sync layer, or test harness can implement only
//! the callbacks it needs.
//!
//! ```
//! use microgames::{Rgb8, TetrisEvents};
//!
//! struct Effects;
//!
//! impl TetrisEvents for Effects {
//!     fn on_block_placed(&mut self, color: Rgb8, x: usize, y: usize) {
//!         let _ = (color, x, y);
//!     }
//! }
//! ```
//!
//! Use [`NoopEvents`] when you only want to drive the rules.

use core::cmp::{max, min};

/// Bejeweled-style match-3 game logic.
pub mod bejewled;
/// Chess board, legal move, castling, promotion, en-passant, and state logic.
pub mod chess;
/// Minesweeper board generation, reveal, flag, chord, and win/loss logic.
pub mod minesweeper;
/// A terminal-oriented Tetris shell adapter built on [`core::fmt`].
pub mod shell;
/// Snake movement, growth, food, scoring, and collision logic.
pub mod snake;

/// Maximum number of cells a Tetris-like piece can occupy.
pub const MAX_PIECE_CELLS: usize = 8;

/// Small RGB color used by the built-in game logic and renderer adapters.
///
/// It is intentionally local to this crate so MicroGames does not depend on any
/// platform UI crate.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rgb8 {
    /// Red channel.
    pub r: u8,
    /// Green channel.
    pub g: u8,
    /// Blue channel.
    pub b: u8,
}

impl Rgb8 {
    /// Creates a new 8-bit RGB color.
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Point {
    x: i16,
    y: i16,
}

impl Point {
    const fn new(x: i16, y: i16) -> Self {
        Self { x, y }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Block {
    /// The block color selected when the piece was spawned.
    pub color: Rgb8,
}

/// Tetris-like piece shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PieceKind {
    Line,
    ZigZag,
    L,
    Square,
    Tee,
    Dot,
    J,
    Evil,
}

impl PieceKind {
    /// All standard non-evil pieces.
    pub const NON_EVIL: [PieceKind; 7] = [
        PieceKind::Line,
        PieceKind::ZigZag,
        PieceKind::L,
        PieceKind::Square,
        PieceKind::Tee,
        PieceKind::Dot,
        PieceKind::J,
    ];

    /// Every piece this engine can spawn, including the optional evil block.
    pub const ALL: [PieceKind; 8] = [
        PieceKind::Line,
        PieceKind::ZigZag,
        PieceKind::L,
        PieceKind::Square,
        PieceKind::Tee,
        PieceKind::Dot,
        PieceKind::J,
        PieceKind::Evil,
    ];
}

/// Rotation direction for Tetris-like pieces.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rotation {
    /// Clockwise rotation.
    Cw,
    /// Counter-clockwise rotation.
    Ccw,
}

/// Result of a Tetris-like gravity or drop tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TickResult {
    /// The current piece moved down.
    Moved,
    /// The current piece locked and a new piece was spawned.
    Locked,
    /// The board can no longer accept a new piece.
    GameOver,
}

/// Feature unlocked by the built-in Tetris level progression.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Feature {
    Minimum,
    Colors,
    Music,
    Preview,
    Rotation,
    ForwardBackward,
    Flashlight,
    SpinUpgrade,
    EvilBlocks,
    Shifted,
}

/// Compact set of enabled [`Feature`] values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FeatureFlags(u16);

impl FeatureFlags {
    /// Creates the default feature set with [`Feature::Minimum`] enabled.
    pub const fn new() -> Self {
        Self(1 << (Feature::Minimum as u16))
    }

    /// Returns true when `feature` is enabled.
    pub fn contains(self, feature: Feature) -> bool {
        (self.0 & (1 << (feature as u16))) != 0
    }

    /// Enables `feature`.
    pub fn insert(&mut self, feature: Feature) {
        self.0 |= 1 << (feature as u16);
    }

    /// Disables `feature`.
    pub fn remove(&mut self, feature: Feature) {
        self.0 &= !(1 << (feature as u16));
    }
}

impl Default for FeatureFlags {
    fn default() -> Self {
        Self::new()
    }
}

/// Built-in Tetris-like level, score, and feature progression.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LevelState {
    /// First level.
    pub start_level: u8,
    /// Final level.
    pub end_level: u8,
    /// Current active level.
    pub current_level: u8,
    /// Score multiplier.
    pub multiplier: u16,
    /// Number of locked pieces.
    pub placed_pieces: u32,
    /// Number of cleared rows.
    pub rows_deleted: u32,
    /// Accumulated points.
    pub total_points: u32,
    /// Enabled feature flags.
    pub features: FeatureFlags,
}

impl LevelState {
    /// Creates the default level state.
    pub const fn new() -> Self {
        Self {
            start_level: 1,
            end_level: 10,
            current_level: 1,
            multiplier: 1,
            placed_pieces: 0,
            rows_deleted: 0,
            total_points: 0,
            features: FeatureFlags::new(),
        }
    }

    /// Rows remaining until the next level boundary.
    pub fn next_level_in_rows(&self) -> u32 {
        5 - (self.rows_deleted % 5)
    }

    /// Current gravity interval in milliseconds.
    pub fn level_speed_seconds(&self) -> u32 {
        let speed_millis = 400_i32 - (self.current_level as i32 * 25_i32);
        max(50, speed_millis) as u32
    }

    /// Updates level state after rows were deleted and emits unlock events.
    pub fn on_rows_deleted<T: TetrisEvents>(&mut self, count: u32, events: &mut T) {
        let before = self.rows_deleted;
        self.rows_deleted = self.rows_deleted.saturating_add(count);

        let old_level = self.current_level;
        let new_level_raw = self.start_level as u32 + (self.rows_deleted / 5);
        self.current_level = min(self.end_level as u32, new_level_raw) as u8;

        if self.current_level <= old_level {
            return;
        }

        for lvl in (old_level + 1)..=self.current_level {
            match lvl {
                2 => self.features.insert(Feature::Colors),
                3 => {
                    self.features.insert(Feature::Music);
                    events.on_music(0);
                }
                4 => self.features.insert(Feature::Preview),
                5 => self.features.insert(Feature::Rotation),
                6 => self.features.insert(Feature::ForwardBackward),
                7 => {
                    self.features.insert(Feature::Flashlight);
                    events.on_music(1);
                }
                8 => {
                    self.features.insert(Feature::SpinUpgrade);
                }
                9 => self.features.insert(Feature::EvilBlocks),
                10 => self.features.insert(Feature::Shifted),
                _ => {}
            }
        }

        let _ = before;
    }

    /// Updates score and placed-piece count after a piece locks.
    pub fn on_piece_placed(&mut self) {
        self.placed_pieces = self.placed_pieces.saturating_add(1);
        self.total_points = self
            .total_points
            .saturating_add(self.current_level as u32 * self.multiplier as u32);
    }
}

impl Default for LevelState {
    fn default() -> Self {
        Self::new()
    }
}

/// Event hooks emitted by the Tetris-like engine.
pub trait TetrisEvents {
    /// Called for each block written to the board when a piece locks.
    fn on_block_placed(&mut self, _color: Rgb8, _x: usize, _y: usize) {}
    /// Called when level progression requests a music change.
    fn on_music(&mut self, _track_id: u8) {}
    /// Called before a full row is removed.
    fn on_row_deleted(&mut self, _row: usize, _colors: &[Option<Rgb8>]) {}
    /// Called when the game reaches a terminal game-over state.
    fn on_game_over(&mut self) {}
}

/// Event sink that ignores every callback.
pub struct NoopEvents;

impl TetrisEvents for NoopEvents {}

/// Random source used by the games that need deterministic randomness.
pub trait RandomSource {
    /// Returns the next random `u32`.
    fn next_u32(&mut self) -> u32;
}

/// Tiny deterministic linear-congruential random source.
#[derive(Clone, Copy, Debug)]
pub struct Lcg32 {
    state: u32,
}

impl Lcg32 {
    /// Creates a new generator from `seed`.
    pub const fn new(seed: u32) -> Self {
        Self { state: seed }
    }
}

impl RandomSource for Lcg32 {
    fn next_u32(&mut self) -> u32 {
        self.state = self.state.wrapping_mul(1664525).wrapping_add(1013904223);
        self.state
    }
}

/// Active Tetris-like piece.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Piece {
    /// Shape.
    pub kind: PieceKind,
    /// Board-space x position.
    pub x: i16,
    /// Board-space y position.
    pub y: i16,
    /// Rotation index.
    pub rotation: u8,
    /// Display color.
    pub color: Rgb8,
}

impl Piece {
    /// Creates a piece at the given board position.
    pub const fn new(kind: PieceKind, x: i16, y: i16, color: Rgb8) -> Self {
        Self {
            kind,
            x,
            y,
            rotation: 0,
            color,
        }
    }

    pub fn rotate(&mut self, dir: Rotation) {
        self.rotation = match dir {
            Rotation::Cw => (self.rotation + 1) & 3,
            Rotation::Ccw => (self.rotation + 3) & 3,
        };
    }

    pub(crate) fn cells(&self) -> [Point; MAX_PIECE_CELLS] {
        rotated_cells(self.kind, self.rotation)
    }

    pub fn cell_count(&self) -> usize {
        cell_count(self.kind)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layer {
    Placed,
    Current,
    Ghost,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CellView {
    pub color: Rgb8,
    pub layer: Layer,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MoveSuggestion {
    pub target_x: i16,
    pub target_rotation: u8,
    pub score: i32,
}

#[derive(Clone)]
pub struct Game<const W: usize, const H: usize, const HIDDEN: usize>
where
    [(); W]:,
    [(); H]:,
{
    board: [[Option<Block>; H]; W],
    current: Piece,
    changed: bool,
    game_over: bool,
    pub level: LevelState,
}

impl<const W: usize, const H: usize, const HIDDEN: usize> Game<W, H, HIDDEN>
where
    [(); W]:,
    [(); H]:,
{
    pub fn new<R: RandomSource, T: TetrisEvents>(rng: &mut R, events: &mut T) -> Self {
        debug_assert!(HIDDEN < H);
        let mut game = Self {
            board: [[None; H]; W],
            current: Piece::new(PieceKind::Line, (W / 2) as i16, 0, Rgb8::new(255, 255, 255)),
            changed: true,
            game_over: false,
            level: LevelState::new(),
        };
        game.spawn_piece(rng, events);
        game
    }

    pub const fn width(&self) -> usize {
        W
    }

    pub const fn height_total(&self) -> usize {
        H
    }

    pub const fn hidden_rows(&self) -> usize {
        HIDDEN
    }

    pub const fn visible_height(&self) -> usize {
        H - HIDDEN
    }

    pub fn has_changed(&self) -> bool {
        self.changed
    }

    pub fn consume_changed(&mut self) -> bool {
        let changed = self.changed;
        self.changed = false;
        changed
    }

    pub fn is_game_over(&self) -> bool {
        self.game_over
    }

    pub fn current_piece(&self) -> Piece {
        self.current
    }

    pub fn soft_drop<R: RandomSource, T: TetrisEvents>(
        &mut self,
        rng: &mut R,
        events: &mut T,
    ) -> TickResult {
        if self.game_over {
            return TickResult::GameOver;
        }

        let mut moved = self.current;
        moved.y += 1;
        if self.can_place(moved) {
            self.current = moved;
            self.changed = true;
            return TickResult::Moved;
        }

        let game_over = self.lock_current_piece(events);
        if game_over {
            self.game_over = true;
            events.on_game_over();
            self.changed = true;
            return TickResult::GameOver;
        }

        self.spawn_piece(rng, events);
        self.changed = true;
        TickResult::Locked
    }

    pub fn hard_drop<R: RandomSource, T: TetrisEvents>(
        &mut self,
        rng: &mut R,
        events: &mut T,
    ) -> TickResult {
        if self.game_over {
            return TickResult::GameOver;
        }
        while self.can_drop() {
            self.current.y += 1;
        }
        let game_over = self.lock_current_piece(events);
        if game_over {
            self.game_over = true;
            events.on_game_over();
            self.changed = true;
            return TickResult::GameOver;
        }
        self.spawn_piece(rng, events);
        self.changed = true;
        TickResult::Locked
    }

    pub fn move_left(&mut self) -> bool {
        let mut moved = self.current;
        moved.x -= 1;
        if self.can_place(moved) {
            self.current = moved;
            self.changed = true;
            return true;
        }
        false
    }

    pub fn move_right(&mut self) -> bool {
        let mut moved = self.current;
        moved.x += 1;
        if self.can_place(moved) {
            self.current = moved;
            self.changed = true;
            return true;
        }
        false
    }

    pub fn rotate(&mut self, dir: Rotation) -> bool {
        let mut turned = self.current;
        turned.rotate(dir);
        if self.can_place(turned) {
            self.current = turned;
            self.changed = true;
            return true;
        }
        false
    }

    pub fn can_drop(&self) -> bool {
        let mut moved = self.current;
        moved.y += 1;
        self.can_place(moved)
    }

    pub fn ghost_piece(&self) -> Piece {
        let mut ghost = self.current;
        while {
            let mut trial = ghost;
            trial.y += 1;
            self.can_place(trial)
        } {
            ghost.y += 1;
        }
        ghost
    }

    pub fn cell_view_at(&self, x: usize, y: usize, include_ghost: bool) -> Option<CellView> {
        if x >= W || y >= H {
            return None;
        }

        if let Some(block) = self.board[x][y] {
            return Some(CellView {
                color: block.color,
                layer: Layer::Placed,
            });
        }

        if include_ghost {
            let ghost = self.ghost_piece();
            if piece_has_cell_at(ghost, x as i16, y as i16) {
                return Some(CellView {
                    color: ghost.color,
                    layer: Layer::Ghost,
                });
            }
        }

        if piece_has_cell_at(self.current, x as i16, y as i16) {
            return Some(CellView {
                color: self.current.color,
                layer: Layer::Current,
            });
        }

        None
    }

    pub fn suggest_best_move(&self) -> Option<MoveSuggestion> {
        if self.game_over {
            return None;
        }

        let mut best: Option<MoveSuggestion> = None;
        let mut best_score = i32::MIN;

        for rot in 0..4 {
            for target_x in -4_i16..(W as i16 + 4_i16) {
                let mut candidate = self.current;
                candidate.rotation = rot;
                candidate.x = target_x;
                if !self.can_place(candidate) {
                    continue;
                }

                while {
                    let mut trial = candidate;
                    trial.y += 1;
                    self.can_place(trial)
                } {
                    candidate.y += 1;
                }

                let score = self.heuristic_if_locked(candidate);
                if score > best_score {
                    best_score = score;
                    best = Some(MoveSuggestion {
                        target_x,
                        target_rotation: rot,
                        score,
                    });
                }
            }
        }
        best
    }

    fn spawn_piece<R: RandomSource, T: TetrisEvents>(&mut self, rng: &mut R, events: &mut T) {
        let allow_evil = self.level.features.contains(Feature::EvilBlocks);
        let kind = if allow_evil {
            PieceKind::ALL[(rng.next_u32() as usize) % PieceKind::ALL.len()]
        } else {
            PieceKind::NON_EVIL[(rng.next_u32() as usize) % PieceKind::NON_EVIL.len()]
        };

        let color = color_for_piece(kind, rng);
        let spawn_x = (W / 2) as i16 - 1;
        let spawn_y = HIDDEN as i16 - 4;
        self.current = Piece::new(kind, spawn_x, spawn_y, color);
        if !self.can_place(self.current) {
            self.game_over = true;
            events.on_game_over();
        }
    }

    fn can_place(&self, piece: Piece) -> bool {
        let cells = piece.cells();
        let count = piece.cell_count();
        for point in cells.iter().take(count) {
            let x = piece.x + point.x;
            let y = piece.y + point.y;
            if x < 0 || y < 0 {
                return false;
            }
            let ux = x as usize;
            let uy = y as usize;
            if ux >= W || uy >= H {
                return false;
            }
            if self.board[ux][uy].is_some() {
                return false;
            }
        }
        true
    }

    fn lock_current_piece<T: TetrisEvents>(&mut self, events: &mut T) -> bool {
        self.level.on_piece_placed();

        let mut game_over = false;
        let cells = self.current.cells();
        let count = self.current.cell_count();
        for point in cells.iter().take(count) {
            let x = self.current.x + point.x;
            let y = self.current.y + point.y;
            if x < 0 || y < 0 {
                game_over = true;
                continue;
            }
            let ux = x as usize;
            let uy = y as usize;
            if ux >= W || uy >= H {
                game_over = true;
                continue;
            }

            self.board[ux][uy] = Some(Block {
                color: self.current.color,
            });
            events.on_block_placed(self.current.color, ux, uy);

            if uy < HIDDEN {
                game_over = true;
            }
        }

        let cleared = self.clear_full_rows(events);
        if cleared > 0 {
            self.level.on_rows_deleted(cleared, events);
        }
        game_over
    }

    fn clear_full_rows<T: TetrisEvents>(&mut self, events: &mut T) -> u32 {
        let mut deleted = 0_u32;
        let mut y = 0_usize;

        while y < H {
            let mut full = true;
            for x in 0..W {
                if self.board[x][y].is_none() {
                    full = false;
                    break;
                }
            }

            if full {
                deleted += 1;
                let mut colors = [None; W];
                for x in 0..W {
                    colors[x] = self.board[x][y].map(|block| block.color);
                }
                events.on_row_deleted(y, &colors);

                for src_y in (1..=y).rev() {
                    for x in 0..W {
                        self.board[x][src_y] = self.board[x][src_y - 1];
                    }
                }
                for x in 0..W {
                    self.board[x][0] = None;
                }
            } else {
                y += 1;
            }
        }
        deleted
    }

    fn heuristic_if_locked(&self, piece: Piece) -> i32 {
        let mut board = self.board;
        let cells = piece.cells();
        let count = piece.cell_count();
        for point in cells.iter().take(count) {
            let x = piece.x + point.x;
            let y = piece.y + point.y;
            if x < 0 || y < 0 {
                return i32::MIN / 4;
            }
            let ux = x as usize;
            let uy = y as usize;
            if ux >= W || uy >= H {
                return i32::MIN / 4;
            }
            board[ux][uy] = Some(Block { color: piece.color });
        }

        let mut lines = 0_i32;
        for y in 0..H {
            let mut full = true;
            for x in 0..W {
                if board[x][y].is_none() {
                    full = false;
                    break;
                }
            }
            if full {
                lines += 1;
            }
        }

        let mut heights = [0_i32; W];
        let mut holes = 0_i32;

        for x in 0..W {
            let mut seen = false;
            for y in 0..H {
                if board[x][y].is_some() {
                    if !seen {
                        heights[x] = (H - y) as i32;
                        seen = true;
                    }
                } else if seen {
                    holes += 1;
                }
            }
        }

        let mut bumpiness = 0_i32;
        for x in 0..(W.saturating_sub(1)) {
            bumpiness += (heights[x] - heights[x + 1]).abs();
        }
        let aggregate_height: i32 = heights.iter().copied().sum();

        lines * 1000 - holes * 50 - bumpiness * 6 - aggregate_height * 2
    }
}

fn piece_has_cell_at(piece: Piece, x: i16, y: i16) -> bool {
    let cells = piece.cells();
    for point in cells.iter().take(piece.cell_count()) {
        if piece.x + point.x == x && piece.y + point.y == y {
            return true;
        }
    }
    false
}

fn shade_channel(base: u8, step: u8) -> u8 {
    let factor = 100_u16.saturating_sub(step as u16 * 5);
    ((base as u16 * factor) / 100) as u8
}

fn color_for_piece<R: RandomSource>(kind: PieceKind, rng: &mut R) -> Rgb8 {
    let base = match kind {
        PieceKind::Line => (0_u8, 0_u8, 255_u8),
        PieceKind::ZigZag => (0_u8, 255_u8, 0_u8),
        PieceKind::L => (255_u8, 0_u8, 0_u8),
        PieceKind::Square => (255_u8, 255_u8, 0_u8),
        PieceKind::Tee => (255_u8, 0_u8, 255_u8),
        PieceKind::Dot => (0_u8, 255_u8, 255_u8),
        PieceKind::J => (255_u8, 128_u8, 0_u8),
        PieceKind::Evil => (200_u8, 200_u8, 200_u8),
    };
    let shade = (rng.next_u32() % 6) as u8;
    Rgb8::new(
        shade_channel(base.0, shade),
        shade_channel(base.1, shade),
        shade_channel(base.2, shade),
    )
}

const LINE_CELLS: [Point; MAX_PIECE_CELLS] = [
    Point::new(1, 0),
    Point::new(1, 1),
    Point::new(1, 2),
    Point::new(1, 3),
    Point::new(0, 0),
    Point::new(0, 0),
    Point::new(0, 0),
    Point::new(0, 0),
];
const ZIGZAG_CELLS: [Point; MAX_PIECE_CELLS] = [
    Point::new(0, 0),
    Point::new(0, 1),
    Point::new(1, 1),
    Point::new(2, 1),
    Point::new(0, 0),
    Point::new(0, 0),
    Point::new(0, 0),
    Point::new(0, 0),
];
const L_CELLS: [Point; MAX_PIECE_CELLS] = [
    Point::new(0, 0),
    Point::new(0, 1),
    Point::new(0, 2),
    Point::new(1, 2),
    Point::new(0, 0),
    Point::new(0, 0),
    Point::new(0, 0),
    Point::new(0, 0),
];
const SQUARE_CELLS: [Point; MAX_PIECE_CELLS] = [
    Point::new(0, 0),
    Point::new(1, 0),
    Point::new(0, 1),
    Point::new(1, 1),
    Point::new(0, 0),
    Point::new(0, 0),
    Point::new(0, 0),
    Point::new(0, 0),
];
const TEE_CELLS: [Point; MAX_PIECE_CELLS] = [
    Point::new(1, 0),
    Point::new(0, 1),
    Point::new(1, 1),
    Point::new(2, 1),
    Point::new(0, 0),
    Point::new(0, 0),
    Point::new(0, 0),
    Point::new(0, 0),
];
const DOT_CELLS: [Point; MAX_PIECE_CELLS] = [
    Point::new(0, 0),
    Point::new(0, 0),
    Point::new(0, 0),
    Point::new(0, 0),
    Point::new(0, 0),
    Point::new(0, 0),
    Point::new(0, 0),
    Point::new(0, 0),
];
const J_CELLS: [Point; MAX_PIECE_CELLS] = [
    Point::new(1, 0),
    Point::new(1, 1),
    Point::new(1, 2),
    Point::new(0, 2),
    Point::new(0, 0),
    Point::new(0, 0),
    Point::new(0, 0),
    Point::new(0, 0),
];
const EVIL_CELLS: [Point; MAX_PIECE_CELLS] = [
    Point::new(0, 0),
    Point::new(1, 0),
    Point::new(2, 0),
    Point::new(0, 1),
    Point::new(2, 1),
    Point::new(0, 2),
    Point::new(1, 2),
    Point::new(2, 2),
];

fn cell_count(kind: PieceKind) -> usize {
    match kind {
        PieceKind::Evil => 8,
        PieceKind::Dot => 1,
        _ => 4,
    }
}

fn base_cells(kind: PieceKind) -> [Point; MAX_PIECE_CELLS] {
    match kind {
        PieceKind::Line => LINE_CELLS,
        PieceKind::ZigZag => ZIGZAG_CELLS,
        PieceKind::L => L_CELLS,
        PieceKind::Square => SQUARE_CELLS,
        PieceKind::Tee => TEE_CELLS,
        PieceKind::Dot => DOT_CELLS,
        PieceKind::J => J_CELLS,
        PieceKind::Evil => EVIL_CELLS,
    }
}

fn rotated_cells(kind: PieceKind, rotation: u8) -> [Point; MAX_PIECE_CELLS] {
    let mut out = base_cells(kind);
    let count = cell_count(kind);
    let turns = rotation & 3;
    if turns == 0 || matches!(kind, PieceKind::Square | PieceKind::Dot) {
        return out;
    }

    for _ in 0..turns {
        for point in out.iter_mut().take(count) {
            let x = point.x;
            let y = point.y;
            point.x = 3 - y;
            point.y = x;
        }
    }

    normalize_points(&mut out, count);
    out
}

fn normalize_points(points: &mut [Point; MAX_PIECE_CELLS], count: usize) {
    let mut min_x = i16::MAX;
    let mut min_y = i16::MAX;
    for point in points.iter().take(count) {
        min_x = min(min_x, point.x);
        min_y = min(min_y, point.y);
    }
    for point in points.iter_mut().take(count) {
        point.x -= min_x;
        point.y -= min_y;
    }
}
