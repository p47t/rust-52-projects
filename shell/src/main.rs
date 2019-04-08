use rustyline::Editor;

fn main() {
    let mut rl = Editor::<()>::new();
    loop {
        if let Ok(cmdline) = rl.readline("> ") {
            print!("{}", cmdline);
        }
    }
}
