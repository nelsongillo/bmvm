use std::env;
use std::time::Duration;

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();

    for arg in args {
        match arg.parse::<u64>() {
            Ok(nanos) => {
                let duration = Duration::from_nanos(nanos);

                println!("{} ns = {:?}", nanos, duration);
            }
            Err(_) => eprintln!("Invalid input: '{}'. Please provide a valid number.", arg),
        }
    }
}
