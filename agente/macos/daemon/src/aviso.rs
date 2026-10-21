use std::io::{BufRead, Write};
use std::process::{Command, Stdio};

use crate::baja::{AVISO_CONTROL, AVISO_DESINSTALADO, AVISO_INSTALACION, AVISO_IRREVERSIBLE};

pub fn advertir_desinstalacion() -> Result<(), String> {
    confirmar("AIcontrol", AVISO_CONTROL)?;
    confirmar("AIcontrol", AVISO_IRREVERSIBLE)
}

pub fn informar_instalacion() {
    let _ = informar("AIcontrol", AVISO_INSTALACION);
}

pub fn informar_desinstalacion() {
    let _ = informar("AIcontrol", AVISO_DESINSTALADO);
}

pub fn informar(titulo: &str, texto: &str) -> Result<(), String> {
    dialogo(titulo, texto, &["Aceptar"], "Aceptar")
}

pub fn confirmar(titulo: &str, cuerpo: &str) -> Result<(), String> {
    dialogo(titulo, cuerpo, &["Cancelar", "Continuar"], "Continuar")
}

pub fn pedir_clave_ajustes() -> Result<String, String> {
    if !cfg!(target_os = "macos") {
        return Err("cancelado".into());
    }
    let mut comando = Command::new("osascript");
    comando.args([
        "-e",
        r#"set respuesta to display dialog "Clave de Config" default answer "" with hidden answer buttons {"Cancelar", "Continuar"} default button "Continuar" cancel button "Cancelar" with title "AIcontrol""#,
        "-e",
        "text returned of respuesta",
    ]);
    let salida = comando.output().map_err(|error| error.to_string())?;
    if !salida.status.success() {
        return Err("cancelado".into());
    }
    let texto = String::from_utf8_lossy(&salida.stdout).trim().to_string();
    if texto.is_empty() {
        return Err("cancelado".into());
    }
    Ok(texto)
}

fn dialogo(titulo: &str, cuerpo: &str, botones: &[&str], aceptar: &str) -> Result<(), String> {
    if !cfg!(target_os = "macos") {
        let _ = (titulo, cuerpo, botones, aceptar);
        return Err("cancelado".into());
    }
    let lista = botones.iter().map(|boton| format!("\"{}\"", escapar(boton))).collect::<Vec<_>>().join(", ");
    let script = format!(
        "display dialog \"{}\" buttons {{{lista}}} default button \"{}\" cancel button \"Cancelar\" with title \"{}\"",
        escapar(cuerpo),
        escapar(aceptar),
        escapar(titulo)
    );
    let estado = Command::new("osascript").arg("-e").arg(script).stdin(Stdio::null()).status().map_err(|error| error.to_string())?;
    if estado.success() { Ok(()) } else { Err("cancelado".into()) }
}

fn escapar(texto: &str) -> String {
    texto.replace('\\', "\\\\").replace('"', "\\\"")
}

#[allow(dead_code)]
pub fn leer_linea() -> String {
    let mut linea = String::new();
    let _ = std::io::stdin().lock().read_line(&mut linea);
    let _ = std::io::stdout().flush();
    linea.trim().to_string()
}
