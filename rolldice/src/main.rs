use structopt::StructOpt;
use rand::Rng;

#[derive(StructOpt)]
struct Config {
    #[structopt(short = "n", help = "Number of dice", default_value = "1")]
    number_of_dice: u16,

    #[structopt(short = "r", long = "rowsize", help = "Maximum dice per row", default_value = "8")]
    dice_per_row: u16,
}

fn main() {
    let config = Config::from_args();
    if config.dice_per_row == 0 {
        std::process::exit(1);
    }

    let mut rng = rand::thread_rng();
    let mut rolls: Vec<u32> = Vec::new();
    for _ in 0..config.number_of_dice {
        rolls.push((rng.gen::<u32>() % 6) + 1);
    }
    for group in rolls.chunks(config.dice_per_row as usize) {
        println!("{:?}", group);
    }
}
