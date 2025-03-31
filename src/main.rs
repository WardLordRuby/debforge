mod args;
mod forge;
use args::Args;
use forge::Forge;

fn main() -> std::io::Result<()> {
    Forge::from(Args::parse())?.forge()
}
