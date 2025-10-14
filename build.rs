fn main() {
    // Link against Windows system libraries needed by git2
    #[cfg(target_os = "windows")]
    {
        println!("cargo:rustc-link-lib=advapi32");
    }
}
