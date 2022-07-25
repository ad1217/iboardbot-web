use std::process::Command;

#[cfg(not(windows))]
const NPM_CMD: &str = "npm";

#[cfg(windows)]
const NPM_CMD: &str = "npm.cmd";

fn main() {
    match std::env::var("PROFILE").as_deref() {
        Ok("debug") => {
            println!("cargo:warning=Not building JS dependencies, as we are in debug mode")
        }
        Ok("release") => {
            Command::new(NPM_CMD).arg("clean-install").status().unwrap();
            Command::new(NPM_CMD)
                .args(["run", "build"])
                .status()
                .unwrap();
        }
        _ => (),
    }

    println!("cargo:rerun-if-changed=static/");
}
