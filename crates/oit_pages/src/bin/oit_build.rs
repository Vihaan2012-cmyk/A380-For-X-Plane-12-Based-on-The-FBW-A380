use std::io::Read;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut input = String::new();
    if args.len() > 1 {
        input = std::fs::read_to_string(&args[1]).expect("read model");
    } else {
        std::io::stdin().read_to_string(&mut input).expect("read stdin");
    }
    match oit_pages::build_json(&input) {
        Ok(json) => {
            if args.len() > 2 {
                std::fs::write(&args[2], json).expect("write tree");
            } else {
                println!("{json}");
            }
        }
        Err(e) => {
            eprintln!("bad model: {e}");
            std::process::exit(2);
        }
    }
}
