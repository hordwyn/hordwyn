//! Server binary: `--role=all|web|worker`. Wires adapters into the `app` core.
#![deny(unsafe_code)]

fn main() {
    println!("server: not wired yet");
}

#[cfg(test)]
mod tests {
    #[test]
    fn main_runs() {
        super::main();
    }
}
