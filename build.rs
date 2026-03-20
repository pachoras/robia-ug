use std::process::Command;

fn main() {
    // Compile Sass to CSS using the `sass` command-line tool
    let status = Command::new("sass")
        .args(["src/static/css/styles.scss:src/static/css/styles.css"])
        .status()
        .expect("Failed to run sass. Is it installed? Run: npm install -g sass");

    if !status.success() {
        panic!("Sass compilation failed");
    }
}
