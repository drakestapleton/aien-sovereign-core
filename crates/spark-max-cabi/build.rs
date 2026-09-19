use std::process::Command;
use std::path::Path;

fn main() {
    println!("cargo:rerun-if-changed=mojo/spark_max_bridge.mojo");
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("libspark_max.so");

    if let Ok(status) = Command::new("mojo")
        .args(&["build", "--emit", "shared-lib", "mojo/spark_max_bridge.mojo", "-o"])
        .arg(&dest_path)
        .status()
    {
        if status.success() {
            println!("cargo:rustc-env=SPARK_MAX_SO_BUILT={}", dest_path.display());
            let _ = std::fs::copy(&dest_path, "mojo/libspark_max.so");
        } else {
            println!("cargo:warning=mojo build exited with non-zero status");
        }
    } else {
        println!("cargo:warning=mojo binary not found on PATH, skipping libspark_max.so build");
    }
}
