/* a tiny multiplayer game on the loop SDK: two circles move around a board;
clients send moves over REST and every client watches the shared board
through a Live (WebSocket) endpoint pushing ~20 frames per second */

use std::sync::{Arc, Mutex};
use std::time::Duration;

use http::Method;
use lib::endpoint::engine::Engine;
use lib::endpoint::{Access, Binding, Endpoint, HandlerError, Parameter, Signature, ValueStream};
use lib::schema::{Primitive, Schema, Value};

const BOARD: f64 = 400.0;
const RADIUS: f64 = 14.0;
const FRAME: Duration = Duration::from_millis(50);

#[derive(Clone)]
struct Board {
    players: [(f64, f64); 2],
}

type SharedBoard = Arc<Mutex<Board>>;

fn main() {
    let board = SharedBoard::new(Mutex::new(Board {
        players: [(80.0, 80.0), (320.0, 320.0)],
    }));

    let endpoints = vec![move_endpoint(board.clone()), watch_endpoint(board)];

    let engine = Engine::new(endpoints).expect("invalid endpoint definitions");
    println!("circle game listening on http://127.0.0.1:3000");
    for route in lib::server::routes(&engine) {
        println!("  {route}");
    }
    println!("open examples/circle-game/index.html in a browser to play");
    lib::server::serve_blocking(engine, ("127.0.0.1", 3000)).expect("server failed");
}

// GET /move/{player}?dx=..&dy=.. -> true (GET keeps browser fetch preflight-free)
fn move_endpoint(board: SharedBoard) -> Endpoint {
    Endpoint {
        name: "move".into(),
        signature: Signature {
            params: vec![
                param("player", Primitive::U32),
                param("dx", Primitive::F64),
                param("dy", Primitive::F64),
            ],
            output: Schema::Primitive(Primitive::Bool),
        },
        access: Access::Rest {
            method: Method::GET,
            url: "/move/{player}".into(),
        },
        binding: Binding::Native(Arc::new(move |args: &[Value]| {
            let [Value::U32(player), Value::F64(dx), Value::F64(dy)] = args else {
                return Err("expected player, dx, dy".into());
            };
            let index = match player {
                1 => 0,
                2 => 1,
                _ => return Err("player must be 1 or 2".into()),
            };

            let mut board = board.lock().unwrap();
            let (x, y) = board.players[index];
            board.players[index] = (
                (x + dx).clamp(RADIUS, BOARD - RADIUS),
                (y + dy).clamp(RADIUS, BOARD - RADIUS),
            );
            Ok(Value::Bool(true))
        })),
    }
}

// LIVE /watch -> {"p1": [x, y], "p2": [x, y]} every frame
fn watch_endpoint(board: SharedBoard) -> Endpoint {
    Endpoint {
        name: "watch".into(),
        signature: Signature {
            params: vec![],
            output: Schema::Map(
                Box::new(Schema::Primitive(Primitive::Str)),
                Box::new(Schema::List(Box::new(Schema::Primitive(Primitive::F64)))),
            ),
        },
        access: Access::Live {
            url: "/watch".into(),
        },
        binding: Binding::Stream(Arc::new(
            move |_: &[Value]| -> Result<ValueStream, HandlerError> {
                let board = board.clone();
                Ok(Box::new(std::iter::repeat_with(move || {
                    std::thread::sleep(FRAME);
                    Ok(frame(&board.lock().unwrap()))
                })))
            },
        )),
    }
}

fn frame(board: &Board) -> Value {
    let position = |(x, y): (f64, f64)| Value::List(vec![Value::F64(x), Value::F64(y)]);
    Value::Map(vec![
        (Value::Str("p1".into()), position(board.players[0])),
        (Value::Str("p2".into()), position(board.players[1])),
    ])
}

fn param(name: &str, primitive: Primitive) -> Parameter {
    Parameter {
        name: name.into(),
        schema: Schema::Primitive(primitive),
    }
}
