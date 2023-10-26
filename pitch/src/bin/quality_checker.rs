use pitch::config::qcconfig;
use crate::qcconfig::QCConfiguration;
use clap::Parser;

fn main() {
    match pitch::check_quality(QCConfiguration::parse()) {
        Ok((t, d)) => print_results(t, d),
        Err(why) => println!("main() failed: {}", why),
    }
}

/// Prints the results of the quality check
fn print_results(test: bool, distance: f32) {
    if test {
        println!("Good quality: {}", distance);
    } else {
        println!("Bad quality: {}", distance);
    }
}
