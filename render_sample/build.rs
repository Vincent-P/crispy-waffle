use std::env;
use std::path::Path;
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

fn copy_resource(filename: &str, out_dir: &str) -> std::io::Result<u64> {
    let input_path = Path::new("resources").join(filename);
    let output_path = Path::new(out_dir).join(filename);

    let res = std::fs::copy(&input_path, &output_path);
    println!("cargo:warning={:?} -> {:?}", &input_path, &output_path);
    res
}

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();

    println!("cargo:rerun-if-changed=shaders");

    compile_shader("base.vert", &out_dir);
    compile_shader("base.frag", &out_dir);
    compile_shader("ui.vert", &out_dir);
    compile_shader("ui.frag", &out_dir);

    copy_resource("iAWriterQuattroS-Regular.ttf", &out_dir).unwrap();
}
