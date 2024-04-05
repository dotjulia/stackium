use std::process::Command;

fn main() -> Result<(), ()> {
    println!("cargo:rerun-if-changed=ui");
    println!("cargo:warning=detected UI update, rebuilding");
    match std::env::var("NOUI") {
        Ok(str) => {
            if str == "1" {
                println!("cargo:warning=✅ Skipping UI build");
                return Ok(());
            }
        }
        _ => {}
    }
    let base_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    // return Ok(());
    #[cfg(all(not(debug_assertions), target_arch = "aarch64"))]
    return match Command::new("trunk")
        .current_dir(format!("{}/ui", base_dir))
        .arg("build")
        // .arg("--release")
        .output()
    {
        Ok(output) => {
            if output.status.success() {
                Ok(())
            } else {
                println!(
                    "cargo:warning=❌ Trunk failed: {}\ncargo:warning={}",
                    std::str::from_utf8(&output.stdout).unwrap(),
                    std::str::from_utf8(&output.stderr).unwrap()
                );
                Err(())
            }
        }
        Err(e) => {
            println!("cargo:warning=❌ Failed to run trunk: {} ", e);
            println!("cargo:warning=❗ install trunk using \x1b[1mcargo install trunk\x1b[0m");
            Err(())
        }
    };
    #[cfg(not(debug_assertions))]
    return match Command::new("trunk")
        .current_dir(format!("{}/ui", base_dir))
        .arg("build")
        .arg("--release")
        .output()
    {
        Ok(output) => {
            if output.status.success() {
                Ok(())
            } else {
                println!(
                    "cargo:warning=❌ Trunk failed: {}\ncargo:warning={}",
                    std::str::from_utf8(&output.stdout).unwrap(),
                    std::str::from_utf8(&output.stderr).unwrap()
                );
                Err(())
            }
        }
        Err(e) => {
            println!("cargo:warning=❌ Failed to run trunk: {} ", e);
            println!("cargo:warning=❗ install trunk using \x1b[1mcargo install trunk\x1b[0m");
            Err(())
        }
    };
    #[cfg(debug_assertions)]
    Ok(())
}
