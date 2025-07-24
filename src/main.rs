use clap::{Arg, Command};
use colored::*;
use dirs;
use reqwest;
use serde::{Deserialize, Serialize};
use std::fs;
use std::process::{Command as StdCommand, Stdio};

#[derive(Debug, Serialize, Deserialize)]
struct AurResponse {
    results: Vec<AurPackage>,
}

#[derive(Debug, Serialize, Deserialize)]
struct AurPackage {
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "PackageBase")]
    package_base: String,
    #[serde(rename = "Description")]
    description: Option<String>,
}

fn cli() -> Command {
    Command::new("void")
        .about("Minimalist AUR helper")
        .version("0.2")
        .arg_required_else_help(true)
        .subcommand_required(true)
        .subcommand(
            Command::new("sync")
                .about("Synchronize packages")
                .short_flag('S')
                .subcommand(
                    Command::new("search")
                        .about("Search packages")
                        .short_flag('s')
                        .arg(Arg::new("query").required(true)),
                )
                .subcommand(
                    Command::new("install")
                        .about("Install package")
                        .arg(Arg::new("package").required(true)),
                ),
        )
        .subcommand(
            Command::new("remove")
                .about("Remove package")
                .short_flag('R')
                .arg(Arg::new("package").required(true)),
        )
}

async fn get_package_info(package: &str) -> Result<Option<AurPackage>, Box<dyn std::error::Error>> {
    let url = format!("https://aur.archlinux.org/rpc/?v=5&type=info&arg[]={}", package);
    let response = reqwest::get(&url).await?.json::<AurResponse>().await?;
    Ok(response.results.into_iter().next())
}

async fn search_packages(query: &str) -> Result<Vec<AurPackage>, Box<dyn std::error::Error>> {
    let url = format!("https://aur.archlinux.org/rpc/?v=5&type=search&arg={}", query);
    let response = reqwest::get(&url).await?.json::<AurResponse>().await?;
    
    // Advanced filtering for better relevance
    let query_lower = query.to_lowercase();
    let filtered: Vec<AurPackage> = response.results
        .into_iter()
        .filter(|pkg| {
            let name_lower = pkg.name.to_lowercase();
            
            // Exact match gets highest priority
            if name_lower == query_lower {
                return true;
            }
            
            // Starts with query
            if name_lower.starts_with(&query_lower) {
                return true;
            }
            
            // Contains query as whole word
            if name_lower.split(|c: char| !c.is_alphanumeric())
                .any(|word| word == query_lower) {
                return true;
            }
            
            false
        })
        .collect();
    
    Ok(filtered)
}

async fn show_search_results(query: &str) -> Result<(), Box<dyn std::error::Error>> {
    let packages = search_packages(query).await?;
    
    if packages.is_empty() {
        println!("{} {}", "No packages found for:".red(), query);
        return Ok(());
    }

    println!("{} {}", "Found packages:".green(), query.bold());
    
    // Show max 8 most relevant results
    for pkg in packages.iter().take(8) {
        println!("{} - {}",
            pkg.name.bright_green().bold(),
            pkg.description.as_deref().unwrap_or("No description").dimmed()
        );
    }

    Ok(())
}

async fn install_package(package: &str) -> Result<(), Box<dyn std::error::Error>> {
    match get_package_info(package).await? {
        Some(pkg) => {
            println!("{} {}", "Installing:".bright_green(), pkg.name.bold());
            
            let build_dir = dirs::home_dir()
                .unwrap()
                .join(".void-builds")
                .join(&pkg.package_base);
            
            if build_dir.exists() {
                fs::remove_dir_all(&build_dir)?;
            }

            fs::create_dir_all(&build_dir)?;

            let aur_url = format!("https://aur.archlinux.org/{}.git", pkg.package_base);
            
            if !StdCommand::new("git")
                .arg("clone")
                .arg(&aur_url)
                .arg(&build_dir)
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .status()?
                .success() {
                eprintln!("{}", "Failed to clone repository".red());
                return Ok(());
            }

            let status = StdCommand::new("makepkg")
                .arg("-si")
                .current_dir(&build_dir)
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .status()?;

            if !status.success() {
                eprintln!("{}", "Installation failed".red());
            } else {
                println!("{} {}", "Success:".bright_green(), pkg.name.bold());
            }
        }
        None => {
            println!("{} {}", "Package not found:".red(), package);
            show_search_results(package).await?;
        }
    }

    Ok(())
}

async fn remove_package(package: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("{} {}", "Removing:".yellow(), package.bold());
    
    let status = StdCommand::new("sudo")
        .arg("pacman")
        .arg("-Rns")
        .arg(package)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()?;

    if !status.success() {
        eprintln!("{}", "Removal failed".red());
    } else {
        println!("{} {}", "Removed:".bright_green(), package.bold());
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let matches = cli().get_matches();

    match matches.subcommand() {
        Some(("sync", sync_matches)) => match sync_matches.subcommand() {
            Some(("search", search_matches)) => {
                show_search_results(search_matches.get_one::<String>("query").unwrap()).await?
            }
            Some(("install", install_matches)) => {
                install_package(install_matches.get_one::<String>("package").unwrap()).await?
            }
            _ => eprintln!("{}", "Invalid sync command".red()),
        },
        Some(("remove", remove_matches)) => {
            remove_package(remove_matches.get_one::<String>("package").unwrap()).await?
        }
        _ => eprintln!("{}", "Invalid command".red()),
    }

    Ok(())
}