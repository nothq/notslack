fn main() {
    println!("cargo:rerun-if-changed=src/video_capture_bridge.mm");
    let target_os = std::env::var("CARGO_CFG_TARGET_OS")
        .expect("Cargo must provide the target operating system");
    if target_os == "macos" {
        build_video_capture_bridge();
    }
}

fn build_video_capture_bridge() {
    cc::Build::new()
        .cpp(true)
        .cpp_link_stdlib("c++")
        .file("src/video_capture_bridge.mm")
        .flag("-std=c++17")
        .flag("-fobjc-arc")
        .compile("notslack_video_native_capture");

    println!("cargo:rustc-link-lib=framework=AudioToolbox");
    println!("cargo:rustc-link-lib=framework=AVFoundation");
    println!("cargo:rustc-link-lib=framework=CoreMedia");
    println!("cargo:rustc-link-lib=framework=CoreVideo");
    println!("cargo:rustc-link-lib=framework=Foundation");
}
