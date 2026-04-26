use std::path::Path;

use anyhow::{Context, Result, bail};
use tokio::fs;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command as TokioCommand;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<()> {
    setup_logging();

    let args: Vec<String> = std::env::args().collect();
    let sudo_password = parse_sudo_password(&args)?;

    tracing::info!("Starting allcleaner");

    // Run system update, Flutter update, and Rust update in parallel
    let system_handle = tokio::spawn(update_system(sudo_password.clone()));
    let flutter_handle = tokio::spawn(update_flutter());
    let rust_handle = tokio::spawn(update_rust());

    // Wait for all tasks to complete
    let (system_result, flutter_result, rust_result) = tokio::join!(
        system_handle,
        flutter_handle,
        rust_handle,
    );

    system_result??;
    flutter_result??;
    rust_result??;

    tracing::info!("All updates completed successfully");
    Ok(())
}

fn setup_logging() {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,allcleaner=debug"));

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .with_target(true)
                .with_file(true)
                .with_line_number(true),
        )
        .with(env_filter)
        .init();
}

fn parse_sudo_password(args: &[String]) -> Result<String> {
    if args.len() < 2 {
        bail!("Usage: allcleaner <sudo_password>");
    }
    Ok(args[1].clone())
}

async fn stream_output(label: &str, mut child: tokio::process::Child) -> Result<std::process::ExitStatus> {
    let stdout = child.stdout.take().expect("stdout should be piped");
    let stderr = child.stderr.take().expect("stderr should be piped");

    let mut stdout_reader = BufReader::new(stdout).lines();
    let mut stderr_reader = BufReader::new(stderr).lines();

    let label_out = label.to_string();
    let stdout_task = tokio::spawn(async move {
        while let Ok(Some(line)) = stdout_reader.next_line().await {
            println!("[{}] {}", label_out, line);
        }
    });

    let label_err = label.to_string();
    let stderr_task = tokio::spawn(async move {
        while let Ok(Some(line)) = stderr_reader.next_line().await {
            eprintln!("[{}] {}", label_err, line);
        }
    });

    let status = child.wait().await.context("Failed to wait for child")?;
    stdout_task.await?;
    stderr_task.await?;

    Ok(status)
}

async fn update_system(sudo_password: String) -> Result<()> {
    tracing::info!("Updating system packages");

    // Detect package manager
    let package_manager = detect_package_manager()?;
    tracing::debug!("Detected package manager: {}", package_manager);

    // Run update command with sudo
    let (cmd, args) = match package_manager {
        "apt" => ("apt", vec!["update"]),
        "dnf" => ("dnf", vec!["upgrade", "-y"]),
        "pacman" => ("pacman", vec!["-Syu", "--noconfirm"]),
        _ => bail!("Unsupported package manager"),
    };

    let mut child = TokioCommand::new("sudo")
        .arg("-S")
        .arg(cmd)
        .args(&args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .context("Failed to spawn system update command")?;

    // Write sudo password to stdin
    if let Some(mut stdin) = child.stdin.take() {
        use tokio::io::AsyncWriteExt;
        stdin.write_all(format!("{}\n", sudo_password).as_bytes()).await?;
    }

    let status = stream_output("system", child).await?;

    if !status.success() {
        bail!("System update failed");
    }

    tracing::info!("System update completed");
    Ok(())
}

fn detect_package_manager() -> Result<&'static str> {
    if Path::new("/usr/bin/apt").exists() {
        Ok("apt")
    } else if Path::new("/usr/bin/dnf").exists() {
        Ok("dnf")
    } else if Path::new("/usr/bin/pacman").exists() {
        Ok("pacman")
    } else {
        bail!("No supported package manager found")
    }
}

async fn update_flutter() -> Result<()> {
    tracing::info!("Checking for Flutter updates");

    // Check if Flutter is installed
    let flutter_check = TokioCommand::new("flutter")
        .arg("--version")
        .output()
        .await;

    if flutter_check.is_err() {
        tracing::warn!("Flutter is not installed, skipping");
        return Ok(());
    }

    // Check for available updates using --verify-only
    let verify_output = TokioCommand::new("flutter")
        .arg("upgrade")
        .arg("--verify-only")
        .output()
        .await
        .context("Failed to run flutter upgrade --verify-only")?;

    let verify_stdout = String::from_utf8_lossy(&verify_output.stdout);
    let is_up_to_date = verify_stdout.contains("already up to date");

    if is_up_to_date {
        tracing::info!("Flutter is already up to date, skipping");
        return Ok(());
    }

    tracing::info!("Flutter updates available, updating...");

    let child = TokioCommand::new("flutter")
        .arg("upgrade")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .context("Failed to spawn flutter upgrade")?;

    let status = stream_output("flutter", child).await?;

    if !status.success() {
        bail!("Flutter upgrade failed");
    }

    tracing::info!("Flutter update completed");

    // Find and clean Flutter projects
    clean_flutter_projects().await?;

    Ok(())
}

async fn clean_flutter_projects() -> Result<()> {
    let dev_dir = dirs::home_dir()
        .context("Could not find home directory")?
        .join("dev");

    if !dev_dir.exists() {
        tracing::warn!("~/dev directory does not exist, skipping Flutter projects");
        return Ok(());
    }

    tracing::debug!("Scanning ~/dev for Flutter projects");

    let mut projects = Vec::new();
    find_flutter_projects(&dev_dir, &mut projects).await?;

    if projects.is_empty() {
        tracing::info!("No Flutter projects found in ~/dev");
        return Ok(());
    }

    tracing::info!("Found {} Flutter project(s), running flutter clean", projects.len());

    // Clean all projects in parallel
    let futures: Vec<_> = projects
        .into_iter()
        .map(|project_path| async move {
            tracing::info!("Cleaning Flutter project: {}", project_path.display());
            let child = TokioCommand::new("flutter")
                .arg("clean")
                .current_dir(&project_path)
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .with_context(|| format!("Failed to spawn flutter clean at {}", project_path.display()))?;

            let status = stream_output("flutter-clean", child).await?;

            if !status.success() {
                bail!("Flutter clean failed for {}", project_path.display());
            }

            Ok(())
        })
        .collect();

    // Run all clean operations in parallel
    futures::future::join_all(futures)
        .await
        .into_iter()
        .collect::<Result<()>>()?;

    tracing::info!("All Flutter projects cleaned");
    Ok(())
}

async fn find_flutter_projects(dir: &Path, projects: &mut Vec<std::path::PathBuf>) -> Result<()> {
    let mut entries = fs::read_dir(dir).await?;

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();

        if path.is_dir() {
            // Check if this is a Flutter project
            let pubspec_yaml = path.join("pubspec.yaml");
            if pubspec_yaml.exists() {
                projects.push(path.clone());
            } else {
                // Recurse into subdirectory
                Box::pin(find_flutter_projects(&path, projects)).await?;
            }
        }
    }

    Ok(())
}

async fn update_rust() -> Result<()> {
    tracing::info!("Checking for Rust updates");

    // Check if Rust is installed
    let rustc_check = TokioCommand::new("rustc")
        .arg("--version")
        .output()
        .await;

    if rustc_check.is_err() {
        tracing::warn!("Rust is not installed, skipping");
        return Ok(());
    }

    // Check for available updates
    let check_output = TokioCommand::new("rustup")
        .arg("check")
        .output()
        .await
        .context("Failed to run rustup check")?;

    let check_stdout = String::from_utf8_lossy(&check_output.stdout);
    let has_updates = check_stdout.lines().any(|line| line.contains("update available"));

    if !has_updates {
        tracing::info!("Rust is already up to date, skipping");
        return Ok(());
    }

    tracing::info!("Rust updates available, updating...");

    let child = TokioCommand::new("rustup")
        .arg("update")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .context("Failed to spawn rustup update")?;

    let status = stream_output("rustup", child).await?;

    if !status.success() {
        bail!("Rustup update failed");
    }

    tracing::info!("Rust update completed");

    // Find and clean Rust projects
    clean_rust_projects().await?;

    Ok(())
}

async fn clean_rust_projects() -> Result<()> {
    let dev_dir = dirs::home_dir()
        .context("Could not find home directory")?
        .join("dev");

    if !dev_dir.exists() {
        tracing::warn!("~/dev directory does not exist, skipping Rust projects");
        return Ok(());
    }

    tracing::debug!("Scanning ~/dev for Rust projects");

    let mut projects = Vec::new();
    find_rust_projects(&dev_dir, &mut projects).await?;

    if projects.is_empty() {
        tracing::info!("No Rust projects found in ~/dev");
        return Ok(());
    }

    tracing::info!("Found {} Rust project(s), running cargo clean && rm Cargo.lock", projects.len());

    // Clean all projects in parallel
    let futures: Vec<_> = projects
        .into_iter()
        .map(|project_path| async move {
            tracing::info!("Cleaning Rust project: {}", project_path.display());

            // Run cargo clean
            let child = TokioCommand::new("cargo")
                .arg("clean")
                .current_dir(&project_path)
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .with_context(|| format!("Failed to spawn cargo clean at {}", project_path.display()))?;

            let status = stream_output("cargo-clean", child).await?;

            if !status.success() {
                bail!("Cargo clean failed for {}", project_path.display());
            }

            // Remove Cargo.lock
            let cargo_lock = project_path.join("Cargo.lock");
            if cargo_lock.exists() {
                fs::remove_file(&cargo_lock)
                    .await
                    .with_context(|| format!("Failed to remove Cargo.lock at {}", cargo_lock.display()))?;
                tracing::debug!("Removed Cargo.lock at {}", cargo_lock.display());
            }

            Ok(())
        })
        .collect();

    // Run all clean operations in parallel
    futures::future::join_all(futures)
        .await
        .into_iter()
        .collect::<Result<()>>()?;

    tracing::info!("All Rust projects cleaned");
    Ok(())
}

async fn find_rust_projects(dir: &Path, projects: &mut Vec<std::path::PathBuf>) -> Result<()> {
    let mut entries = fs::read_dir(dir).await?;

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();

        if path.is_dir() {
            // Check if this is a Rust project (has Cargo.toml in root)
            let cargo_toml = path.join("Cargo.toml");
            if cargo_toml.exists() {
                projects.push(path.clone());
            } else {
                // Recurse into subdirectory
                Box::pin(find_rust_projects(&path, projects)).await?;
            }
        }
    }

    Ok(())
}
