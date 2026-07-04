use microgames::shell::{ShellControl, ShellIo};
use microgames::{Lcg32, snake};

const COLS: usize = 120;
const ROWS: usize = 32;
const SNAKE_W: usize = 22;
const SNAKE_H: usize = 16;
const SNAKE_STEP_MS: u32 = 120;

struct StdoutIo;

impl ShellIo for StdoutIo {
    fn write_str(&self, s: &str) {
        std::print!("{s}");
    }

    fn write_fmt(&self, args: core::fmt::Arguments<'_>) {
        std::print!("{args}");
    }
}

fn main() -> std::io::Result<()> {
    let _terminal = TerminalMode::enter()?;
    let io = StdoutIo;
    let mut app = ArcadeApp::new(COLS, ROWS);

    std::print!("\x1b[2J\x1b[H\x1b[?25l");
    app.draw(&io);
    flush_stdout()?;

    let mut input = std::io::stdin();
    let mut bytes = [0_u8; 64];
    loop {
        let count = read_available(&mut input, &mut bytes)?;
        for byte in bytes.iter().copied().take(count) {
            if matches!(app.handle_input_byte(byte), ShellControl::Exit) {
                return Ok(());
            }
        }

        app.tick(16);
        if app.consume_redraw() {
            app.draw(&io);
            flush_stdout()?;
        }

        std::thread::sleep(std::time::Duration::from_millis(16));
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Tetris,
    Snake,
}

struct ArcadeApp {
    mode: Mode,
    cols: usize,
    rows: usize,
    tetris: microgames::shell::ShellApp,
    snake: SnakeApp,
    redraw: bool,
}

impl ArcadeApp {
    fn new(cols: usize, rows: usize) -> Self {
        let mut tetris = microgames::shell::ShellApp::new(0xC11C_7E75, cols, rows);
        tetris.set_terminal_size(cols, rows);
        tetris.set_viewport_top_row(1);

        Self {
            mode: Mode::Tetris,
            cols,
            rows,
            tetris,
            snake: SnakeApp::new(0x51A4_EA7E),
            redraw: true,
        }
    }

    fn handle_input_byte(&mut self, b: u8) -> ShellControl {
        match b {
            b'1' => {
                self.switch_to(Mode::Tetris);
                return ShellControl::Continue;
            }
            b'2' => {
                self.switch_to(Mode::Snake);
                return ShellControl::Continue;
            }
            b'q' | b'Q' | 0x03 | 0x04 => {
                return ShellControl::Exit;
            }
            _ => {}
        }

        match self.mode {
            Mode::Tetris => self.tetris.handle_input_byte(b),
            Mode::Snake => {
                self.snake.handle_input_byte(b);
                ShellControl::Continue
            }
        }
    }

    fn tick(&mut self, elapsed_ms: u32) {
        match self.mode {
            Mode::Tetris => self.tetris.tick(elapsed_ms),
            Mode::Snake => self.snake.tick(elapsed_ms),
        }
    }

    fn consume_redraw(&mut self) -> bool {
        let redraw = self.redraw
            || match self.mode {
                Mode::Tetris => self.tetris.consume_redraw(),
                Mode::Snake => self.snake.consume_redraw(),
            };
        self.redraw = false;
        redraw
    }

    fn draw(&mut self, io: &dyn ShellIo) {
        match self.mode {
            Mode::Tetris => {
                self.tetris.draw(io);
                self.tetris.finalize_frame();
                self.draw_mode_hint(io, "Tetris");
            }
            Mode::Snake => {
                self.snake.draw(io, self.cols, self.rows);
                self.draw_mode_hint(io, "Snake");
            }
        }
    }

    fn switch_to(&mut self, mode: Mode) {
        if self.mode != mode {
            self.mode = mode;
            self.redraw = true;
            match mode {
                Mode::Tetris => self.tetris.set_terminal_size(self.cols, self.rows),
                Mode::Snake => self.snake.force_redraw(),
            }
            std::print!("\x1b[2J\x1b[H");
        }
    }

    fn draw_mode_hint(&self, io: &dyn ShellIo, name: &str) {
        io.write_fmt(format_args!(
            "\x1b[{};2H[1] tetris  [2] snake  current: {name}  q quit\x1b[K",
            self.rows
        ));
    }
}

struct SnakeApp {
    game: snake::Game<SNAKE_W, SNAKE_H>,
    rng: Lcg32,
    events: snake::NoopEvents,
    tick_accum_ms: u32,
    esc_state: u8,
    redraw: bool,
}

impl SnakeApp {
    fn new(seed: u32) -> Self {
        let mut rng = Lcg32::new(seed);
        let mut events = snake::NoopEvents;
        let game = snake::Game::new(&mut rng, &mut events);

        Self {
            game,
            rng,
            events,
            tick_accum_ms: 0,
            esc_state: 0,
            redraw: true,
        }
    }

    fn handle_input_byte(&mut self, b: u8) {
        if self.esc_state == 1 {
            self.esc_state = if b == b'[' { 2 } else { 0 };
            return;
        }

        if self.esc_state == 2 {
            self.esc_state = 0;
            match b {
                b'A' => self.set_direction(snake::Direction::Up),
                b'B' => self.set_direction(snake::Direction::Down),
                b'C' => self.set_direction(snake::Direction::Right),
                b'D' => self.set_direction(snake::Direction::Left),
                _ => {}
            }
            return;
        }

        match b {
            0x1B => {
                self.esc_state = 1;
            }
            b'r' | b'R' => {
                self.game.reset(&mut self.rng, &mut self.events);
                self.tick_accum_ms = 0;
                self.redraw = true;
            }
            b'w' | b'k' | b'K' => self.set_direction(snake::Direction::Up),
            b's' | b'j' | b'J' => self.set_direction(snake::Direction::Down),
            b'a' | b'h' | b'H' => self.set_direction(snake::Direction::Left),
            b'd' | b'l' | b'L' => self.set_direction(snake::Direction::Right),
            _ => {}
        }
    }

    fn tick(&mut self, elapsed_ms: u32) {
        if !matches!(self.game.state(), snake::GameState::Running) {
            return;
        }

        self.tick_accum_ms = self.tick_accum_ms.saturating_add(elapsed_ms);
        while self.tick_accum_ms >= SNAKE_STEP_MS {
            self.tick_accum_ms -= SNAKE_STEP_MS;
            let _ = self.game.tick(&mut self.rng, &mut self.events);
        }
    }

    fn consume_redraw(&mut self) -> bool {
        let redraw = self.redraw || self.game.consume_changed();
        self.redraw = false;
        redraw
    }

    fn force_redraw(&mut self) {
        self.redraw = true;
    }

    fn set_direction(&mut self, direction: snake::Direction) {
        if self.game.set_direction(direction) {
            self.redraw = true;
        }
    }

    fn draw(&self, io: &dyn ShellIo, cols: usize, rows: usize) {
        let board_cols = SNAKE_W * 2 + 2;
        let board_rows = SNAKE_H + 2;
        let panel_cols = board_cols + 24;
        let start_col = (cols.saturating_sub(panel_cols) / 2).max(1);
        let start_row = (rows.saturating_sub(board_rows) / 2).max(1);
        let stats_col = start_col + board_cols + 3;

        io.write_fmt(format_args!("\x1b[{};1H\x1b[J\x1b[?25l", start_row));
        self.draw_board(io, start_row, start_col);
        self.draw_stats(io, start_row, stats_col);
    }

    fn draw_board(&self, io: &dyn ShellIo, start_row: usize, start_col: usize) {
        io.write_fmt(format_args!("\x1b[{};{}H+", start_row, start_col));
        for _ in 0..SNAKE_W {
            io.write_str("--");
        }
        io.write_str("+");

        for y in 0..SNAKE_H {
            io.write_fmt(format_args!("\x1b[{};{}H|", start_row + y + 1, start_col));
            for x in 0..SNAKE_W {
                match self.game.cell_kind_at(x, y) {
                    Some(snake::CellKind::Head) => io.write_str("\x1b[42m  \x1b[0m"),
                    Some(snake::CellKind::Body) => io.write_str("\x1b[102m  \x1b[0m"),
                    Some(snake::CellKind::Food) => io.write_str("\x1b[41m  \x1b[0m"),
                    _ => io.write_str("  "),
                }
            }
            io.write_str("|");
        }

        io.write_fmt(format_args!(
            "\x1b[{};{}H+",
            start_row + SNAKE_H + 1,
            start_col
        ));
        for _ in 0..SNAKE_W {
            io.write_str("--");
        }
        io.write_str("+");
    }

    fn draw_stats(&self, io: &dyn ShellIo, start_row: usize, stats_col: usize) {
        let state = match self.game.state() {
            snake::GameState::Running => "running",
            snake::GameState::Won => "won",
            snake::GameState::Lost => "lost",
        };
        let (head_x, head_y) = self.game.head();
        let food = self
            .game
            .food()
            .map(|(x, y)| (x as isize, y as isize))
            .unwrap_or((-1, -1));

        self.write_stat(io, start_row, stats_col, "Snake");
        self.write_stat(io, start_row + 2, stats_col, "wasd/hjkl move");
        self.write_stat(io, start_row + 3, stats_col, "arrows move");
        self.write_stat(io, start_row + 4, stats_col, "r reset");
        io.write_fmt(format_args!(
            "\x1b[{};{}Hscore {:>5}\x1b[K",
            start_row + 6,
            stats_col,
            self.game.score()
        ));
        io.write_fmt(format_args!(
            "\x1b[{};{}Hlength {:>4}\x1b[K",
            start_row + 7,
            stats_col,
            self.game.length()
        ));
        io.write_fmt(format_args!(
            "\x1b[{};{}Hhead {:>2},{:<2}\x1b[K",
            start_row + 8,
            stats_col,
            head_x,
            head_y
        ));
        io.write_fmt(format_args!(
            "\x1b[{};{}Hfood {:>2},{:<2}\x1b[K",
            start_row + 9,
            stats_col,
            food.0,
            food.1
        ));
        io.write_fmt(format_args!(
            "\x1b[{};{}Hstate {state}\x1b[K",
            start_row + 11,
            stats_col
        ));
    }

    fn write_stat(&self, io: &dyn ShellIo, row: usize, col: usize, text: &str) {
        io.write_fmt(format_args!("\x1b[{row};{col}H{text}\x1b[K"));
    }
}

fn read_available(input: &mut std::io::Stdin, bytes: &mut [u8]) -> std::io::Result<usize> {
    match std::io::Read::read(input, bytes) {
        Ok(count) => Ok(count),
        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => Ok(0),
        Err(error) => Err(error),
    }
}

fn flush_stdout() -> std::io::Result<()> {
    std::io::Write::flush(&mut std::io::stdout())
}

struct TerminalMode;

impl TerminalMode {
    fn enter() -> std::io::Result<Self> {
        run_stty(&["raw", "-echo", "min", "0", "time", "0"])?;
        Ok(Self)
    }
}

impl Drop for TerminalMode {
    fn drop(&mut self) {
        std::print!("\x1b[?25h\x1b[0m\x1b[2J\x1b[H");
        let _ = flush_stdout();
        let _ = run_stty(&["sane"]);
    }
}

fn run_stty(args: &[&str]) -> std::io::Result<()> {
    let status = std::process::Command::new("stty").args(args).status()?;
    if status.success() {
        Ok(())
    } else {
        Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "stty failed to configure terminal",
        ))
    }
}
