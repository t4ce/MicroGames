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
    let io = StdoutIo;
    let mut app = microgames::shell::ShellApp::new(0xC11C_7E75, 120, 32);
    app.set_terminal_size(120, 32);
    app.set_viewport_top_row(1);

    std::print!("\x1b[2J\x1b[H\x1b[?25l");
    app.draw(&io);
    app.finalize_frame();
    flush_stdout()?;

    let mut line = std::string::String::new();
    loop {
        std::print!(
            "\nwasd/hjkl move, w/k rotate, z rotate back, space hard-drop, p pause, r reset, q quit > "
        );
        flush_stdout()?;

        line.clear();
        if std::io::stdin().read_line(&mut line)? == 0 {
            break;
        }

        for byte in line.bytes() {
            if matches!(app.handle_input_byte(byte), ShellControl::Exit) {
                std::print!("\x1b[?25h\x1b[0m\x1b[2J\x1b[H");
                flush_stdout()?;
                return Ok(());
            }
        }

        app.tick(160);
        if app.consume_redraw() {
            app.draw(&io);
            app.finalize_frame();
            flush_stdout()?;
        }
    }

    std::print!("\x1b[?25h\x1b[0m\x1b[2J\x1b[H");
    flush_stdout()
}

fn flush_stdout() -> std::io::Result<()> {
    std::io::Write::flush(&mut std::io::stdout())
}
