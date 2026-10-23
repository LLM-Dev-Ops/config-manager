//! Dependency vulnerability scanner CLI binary.
//!
//! Scans project dependencies for known vulnerabilities using cargo-audit.

use clap::Parser;
use colored::Colorize;
use std::path::PathBuf;
use std::process::{self, Command};

#[derive(Parser)]
#[command(name = "llm-dependency-scan")]
#[command(about = "Dependency vulnerability scanner for LLM Config Manager", long_about = None)]
#[command(version)]
struct Cli {
    /// Project root directory to scan
    #[arg(short, long, default_value = ".")]
    project: PathBuf,

    /// Output file path for JSON report
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Fail with non-zero exit code if vulnerabilities are found
    #[arg(long, default_value = "true")]
    fail_on_vulnerabilities: bool,

    /// Check for outdated dependencies
    #[arg(long)]
    check_outdated: bool,

    /// Check for unused dependencies
    #[arg(long)]
    check_unused: bool,
}

fn main() {
    let cli = Cli::parse();

    // Print banner
    println!(
        "{}",
        "═══════════════════════════════════════════".cyan().bold()
    );
    println!("{}", "  Dependency Vulnerability Scanner".cyan().bold());
    println!(
        "{}",
        "═══════════════════════════════════════════".cyan().bold()
    );
    println!();

    println!("{} {}", "Scanning:".blue().bold(), cli.project.display());
    println!();

    // Check if cargo-audit is installed
    if !is_cargo_audit_available() {
        eprintln!("{} cargo-audit not found", "Warning:".yellow().bold());
        eprintln!("Install with: cargo install cargo-audit");
        eprintln!("Skipping dependency vulnerability scan");
        process::exit(1);
    }

    // Run cargo audit
    println!("{} Running cargo audit...", "➤".blue().bold());
    let audit_result = run_cargo_audit(&cli.project, cli.output.as_ref());

    if cli.check_outdated {
        println!();
        println!(
            "{} Checking for outdated dependencies...",
            "➤".blue().bold()
        );
        check_outdated_dependencies(&cli.project);
    }

    if cli.check_unused {
        println!();
        println!("{} Checking for unused dependencies...", "➤".blue().bold());
        check_unused_dependencies(&cli.project);
    }

    // Exit with appropriate code
    if cli.fail_on_vulnerabilities && !audit_result {
        println!();
        eprintln!("{} {}", "✗".red().bold(), "Vulnerabilities detected".red());
        process::exit(1);
    }

    if audit_result {
        println!();
        println!(
            "{} {}",
            "✓".green().bold(),
            "No vulnerabilities found!".green()
        );
    }

    process::exit(0);
}

fn is_cargo_audit_available() -> bool {
    Command::new("cargo")
        .arg("audit")
        .arg("--version")
        .output()
        .is_ok()
}

fn run_cargo_audit(project_root: &PathBuf, output_file: Option<&PathBuf>) -> bool {
    let mut cmd = Command::new("cargo");
    cmd.arg("audit").current_dir(project_root);

    if let Some(output) = output_file {
        cmd.arg("--json").arg("--output").arg(output);
    }

    match cmd.status() {
        Ok(status) => {
            if status.success() {
                println!("  {} No vulnerabilities found", "✓".green().bold());
                true
            } else {
                eprintln!("  {} Vulnerabilities detected", "✗".red().bold());
                false
            }
        }
        Err(e) => {
            eprintln!("{} Failed to run cargo audit: {}", "Error:".red().bold(), e);
            false
        }
    }
}

fn check_outdated_dependencies(project_root: &PathBuf) {
    // Check if cargo-outdated is installed
    if !Command::new("cargo")
        .arg("outdated")
        .arg("--version")
        .output()
        .is_ok()
    {
        eprintln!("  {} cargo-outdated not found", "⚠".yellow().bold());
        eprintln!("  Install with: cargo install cargo-outdated");
        return;
    }

    match Command::new("cargo")
        .arg("outdated")
        .current_dir(project_root)
        .status()
    {
        Ok(_) => {}
        Err(e) => {
            eprintln!(
                "  {} Failed to check outdated dependencies: {}",
                "Error:".red().bold(),
                e
            );
        }
    }
}

fn check_unused_dependencies(project_root: &PathBuf) {
    // Check if cargo-udeps is installed
    if !Command::new("cargo")
        .arg("+nightly")
        .arg("udeps")
        .arg("--version")
        .output()
        .is_ok()
    {
        eprintln!("  {} cargo-udeps not found", "⚠".yellow().bold());
        eprintln!("  Install with: cargo +nightly install cargo-udeps");
        return;
    }

    match Command::new("cargo")
        .arg("+nightly")
        .arg("udeps")
        .arg("--all-targets")
        .current_dir(project_root)
        .status()
    {
        Ok(_) => {}
        Err(e) => {
            eprintln!(
                "  {} Failed to check unused dependencies: {}",
                "Error:".red().bold(),
                e
            );
        }
    }
}
