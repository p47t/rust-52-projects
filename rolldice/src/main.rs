use rand::Rng;
use clap::Parser;

use rolldice::{multizip, RollResult};

#[derive(Parser, Debug)]
#[command(
    name = "rolldice",
    bin_name = "rolldice",
    about = "Rolls some numbers of 6 sided dice."
)]
struct Config {
    #[arg(short = 'n', help = "Number of dice", default_value = "1")]
    number_of_dice: u16,

    #[arg(
        short = 'r',
        long = "rowsize",
        help = "Maximum dice per row",
        default_value = "8"
    )]
    dice_per_row: u16,
}

fn print_row(rolls: &[RollResult]) {
    let formatted: Vec<_> = rolls.iter().map(|roll| format!("{}", roll)).collect();
    let iters: Vec<_> = formatted.iter().map(|s| s.lines()).collect();
    for line in multizip(iters) {
        println!("{}", line.as_slice().join(" "));
    }
}

fn main() {
    let config = Config::parse();
    if config.dice_per_row == 0 {
        eprintln!("--rowsize must be greater than 0");
        std::process::exit(1);
    }

    let mut rng = rand::thread_rng();
    let mut rolls = Vec::new();
    for _ in 0..config.number_of_dice {
        rolls.push(rng.gen());
    }

    for group in rolls.chunks(config.dice_per_row as usize) {
        print_row(group);
    }
}
