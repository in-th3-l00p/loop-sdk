/* a tiny multiplayer game on the loop SDK attribute UX: two circles move
around a board; moves arrive over REST and every client watches the shared
board through a Live (WebSocket) endpoint pushing ~20 frames per second */

use std::collections::BTreeMap;
use std::sync::Mutex;
use std::time::Duration;

use lib::prelude::*;

const BOARD: f64 = 400.0;
const RADIUS: f64 = 14.0;
const FRAME: Duration = Duration::from_millis(50);

static PLAYERS: Mutex<[(f64, f64); 2]> = Mutex::new([(80.0, 80.0), (320.0, 320.0)]);

#[rest(get, "/move/{player}")]
fn move_player(player: u32, dx: f64, dy: f64) -> Result<bool, HandlerError> {
    let index = match player {
        1 => 0,
        2 => 1,
        _ => return Err("player must be 1 or 2".into()),
    };

    let mut players = PLAYERS.lock().unwrap();
    let (x, y) = players[index];
    players[index] = (
        (x + dx).clamp(RADIUS, BOARD - RADIUS),
        (y + dy).clamp(RADIUS, BOARD - RADIUS),
    );
    Ok(true)
}

#[live("/watch")]
fn watch() -> Result<impl Iterator<Item = BTreeMap<String, Vec<f64>>>, HandlerError> {
    Ok(std::iter::repeat_with(|| {
        std::thread::sleep(FRAME);
        frame()
    }))
}

fn frame() -> BTreeMap<String, Vec<f64>> {
    let players = PLAYERS.lock().unwrap();
    BTreeMap::from([
        ("p1".to_string(), vec![players[0].0, players[0].1]),
        ("p2".to_string(), vec![players[1].0, players[1].1]),
    ])
}

fn main() {
    println!("open examples/circle-game/index.html in a browser to play");
    lib::server::run();
}
