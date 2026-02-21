mod shared;

use mimalloc::MiMalloc;
use starbase::MainResult;
use std::env;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

#[tokio::main(flavor = "current_thread")]
async fn main() -> MainResult {
    shared::run_cli(env::args_os().collect()).await
}
