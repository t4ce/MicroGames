use std::io::{self, Write};

use microgames::shell::{ShellControl, ShellIo};

struct StdoutIo;

impl ShellIo for StdoutIo {
    fn write_str(&self, s: &str) {
        print!("{s}");
    }

    fn write_fmt(&self, args: core::fmt::Arguments<'_>) {
        print!("{args}");
    }
}

fn main() -> io::Result<()> {
    let io = StdoutIo;
    let mut app = microgames::shell::ShellApp::new(0xC11C_7E75, 120, 32);
    app.set_terminal_size(120, 32);
    app.set_viewport_top_row(1);

    print!("\x1b[2J\x1b[H\x1b[?25l");
    app.draw(&io);
    app.finalize_frame();
    io::stdout().flush()?;

    let mut line = String::new();
    loop {
        print!(
            "\nwasd/hjkl move, w/k rotate, z rotate back, space hard-drop, p pause, r reset, q quit > "
        );
        io::stdout().flush()?;

        line.clear();
        if io::stdin().read_line(&mut line)? == 0 {
            break;
        }

        for byte in line.bytes() {
            if matches!(app.handle_input_byte(byte), ShellControl::Exit) {
                print!("\x1b[?25h\x1b[0m\x1b[2J\x1b[H");
                io::stdout().flush()?;
                return Ok(());
            }
        }

        app.tick(160);
        if app.consume_redraw() {
            app.draw(&io);
            app.finalize_frame();
            io::stdout().flush()?;
        }
    }

    print!("\x1b[?25h\x1b[0m\x1b[2J\x1b[H");
    io::stdout().flush()
}
