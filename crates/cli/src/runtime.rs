/* a fresh single-purpose tokio runtime, since db commands don't run inside
one (unlike `dev`/`build`, which just shell out to `cargo`) */

pub fn block_on<F: std::future::Future>(future: F) -> F::Output {
    tokio::runtime::Runtime::new()
        .expect("failed to start tokio runtime")
        .block_on(future)
}
