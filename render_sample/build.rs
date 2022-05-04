use std::env;
use std::process::Command;

fn compile_shader(shader_name: &str, out_dir: &str) {
    let src_path = format!("shaders/{}.glsl", shader_name);
    let dst_path = format!("{}/{}.spv", out_dir, shader_name);

    let status = Command::new("glslc")
        .args(&[&src_path, "-o", &dst_path])
        .status()
        .expect("failed to execute glslc");

    if !status.success() {
        eprintln!("Failed to compile {}", src_path);
        eprintln!("{}", status);
        std::process::exit(1);
    }

    println!(
        "cargo:warning={} -> $OUT_DIR/{}.spv",
        &src_path, shader_name
    );
}

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();

    println!("cargo:rerun-if-changed=shaders");

    compile_shader("base.vert", &out_dir);
    compile_shader("base.frag", &out_dir);
    compile_shader("ui.vert", &out_dir);
    compile_shader("ui.frag", &out_dir);
}
