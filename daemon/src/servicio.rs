use std::process::Command;

use crate::sistema::{self, ETIQUETA_MENU};

pub fn asegurar_menu() {
    if !cfg!(target_os = "macos") {
        return;
    }
    if menu_abierta() {
        return;
    }
    let app = "/Applications/AIcontrol.app";
    if let Some(usuario) = usuario_consola() {
        if let Some(uid) = uid_de(&usuario) {
            let destino = format!("gui/{uid}/{ETIQUETA_MENU}");
            let _ = Command::new("launchctl").args(["kickstart", &destino]).status();
            if menu_abierta() {
                return;
            }
            let _ = Command::new("launchctl").args(["asuser", &uid, "open", "-gja", app]).status();
            return;
        }
    }
    let _ = Command::new("open").args(["-gja", app]).status();
}

pub fn aviso(nombre: &str) {
    let texto = format!("{nombre} no se puede abrir");
    if cfg!(target_os = "macos") {
        let script = format!("display notification \"{}\" with title \"AIcontrol\"", texto.replace('"', "'"));
        let _ = Command::new("osascript").args(["-e", &script]).spawn();
    } else {
        eprintln!("{texto}");
    }
}

fn menu_abierta() -> bool {
    sistema::listar_procesos().iter().any(|proceso| {
        let texto = format!("{} {}", proceso.exe, proceso.cmdline);
        texto.contains("AIcontrol.app/Contents/MacOS/AIcontrol") && !texto.contains("AIcontrol-daemon")
    })
}

fn usuario_consola() -> Option<String> {
    let salida = Command::new("stat").args(["-f", "%Su", "/dev/console"]).output().ok()?;
    let nombre = String::from_utf8_lossy(&salida.stdout).trim().to_string();
    if nombre.is_empty() || nombre == "root" { None } else { Some(nombre) }
}

fn uid_de(usuario: &str) -> Option<String> {
    let salida = Command::new("id").args(["-u", usuario]).output().ok()?;
    let uid = String::from_utf8_lossy(&salida.stdout).trim().to_string();
    if uid.is_empty() { None } else { Some(uid) }
}
