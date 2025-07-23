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
        .about("A minimalist AUR helper")
        .version("0.1")
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(
            Command::new("install")
                .short_flag('S')
                .about("Install a package")
                .arg(Arg::new("package").required(true)),
        )
        .subcommand(
            Command::new("remove")
                .short_flag('R')
                .about("Remove a package")
                .arg(Arg::new("package").required(true)),
        )
}

async fn get_package_info(package: &str) -> Result<Option<AurPackage>, Box<dyn std::error::Error>> {
    let info_url = format!("https://aur.archlinux.org/rpc/?v=5&type=info&arg[]={}", package);
    let response = reqwest::get(&info_url).await?.json::<AurResponse>().await?;
    Ok(response.results.into_iter().next())
}

async fn search_packages(query: &str) -> Result<Vec<AurPackage>, Box<dyn std::error::Error>> {
    let url = format!("https://aur.archlinux.org/rpc/?v=5&type=search&arg={}", query);
    let response = reqwest::get(&url).await?.json::<AurResponse>().await?;
    
    let filtered = response.results.into_iter()
        .filter(|pkg| {
            let name = pkg.name.to_lowercase();
            let query = query.to_lowercase();
            name.starts_with(&query) ||
            name.split('-').any(|part| part == query) ||
            name.split('_').any(|part| part == query)
        })
        .collect();
    
    Ok(filtered)
}

async fn install_package(package: &str) -> Result<(), Box<dyn std::error::Error>> {
    match get_package_info(package).await? {
        Some(pkg) => {
            println!("{} {}", "Installing:".green(), pkg.name);
            
            let build_dir = dirs::home_dir()
                .unwrap()
                .join("aur-builds")
                .join(&pkg.package_base);
            
            if build_dir.exists() {
                println!("{} {}", "Removing existing directory:".yellow(), build_dir.display());
                fs::remove_dir_all(&build_dir)?;
            }

            fs::create_dir_all(&build_dir)?;

            let aur_url = format!("https://aur.archlinux.org/{}.git", pkg.package_base);
            
            let git_status = StdCommand::new("git")
                .arg("clone")
                .arg(&aur_url)
                .arg(&build_dir)
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .status()?;

            if !git_status.success() {
                eprintln!("{}", "Failed to clone repository".red());
                return Ok(());
            }

            let makepkg = StdCommand::new("makepkg")
                .arg("-si")
                .current_dir(&build_dir)
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .status();

            match makepkg {
                Ok(status) if status.success() => {
                    println!("{} {}", "Successfully installed:".green(), pkg.name.bold());
                }
                _ => {
                    eprintln!("{}", "First attempt failed, trying with PGP check skip...".yellow());
                    
                    let status = StdCommand::new("makepkg")
                        .arg("-si")
                        .arg("--skippgpcheck")
                        .current_dir(&build_dir)
                        .stdout(Stdio::inherit())
                        .stderr(Stdio::inherit())
                        .status()?;
                    
                    if !status.success() {
                        eprintln!("{}", "Failed to build and install package".red());
                        eprintln!("{}", "You may need to manually import PGP keys:".yellow());
                        eprintln!("{}", "Look for any missing key IDs in the error messages above".cyan());
                        eprintln!("{}", "Then run: gpg --recv-keys <KEY_ID>".cyan());
                    } else {
                        println!("{} {}", "Successfully installed with PGP check skip:".yellow(), pkg.name.bold());
                    }
                }
            }
        }
        None => {
            println!("{} {}", "Package not found in AUR:".red(), package);
            suggest_similar_packages(package).await?;
        }
    }

    Ok(())
}

async fn remove_package(package: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("{} {}", "Removing package:".yellow(), package);
    
    let status = StdCommand::new("sudo")
        .arg("pacman")
        .arg("-Rns")
        .arg(package)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()?;

    if !status.success() {
        eprintln!("{}", "Failed to remove package".red());
    } else {
        println!("{} {}", "Successfully removed:".green(), package.bold());
    }

    Ok(())
}

async fn suggest_similar_packages(query: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", "Searching for similar packages...".yellow());
    
    let packages = search_packages(query).await?;
    
    if packages.is_empty() {
        println!("{}", "No similar packages found.".red());
        return Ok(());
    }

    println!("{}", "Did you mean:".yellow());
    for (i, pkg) in packages.iter().take(5).enumerate() {
        println!("{}. {} - {}", 
            i + 1, 
            pkg.name.green(), 
            pkg.description.as_deref().unwrap_or("No description").dimmed());
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let matches = cli().get_matches();

    match matches.subcommand() {
        Some(("install", sub_matches)) => {
            let package = sub_matches.get_one::<String>("package").unwrap();
            install_package(package).await?;
        }
        Some(("remove", sub_matches)) => {
            let package = sub_matches.get_one::<String>("package").unwrap();
            remove_package(package).await?;
        }
        _ => unreachable!(),
    }

    Ok(())
}