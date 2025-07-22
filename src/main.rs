use std::process::{Command, Stdio};
use std::{env, fs, path::Path};

const AUR_URL: &str = "https://aur.archlinux.org";
const WORKDIR: &str = "/tmp/void-helper";

fn run_cmd(cmd: &str, args: &[&str]) -> bool {
    println!("[+] Executando: {} {}", cmd, args.join(" "));
    let status = Command::new(cmd)
        .args(args)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status();

    match status {
        Ok(s) if s.success() => true,
        _ => false,
    }
}

fn clean_pkg_folder(pkg: &str) {
    let pkg_path = format!("{}/{}", WORKDIR, pkg);
    let path = Path::new(&pkg_path);
    if path.exists() {
        println!("[*] Pasta {} já existe, removendo...", pkg_path);
        if let Err(e) = fs::remove_dir_all(path) {
            eprintln!("Erro ao remover pasta {}: {}", pkg_path, e);
        }
    }
}

fn install_package(pkg: &str) {
    if run_cmd("pacman", &["-Qi", pkg]) {
        println!("[✓] Pacote '{}' já está instalado.", pkg);
        return;
    }

    println!("[+] Tentando instalar '{}' dos repositórios oficiais...", pkg);
    if run_cmd("sudo", &["pacman", "-S", "--noconfirm", pkg]) {
        println!("[✓] Pacote oficial instalado.");
        return;
    }

    if !Path::new(WORKDIR).exists() {
        fs::create_dir_all(WORKDIR).expect("Falha ao criar diretório de trabalho");
    }
    env::set_current_dir(WORKDIR).expect("Falha ao mudar para diretório de trabalho");

    clean_pkg_folder(pkg);

    if !run_cmd("git", &["clone", &format!("{}/{}.git", AUR_URL, pkg)]) {
        println!("[!] Erro ao clonar pacote do AUR.");
        return;
    }

    let pkg_path = format!("{}/{}", WORKDIR, pkg);
    env::set_current_dir(&pkg_path).expect("Falha ao entrar na pasta do pacote");

    if run_cmd("makepkg", &["-si", "--noconfirm"]) {
        println!("[✓] Pacote '{}' instalado do AUR.", pkg);
    } else {
        println!("[!] Falha na instalação do pacote '{}'.", pkg);
    }
}

fn remove_package(pkg: &str) {
    if !run_cmd("pacman", &["-Qi", pkg]) {
        println!("[!] Pacote '{}' não está instalado.", pkg);
        return;
    }

    println!("[+] Removendo pacote '{}'...", pkg);
    if run_cmd("sudo", &["pacman", "-Rns", "--noconfirm", pkg]) {
        println!("[✓] Pacote removido.");
    } else {
        println!("[!] Falha na remoção do pacote.");
    }
}

fn print_usage() {
    println!("Uso:");
    println!("  void -S <pacote>   : Instalar pacote (oficial ou AUR)");
    println!("  void -R <pacote>   : Remover pacote");
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        print_usage();
        return;
    }

    match args[1].as_str() {
        "-S" => install_package(&args[2]),
        "-R" => remove_package(&args[2]),
        _ => print_usage(),
    }
}
