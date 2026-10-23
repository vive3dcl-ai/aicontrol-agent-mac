use std::io::{Read, Write};
use std::net::TcpListener;

use crate::baja;
use crate::estado::{self, Estado};
use crate::politica;
use crate::sesion::Rutas;

pub fn servir(rutas: Rutas) {
    let listener = match TcpListener::bind("127.0.0.1:4765") {
        Ok(listener) => listener,
        Err(error) => {
            eprintln!("tarjeta detenida: {error}");
            return;
        }
    };
    println!("tarjeta en http://127.0.0.1:4765");
    for conexion in listener.incoming() {
        let Ok(mut stream) = conexion else { continue };
        let rutas = clonar(&rutas);
        std::thread::spawn(move || {
            let _ = atender(&mut stream, &rutas);
        });
    }
}

fn clonar(rutas: &Rutas) -> Rutas {
    Rutas { url: rutas.url.clone(), token: rutas.token.clone(), estado: rutas.estado.clone(), tenant: rutas.tenant.clone(), clave: rutas.clave.clone() }
}

fn atender(stream: &mut impl ReadWrite, rutas: &Rutas) -> Result<(), String> {
    let (metodo, ruta, cuerpo) = leer(stream)?;
    let ruta = ruta.split('?').next().unwrap_or("/");
    match (metodo.as_str(), ruta) {
        ("GET", "/proxy.pac") => {
            let estado = estado::leer(&rutas.estado);
            let resto = crate::proxy::proxy_anterior(&estado).map(|proxy| format!("PROXY {proxy}")).unwrap_or_else(|| "DIRECT".into());
            let pac = politica::texto_pac(&estado.sitios, &estado::reglas_vivas(&estado), &resto, &estado.modo_web);
            escribir(stream, 200, "application/x-ns-proxy-autoconfig", &pac)
        }
        ("GET", "/gobernanza.crl") => {
            let crl = std::fs::read(rutas.dir().join("ca").join("gobernanza.crl")).map_err(|error| error.to_string())?;
            escribir_bytes(stream, 200, "application/pkix-crl", &crl)
        }
        ("GET", "/estado.json") => {
            let estado = estado::leer(&rutas.estado);
            let mut vista = estado::vista_tarjeta(&estado, &rutas.url, rutas.token.is_file());
            // Exponer solo si hay una clave disponible, nunca su contenido. La
            // tarjeta necesita distinguir una cuenta enlazada de una sesión
            // que ya no puede cifrar latidos.
            vista["tieneClaveDefinitiva"] = serde_json::Value::Bool(
                !crate::configuracion::clave_definitiva().is_empty(),
            );
            escribir(stream, 200, "application/json", &vista.to_string())
        }
        ("GET", "/") => escribir(stream, 200, "text/html; charset=utf-8", &pagina(&estado::leer(&rutas.estado))),
        ("POST", "/certificado/instalar") => {
            let pedido: serde_json::Value = serde_json::from_str(&cuerpo).unwrap_or(serde_json::json!({}));
            let cerrar = pedido.get("cerrarNavegadores").and_then(|valor| valor.as_bool()).unwrap_or(false);
            match crate::sesion::instalar_certificado(rutas, cerrar) {
                Ok(pasos) => escribir(stream, 200, "application/json", &serde_json::json!({"pasos": pasos, "error": ""}).to_string()),
                Err(error) => escribir(stream, 400, "application/json", &serde_json::json!({"pasos": [], "error": error}).to_string()),
            }
        }
        ("POST", "/activar") => {
            let pedido: serde_json::Value = serde_json::from_str(&cuerpo).unwrap_or(serde_json::json!({}));
            let correo = pedido.get("correo").and_then(|valor| valor.as_str()).unwrap_or("");
            let clave = pedido.get("clave").and_then(|valor| valor.as_str()).unwrap_or("");
            match crate::sesion::activar(rutas, correo, clave) {
                Ok(()) => escribir(stream, 204, "text/plain", ""),
                Err(error) => escribir(stream, 400, "text/plain", &error),
            }
        }
        ("POST", "/ajustes/entrar") => {
            let pedido: serde_json::Value = serde_json::from_str(&cuerpo).unwrap_or(serde_json::json!({}));
            let clave = pedido.get("clave").and_then(|valor| valor.as_str()).unwrap_or("");
            match crate::sesion::autorizar(rutas, clave.trim()) {
                Ok(()) => escribir(stream, 204, "text/plain", ""),
                Err(_) => escribir(stream, 401, "text/plain", "Clave de Config incorrecta"),
            }
        }
        ("POST", "/ajustes/bloquear") => {
            crate::sesion::bloquear_ajustes(&rutas.estado);
            escribir(stream, 204, "text/plain", "")
        }
        ("POST", "/endpoint/elegir") => elegir(stream, rutas, &cuerpo, true),
        ("POST", "/ajustes/direccion") => elegir(stream, rutas, &cuerpo, false),
        ("POST", "/desinstalar/reto") => {
            if !estado::leer(&rutas.estado).ajustes_abiertos() {
                return escribir(stream, 403, "text/plain", "Ajustes bloqueados");
            }
            let (a, b) = numeros();
            let (a, b) = baja::preparar_reto(a, b);
            escribir(stream, 200, "application/json", &serde_json::json!({"a": a, "b": b}).to_string())
        }
        ("POST", "/desinstalar") => {
            let pedido: serde_json::Value = serde_json::from_str(&cuerpo).unwrap_or(serde_json::json!({}));
            let clave = pedido.get("clave").and_then(|valor| valor.as_str()).unwrap_or("");
            match crate::sesion::autorizar_baja(rutas, clave) {
                Ok(()) => {
                    if let Err(error) = crate::baja::marcar_baja(&rutas.dir()) {
                        return escribir(stream, 500, "text/plain", &error);
                    }
                    escribir(stream, 204, "text/plain", "")?;
                    crate::sesion::programar_desinstalacion(rutas);
                    Ok(())
                }
                Err(error) => escribir(stream, 403, "text/plain", &error),
            }
        }
        ("POST", "/cerrar") => {
            if !estado::leer(&rutas.estado).ajustes_abiertos() {
                return escribir(stream, 403, "text/plain", "Ajustes bloqueados");
            }
            escribir(stream, 204, "text/plain", "")?;
            let rutas = clonar(rutas);
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(200));
                crate::sesion::cerrar_hasta_inicio(&rutas);
            });
            Ok(())
        }
        _ => escribir(stream, 404, "text/plain", "no encontrado"),
    }
}

fn elegir(stream: &mut impl Write, rutas: &Rutas, cuerpo: &str, primera: bool) -> Result<(), String> {
    let pedido: serde_json::Value = serde_json::from_str(cuerpo).unwrap_or(serde_json::json!({}));
    let modo = pedido.get("modo").and_then(|valor| valor.as_str()).unwrap_or("");
    let host = pedido.get("host").and_then(|valor| valor.as_str()).unwrap_or("");
    let resultado = if primera { crate::sesion::elegir_endpoint(rutas, modo, host) } else { crate::sesion::elegir_endpoint(rutas, modo, host) };
    match resultado {
        Ok(()) => escribir(stream, 204, "text/plain", ""),
        Err(error) => escribir(stream, 400, "text/plain", &error),
    }
}

fn numeros() -> (u8, u8) {
    let ahora = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos();
    let a = (ahora % 8) as u8 + 2;
    let b = ((ahora / 8) % 8) as u8 + 2;
    (a, b)
}

fn pagina(estado: &Estado) -> String {
    format!(
        "<!doctype html><meta charset=utf-8><title>AIcontrol</title><h1>AIcontrol</h1><p>Conexión: {}</p><p>Certificado: {}</p><p>Cuenta: {}</p>",
        politica::escapar_html(&estado.conexion),
        politica::escapar_html(&estado.certificado),
        politica::escapar_html(&estado.correo),
    )
}

fn leer(stream: &mut impl Read) -> Result<(String, String, String), String> {
    let mut datos = Vec::new();
    let mut byte = [0u8; 1];
    while datos.len() < 64 * 1024 {
        let n = stream.read(&mut byte).map_err(|error| error.to_string())?;
        if n == 0 { break; }
        datos.push(byte[0]);
        if datos.ends_with(b"\r\n\r\n") { break; }
    }
    let texto = String::from_utf8(datos).map_err(|_| "petición no es utf-8".to_string())?;
    let (cabeceras, _) = texto.split_once("\r\n\r\n").unwrap_or((texto.as_str(), ""));
    let mut lineas = cabeceras.lines();
    let primera = lineas.next().unwrap_or("");
    let mut partes = primera.split_whitespace();
    let metodo = partes.next().unwrap_or("").to_string();
    let ruta = partes.next().unwrap_or("/").to_string();
    let largo = cabeceras.lines().find_map(|linea| linea.split_once(':').and_then(|(nombre, valor)| {
        if nombre.eq_ignore_ascii_case("content-length") { valor.trim().parse::<usize>().ok() } else { None }
    })).unwrap_or(0).min(1_048_576);
    let mut cuerpo = vec![0u8; largo];
    if largo > 0 {
        stream.read_exact(&mut cuerpo).map_err(|error| error.to_string())?;
    }
    Ok((metodo, ruta, String::from_utf8_lossy(&cuerpo).to_string()))
}

fn escribir(stream: &mut impl Write, codigo: u16, tipo: &str, cuerpo: &str) -> Result<(), String> {
    escribir_bytes(stream, codigo, tipo, cuerpo.as_bytes())
}

fn escribir_bytes(stream: &mut impl Write, codigo: u16, tipo: &str, cuerpo: &[u8]) -> Result<(), String> {
    let razon = match codigo { 200 => "OK", 204 => "No Content", 400 => "Bad Request", 401 => "Unauthorized", 403 => "Forbidden", 404 => "Not Found", 502 => "Bad Gateway", _ => "OK" };
    let cabeceras = format!("HTTP/1.1 {codigo} {razon}\r\nContent-Type: {tipo}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n", cuerpo.len());
    stream.write_all(cabeceras.as_bytes()).map_err(|error| error.to_string())?;
    stream.write_all(cuerpo).map_err(|error| error.to_string())
}

trait ReadWrite: Read + Write {}
impl<T: Read + Write> ReadWrite for T {}
