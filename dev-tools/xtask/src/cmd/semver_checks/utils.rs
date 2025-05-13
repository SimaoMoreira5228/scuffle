use std::collections::HashSet;
use std::path::PathBuf;
use std::process::Command;

use cargo_metadata::Metadata;

pub struct WorktreeCleanup {
    path: PathBuf,
}

impl Drop for WorktreeCleanup {
    fn drop(&mut self) {
        // extra line to separate from semver output
        println!("\n<details>");
        // extra line to separate from below output for proper formatting
        println!("<summary> 🛬 Cleanup details 🛬 </summary>\n");
        println!("Cleaning up git worktree at {:?}\n", self.path);
        let status = Command::new("git")
            .args(["worktree", "remove", "--force", self.path.to_str().unwrap()])
            .status();

        match status {
            Ok(status) if status.success() => {
                println!("Successfully removed git worktree");
            }
            Ok(status) => {
                eprintln!("Failed to remove git worktree. Exit code: {status}");
            }
            Err(e) => {
                eprintln!("Error removing git worktree: {e:?}");
            }
        }

        println!("</details>");
    }
}

pub fn workspace_crates_in_folder(meta: &Metadata, folder: &str) -> HashSet<String> {
    let folder_path = std::fs::canonicalize(folder).expect("folder should exist");

    meta.packages
        .iter()
        .filter(|p| {
            // All crate examples have publish = false.
            // The scuffle-bootstrap-derive crate doesn't work with the semver-checks tool at the moment.
            let manifest_path = p.manifest_path.parent().unwrap();
            manifest_path.starts_with(&folder_path)
                && p.publish.as_ref().map(|v| !v.is_empty()).unwrap_or(true)
                && p.name != "scuffle-bootstrap-derive"
                && p.name != "scuffle-metrics-derive"
        })
        .map(|p| p.name.clone())
        .collect()
}

pub fn is_published_on_crates_io(crate_name: &str) -> bool {
    let url = crate_index_url(crate_name);

    let output = Command::new("curl")
        .args(["-s", "--head", "-L", "-w", "%{http_code}", &url])
        .output();

    if let Err(e) = output {
        eprintln!("Error checking crate on crates.io: {e}");
        return false;
    }

    let output = output.unwrap();
    let status_code = String::from_utf8_lossy(&output.stdout);

    status_code.contains("200")
}

fn crate_index_url(crate_name: &str) -> String {
    let name = crate_name.to_lowercase();
    let len = name.len();

    match len {
        0 => panic!("Invalid crate name"),
        1 => format!("https://index.crates.io/1/{name}"),
        2 => format!("https://index.crates.io/2/{name}"),
        3 => format!("https://index.crates.io/3/{}/{}", &name[0..1], name),
        _ => {
            let prefix = &name[0..2];
            let suffix = &name[2..4];
            format!("https://index.crates.io/{prefix}/{suffix}/{name}")
        }
    }
}
