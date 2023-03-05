use clap::Parser;

mod cli;
mod md;

fn main() {
    let args = cli::Args::parse();
    println!("{:?}", args.mode);
    md::example().unwrap();
}
