use std::collections::HashSet;
use std::path::PathBuf;
use std::process::Stdio;

use anyhow::{Context, Result};
use clap::Parser;
use regex::Regex;

use crate::utils::{cargo_cmd, metadata};

mod utils;
use utils::{checkout_baseline, metadata_from_dir, workspace_crates_in_folder};

#[derive(Debug, Clone, Parser)]
pub struct SemverChecks {
    /// Baseline git revision branch to compare against
    #[clap(long, default_value = "main")]
    baseline: String,

    /// Disable hakari
    #[clap(long, default_value = "false")]
    disable_hakari: bool,
}

impl SemverChecks {
    pub fn run(self) -> Result<()> {
        println!("<details>");
        println!("<summary> 🛫 Startup details 🛫 </summary>");
        let current_metadata = metadata().context("getting current metadata")?;
        let current_crates_set = workspace_crates_in_folder(&current_metadata, "crates");

        let tmp_dir = PathBuf::from("target/semver-baseline");

        // Checkout baseline (auto-cleanup on Drop)
        let _worktree_cleanup = checkout_baseline(&self.baseline, &tmp_dir).context("checking out baseline")?;

        let baseline_metadata = metadata_from_dir(&tmp_dir).context("getting baseline metadata")?;
        let baseline_crates_set = workspace_crates_in_folder(&baseline_metadata, &tmp_dir.join("crates").to_string_lossy());

        let common_crates: HashSet<_> = current_metadata
            .packages
            .iter()
            .map(|p| p.name.clone())
            .filter(|name| current_crates_set.contains(name) && baseline_crates_set.contains(name))
            .collect();

        let mut crates: Vec<_> = common_crates.iter().cloned().collect();
        crates.sort();

        println!("<details>");
        println!("<summary> 📦 Processing crates 📦 </summary>");
        // need to print an empty line for the bullet list to format correctly
        println!();
        for krate in crates {
            println!("- `{}`", krate);
        }
        // close crate details
        println!("</details>");
        // close startup details
        println!("</details>");

        if self.disable_hakari {
            cargo_cmd().args(["hakari", "disable"]).status().context("disabling hakari")?;
        }

        let mut args = vec![
            "semver-checks",
            "check-release",
            "--baseline-root",
            tmp_dir.to_str().unwrap(),
            "--all-features",
        ];

        for package in &common_crates {
            args.push("--package");
            args.push(package);
        }

        let output = cargo_cmd()
            .env("CARGO_TERM_COLOR", "never")
            .args(&args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .context("running semver-checks")?;

        let mut semver_output = String::new();
        semver_output.push_str(&String::from_utf8_lossy(&output.stdout));
        semver_output.push_str(&String::from_utf8_lossy(&output.stderr));

        if semver_output.trim().is_empty() {
            anyhow::bail!("No semver-checks output received. The command may have failed.");
        }

        // empty print to separate from startup details
        println!();

        // Regex to capture "Checking" lines (ignoring leading whitespace).
        // Supports both formats:
        //   "Checking <crate> vX.Y.Z (current)"
        //   "Checking <crate> vX.Y.Z -> vX.Y.Z (no change)"
        let check_re = Regex::new(r"^Checking\s+(?P<crate>\S+)\s+v(?P<curr>\d+\.\d+\.\d+)(?:\s+->\s+v\d+\.\d+\.\d+)?")
            .context("compiling check regex")?;

        // Regex for summary lines that indicate an update is required.
        // Example:
        //   "Summary semver requires new major version: 1 major and 0 minor checks failed"
        let summary_re = Regex::new(r"^Summary semver requires new (?P<update_type>major|minor) version:")
            .context("compiling summary regex")?;

        let commit_hash = std::env::var("SHA").unwrap();
        let scuffle_commit_url = format!("https://github.com/ScuffleCloud/scuffle/blob/{commit_hash}");

        let mut current_crate: Option<(String, String)> = None;
        let mut summary: Vec<String> = Vec::new();
        let mut description: Vec<String> = Vec::new();
        let mut error_count = 0;

        let mut lines = semver_output.lines().peekable();
        while let Some(line) = lines.next() {
            let trimmed = line.trim_start();

            if trimmed.starts_with("Checking") {
                // Capture crate name and version without printing.
                if let Some(caps) = check_re.captures(trimmed) {
                    let crate_name = caps.name("crate").unwrap().as_str().to_string();
                    let current_version = caps.name("curr").unwrap().as_str().to_string();
                    current_crate = Some((crate_name, current_version));
                }
            } else if trimmed.starts_with("Summary") {
                if let Some(caps) = summary_re.captures(trimmed) {
                    let update_type = caps.name("update_type").unwrap().as_str();
                    if let Some((crate_name, current_version)) = current_crate.take() {
                        let new_version = new_version_number(&current_version, update_type)?;
                        error_count += 1;

                        // need to escape the #{error_count} otherwise it will refer to an actual pr
                        summary.push(format!("### 🔖 Error `#{error_count}`"));
                        summary.push(format!("⚠️ {update_type} update required for `{crate_name}`."));
                        summary.push(format!(
                            "🛠️ Please update the version from `v{current_version}` to `{new_version}`."
                        ));

                        summary.push("<details>".to_string());
                        summary.push(format!("<summary> 📜 {crate_name} logs 📜 </summary>"));
                        summary.append(&mut description);
                        summary.push("</details>".to_string());

                        // add a new line after the description
                        summary.push("".to_string());
                    }
                }
            } else if trimmed.starts_with("---") {
                let mut is_failed_in_block = false;

                for desc_line in lines.by_ref() {
                    let desc_trimmed = desc_line.trim_start();

                    if desc_trimmed.starts_with("Checking")
                        || desc_trimmed.starts_with("Built")
                        || desc_trimmed.starts_with("Building")
                        || desc_trimmed.starts_with("Parsing")
                        || desc_trimmed.starts_with("Parsed")
                        || desc_trimmed.starts_with("Finished")
                        || desc_trimmed.starts_with("Summary")
                    {
                        // sometimes an empty new line isn't detected before the description ends
                        // in that case, add a closing `</details>` for the "Failed in" block.
                        if is_failed_in_block {
                            description.push("</details>".to_string());
                        }
                        break;
                    } else if desc_trimmed.starts_with("Failed in:") {
                        // create detail block for "Failed in" block
                        is_failed_in_block = true;
                        description.push("<details>".to_string());
                        description.push("<summary> 🎈 Failed in the following locations 🎈 </summary>".to_string());
                    } else if desc_trimmed.is_empty() && is_failed_in_block {
                        // close detail close for "Failed in" block
                        is_failed_in_block = false;
                        description.push("</details>".to_string());
                    } else if is_failed_in_block {
                        // need new line to allow for bullet list formatting
                        description.push("".to_string());

                        let file_loc = desc_trimmed
                            .split_whitespace()
                            .last()
                            .unwrap()
                            .strip_prefix("/home/runner/work/scuffle/scuffle/")
                            .unwrap()
                            .replace(":", "#L");

                        description.push(format!("- {scuffle_commit_url}/{file_loc}"));
                    } else {
                        description.push(desc_trimmed.to_string());
                    }
                }
            }
        }

        // Print deferred update and failure block messages.
        println!("# Semver-checks summary");
        if error_count > 0 {
            let s = if error_count == 1 { "" } else { "S" };
            println!("\n### 🚩 {error_count} ERROR{s} FOUND 🚩");

            // if there are 5+ errors, shrink the details by default.
            if error_count >= 5 {
                summary.insert(0, "<details>".to_string());
                summary.insert(1, "<summary> 🦗 Open for error description 🦗 </summary>".to_string());
                summary.push("</details>".to_string());
            }

            for line in summary {
                println!("{line}");
            }
        } else {
            println!("## ✅ No semver violations found! ✅");
        }

        // print an empty line to separate output from worktree cleanup line
        println!();

        Ok(())
    }
}

fn new_version_number(version: &str, update_type: &str) -> Result<String> {
    let version = version.strip_prefix('v').unwrap_or(version);
    let mut parts: Vec<u64> = version
        .split('.')
        .map(|s| s.parse::<u64>())
        .collect::<Result<_, _>>()
        .context("parsing version numbers")?;
    if parts.len() != 3 {
        anyhow::bail!("expected version format vX.Y.Z, got: {version}");
    }
    match update_type {
        "minor" => parts[2] += 1,
        "major" => parts[1] += 1,
        _ => anyhow::bail!("Failed to parse update type: {update_type}"),
    }
    Ok(format!("v{}.{}.{}", parts[0], parts[1], parts[2]))
}
