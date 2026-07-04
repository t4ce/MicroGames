use microgames::shell::{ShellControl, ShellIo};

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
    let mut app = microgames::shell::ShellApp::new(0xC11C_7E75, 120, 32);
    app.set_terminal_size(120, 32);
    app.set_viewport_top_row(1);

    std::print!("\x1b[2J\x1b[H\x1b[?25l");
    app.draw(&io);
    app.finalize_frame();
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
            app.finalize_frame();
            flush_stdout()?;
        }

        std::thread::sleep(std::time::Duration::from_millis(16));
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
