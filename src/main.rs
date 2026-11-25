mod app;
mod audio;
mod composition;
mod configuration;
mod entropy;
mod exhibition;
mod export;
mod interaction;
mod memory;
mod organisms;
mod platform;
mod renderer;
mod simulation;
mod typography;

use clap::Parser;
use configuration::Args;

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    args.validate()?;
    if args.render.is_some() || args.frames.is_some() {
        exhibition::export(args)
    } else {
        app::run(args)
    }
}
