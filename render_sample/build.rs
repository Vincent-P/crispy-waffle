use std::env;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::process::Command;

fn create_ninja_build(shaders: &[&str], deps: &[&str]) -> std::io::Result<()> {
    println!("cargo:warning=BUILD SHADERS");

    let build_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("shaders")
        .join("build.ninja");

    let mut f = File::create(build_path)?;

    let out_dir = Path::new(&env::var("OUT_DIR").unwrap())
        .join("shaders")
        .into_os_string()
        .into_string()
        .unwrap();

    let in_dir = Path::new(&env!("CARGO_MANIFEST_DIR"))
        .join("shaders")
        .into_os_string()
        .into_string()
        .unwrap();

    f.write_all(format!("outdir = {}\n", out_dir).as_bytes())?;
    f.write_all(format!("indir = {}\n", in_dir).as_bytes())?;

    f.write_all(
        r#"
rule spv
  command = glslc $in -o $out "#
            .as_bytes(),
    )?;

    f.write_all(b"-I ./include/ ")?;
    for dep in deps {
        f.write_all(format!("-I ../../{}/shaders/include ", dep).as_bytes())?;
    }

    f.write_all(b"\n\n")?;

    for shader in shaders {
        let src_path = format!("$indir/{}.glsl", shader);
        let dst_path = format!("$outdir/{}.spv", shader);
        f.write_all(format!("build {}: spv {}\n", dst_path, src_path).as_bytes())?;
    }

    Ok(())
}

fn copy_resource(filename: &str) -> std::io::Result<u64> {
    let out_dir = env::var("OUT_DIR").unwrap();
    let input_path = Path::new("resources").join(filename);
    let output_path = Path::new(&out_dir).join(filename);

    let res = std::fs::copy(&input_path, &output_path);
    println!("cargo:warning={:?} -> {:?}", &input_path, &output_path);
    res
}

fn main() {
    println!("cargo:rerun-if-changed=shaders");
    let shader_deps = ["render"];
    let shaders = ["ui.vert", "ui.frag"];

    create_ninja_build(&shaders, &shader_deps).expect("failed to generate ninja build file");

    let success = Command::new("ninja")
        .args(["-C", "shaders"])
        .status()
        .expect("failed to execute process")
        .success();
    assert!(success);

    copy_resource("iAWriterQuattroS-Regular.ttf").unwrap();
}
