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
    match Command::new("trunk")
        .current_dir(format!("{}/ui", base_dir))
        .arg("build")
        .arg("--release")
        .output()
    {
        Ok(_) => {
            match Command::new("cp")
                .arg("-R")
                .arg(format!("{}/ui/dist", base_dir))
                .arg(base_dir + "/dist")
                .output()
            {
                Ok(_) => Ok(()),
                Err(e) => {
                    println!("cargo:warning=❌ Failed fetching dist files: {}", e);
                    Err(())
                }
            }
        }
        Err(e) => {
            println!("cargo:warning=❌ Failed to run trunk: {} ", e);
            println!("cargo:warning=❗ install trunk using \x1b[1mcargo install trunk\x1b[0m");
            Err(())
        }
    }
}
