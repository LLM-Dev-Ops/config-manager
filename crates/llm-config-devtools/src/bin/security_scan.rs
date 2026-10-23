//! Security scanner CLI binary.
//!
//! Runs comprehensive security scans on Rust projects.

use clap::Parser;
use colored::Colorize;
use llm_config_devtools::report::{generate_report, write_report, OutputFormat};
use llm_config_devtools::security::{ScanConfig, SecurityScanner};
use std::path::PathBuf;
use std::process;
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "llm-security-scan")]
#[command(about = "Security scanner for LLM Config Manager", long_about = None)]
#[command(version)]
struct Cli {
    /// Project root directory to scan
    #[arg(short, long, default_value = ".")]
    project: PathBuf,

    /// Output file path (prints to stdout if not specified)
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Output format (json, yaml, markdown, sarif)
    #[arg(short, long, default_value = "markdown")]
    format: String,

    /// Disable clippy scanning
    #[arg(long)]
    no_clippy: bool,

    /// Disable unsafe code detection
    #[arg(long)]
    no_unsafe: bool,

    /// Disable secret scanning
    #[arg(long)]
    no_secrets: bool,

    /// Disable SQL injection scanning
    #[arg(long)]
    no_sql: bool,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,

    /// Fail with non-zero exit code if high severity findings are found
    #[arg(long)]
    fail_on_high: bool,
}

fn main() {
    let cli = Cli::parse();

    // Initialize logging
    let filter = if cli.verbose {
        EnvFilter::new("llm_config_devtools=debug")
    } else {
        EnvFilter::new("llm_config_devtools=info")
    };

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .init();

    // Parse output format
    let format = match cli.format.parse::<OutputFormat>() {
        Ok(f) => f,
        Err(e) => {
            eprintln!("{} {}", "Error:".red().bold(), e);
            process::exit(1);
        }
    };

    // Create scan configuration
    let config = ScanConfig {
        project_root: cli.project.clone(),
        scan_clippy: !cli.no_clippy,
        scan_unsafe: !cli.no_unsafe,
        scan_secrets: !cli.no_secrets,
        scan_sql: !cli.no_sql,
        max_workers: None,
    };

    // Print banner
    println!(
        "{}",
        "═══════════════════════════════════════════".cyan().bold()
    );
    println!("{}", "  LLM Config Manager Security Scanner".cyan().bold());
    println!(
        "{}",
        "═══════════════════════════════════════════".cyan().bold()
    );
    println!();

    // Run security scan
    println!("{} {}", "Scanning:".blue().bold(), cli.project.display());
    println!();

    let scanner = SecurityScanner::new(config);
    let report = match scanner.scan() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("{} {}", "Error:".red().bold(), e);
            process::exit(1);
        }
    };

    // Print summary
    print_summary(&report);

    // Generate report
    let report_content = match generate_report(&report, format) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{} {}", "Error generating report:".red().bold(), e);
            process::exit(1);
        }
    };

    // Write report
    if let Some(output_path) = cli.output {
        match write_report(&report, format, &output_path) {
            Ok(()) => {
                println!(
                    "{} {}",
                    "Report written to:".green().bold(),
                    output_path.display()
                );
            }
            Err(e) => {
                eprintln!("{} {}", "Error writing report:".red().bold(), e);
                process::exit(1);
            }
        }
    } else {
        println!();
        println!("{}", "─".repeat(50).dimmed());
        println!();
        println!("{}", report_content);
    }

    // Exit with appropriate code
    if cli.fail_on_high && report.has_high_severity() {
        eprintln!();
        eprintln!(
            "{} {}",
            "✗".red().bold(),
            "High severity findings detected".red()
        );
        process::exit(1);
    }

    if report.summary.total > 0 {
        println!();
        println!(
            "{} {}",
            "⚠".yellow().bold(),
            format!("{} findings - review recommended", report.summary.total).yellow()
        );
        process::exit(0);
    }

    println!();
    println!(
        "{} {}",
        "✓".green().bold(),
        "No security issues found!".green()
    );
    process::exit(0);
}

fn print_summary(report: &llm_config_devtools::security::SecurityReport) {
    println!("{}", "Summary:".blue().bold());
    println!(
        "  {} {}",
        "Total:".dimmed(),
        if report.summary.total > 0 {
            report.summary.total.to_string().yellow()
        } else {
            report.summary.total.to_string().green()
        }
    );
    println!(
        "  {} {}",
        "Critical:".dimmed(),
        report.summary.critical.to_string().red()
    );
    println!(
        "  {} {}",
        "High:".dimmed(),
        report.summary.high.to_string().bright_red()
    );
    println!(
        "  {} {}",
        "Medium:".dimmed(),
        report.summary.medium.to_string().yellow()
    );
    println!(
        "  {} {}",
        "Low:".dimmed(),
        report.summary.low.to_string().blue()
    );
    println!(
        "  {} {:.2}s",
        "Duration:".dimmed(),
        report.summary.duration_seconds
    );
    println!();
}
