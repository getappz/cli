use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "appz-server", version, about = "Appz development server daemon")]
struct Args {
    #[arg(long, default_value = "47831")]
    port: u16,
    #[arg(long)]
    socket: Option<String>,
}

#[tokio::main]
async fn main() {
    let _args = Args::parse();
    eprintln!("appz-server: not yet implemented");
}
