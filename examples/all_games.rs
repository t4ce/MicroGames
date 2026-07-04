use microgames::bejewled;
use microgames::chess::{Move, Square};
use microgames::minesweeper;
use microgames::snake;
use microgames::{Lcg32, NoopEvents, Rotation};

fn main() {
    let mut rng = Lcg32::new(0xA11_6A4E5);

    let mut tetris_events = NoopEvents;
    let mut tetris = microgames::Game::<10, 24, 4>::new(&mut rng, &mut tetris_events);
    tetris.move_left();
    tetris.rotate(Rotation::Cw);
    let tetris_tick = tetris.soft_drop(&mut rng, &mut tetris_events);
    let tetris_cell = tetris.cell_view_at(4, 23, true);

    let mut snake_events = snake::NoopEvents;
    let mut snake = snake::Game::<12, 8>::new(&mut rng, &mut snake_events);
    let _ = snake.set_direction(snake::Direction::Right);
    let snake_tick = snake.tick(&mut rng, &mut snake_events);

    let mut mine_events = minesweeper::NoopEvents;
    let mut mines = minesweeper::Game::<8, 8>::new(minesweeper::Config::new(10)).unwrap();
    let mine_reveal = mines.reveal(0, 0, &mut rng, &mut mine_events);

    let mut jewel_events = bejewled::NoopEvents;
    let jewels = bejewled::Game::<8, 8>::new(&mut rng, &mut jewel_events);

    let mut chess = microgames::chess::Game::new();
    let chess_move = chess
        .apply_move(Move::new(square(4, 1), square(4, 3)))
        .unwrap();

    println!("microgames all-engines smoke");
    println!(
        "tetris: {:?}, visible_cell={}",
        tetris_tick,
        tetris_cell.is_some()
    );
    println!(
        "snake: {:?}, score={}, head={:?}",
        snake_tick,
        snake.score(),
        snake.head()
    );
    println!(
        "minesweeper: {:?}, revealed={}",
        mine_reveal.state, mine_reveal.revealed_cells
    );
    println!(
        "bejewled: score={}, gem00={:?}",
        jewels.score(),
        jewels.gem_at(0, 0)
    );
    println!(
        "chess: moved={:?}, side_to_move={:?}",
        chess_move.moved_piece,
        chess.side_to_move()
    );
}

fn square(file: u8, rank: u8) -> Square {
    Square::new(file, rank).unwrap()
}
