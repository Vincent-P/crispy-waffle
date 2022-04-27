use std::env;
use std::error::Error;
use std::process::{Command, ExitStatus};

fn main() -> Result<(), Box<dyn Error>> {
    let out_dir = env::var("OUT_DIR").unwrap();

    //println!("cargo:rerun-if-changed=shaders");

    let status = Command::new("glslc")
        .args(&["shaders/base.vert.glsl", "-o"])
        .arg(&format!("{}/base.vert.spv", out_dir))
        .status()
        .expect("failed to execute glslc");

    if !status.success() {
        eprintln!("Failed to compile base.vert");
        eprintln!("{}", status);
        std::process::exit(1);
    }

    let status = Command::new("glslc")
        .args(&["shaders/base.frag.glsl", "-o"])
        .arg(&format!("{}/base.frag.spv", out_dir))
        .status()
        .expect("failed to execute glslc");

    if !status.success() {
        eprintln!("Failed to compile base.frag");
        eprintln!("{}", status);
        std::process::exit(1);
    }

    Ok(())
}
