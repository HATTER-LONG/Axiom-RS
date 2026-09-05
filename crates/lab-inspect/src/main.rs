//! JSON/stdio entry for the lab-inspect reference host.

use std::io::{self, BufReader};

use axiom_rs::runtime::Runtime;
use lab_inspect::register;

fn main() -> io::Result<()> {
    let runtime = Runtime::new();
    register(&runtime).expect("reference capabilities register once");
    axiom_stdio::serve(
        &runtime,
        BufReader::new(io::stdin()),
        io::stdout(),
        io::stderr(),
    )
}
