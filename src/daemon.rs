use std::thread;
use std::time::Duration;

pub fn run_daemon() -> Result<(), String> {
    eprintln!("rustbox-tmuxd started");
    eprintln!("static render state initialized");

    loop {
        thread::sleep(Duration::from_secs(60));
    }
}
