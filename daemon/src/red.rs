use serde_json::Value;
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

pub struct Destino {
    pub esquema: String,
    pub host: String,
    pub port: u16,
    pub path: String,
    pub host_header: String,
}

pub fn parsear_url(url: &str) -> Result<Destino, String> {
    let (esquema, resto) = if let Some(resto) = url.strip_prefix("https://") {
        ("https", resto)
    } else if let Some(resto) = url.strip_prefix("http://") {
        ("http", resto)
    } else {
        return Err("la dirección debe ser http o https".into());
    };
    let (autoridad, path) = match resto.split_once('/') {
        Some((autoridad, path)) => (autoridad, format!("/{path}")),
        None => (resto, "/".to_string()),
    };
    if autoridad.is_empty() || autoridad.contains('@') {
        return Err("url sin host".into());
    }
    let (host, port) = if let Some((host, puerto)) = autoridad.rsplit_once(':') {
        if puerto.parse::<u16>().is_err() {
            (autoridad.to_string(), if esquema == "https" { 443 } else { 80 })
        } else {
            (host.to_string(), puerto.parse::<u16>().unwrap_or(80))
        }
    } else {
        (autoridad.to_string(), if esquema == "https" { 443 } else { 80 })
    };
    let host_header = if (esquema == "http" && port == 80) || (esquema == "https" && port == 443) {
        host.clone()
    } else {
        format!("{host}:{port}")
    };
    Ok(Destino { esquema: esquema.into(), host, port, path, host_header })
}

pub fn unir(base: &str, ruta: &str) -> String {
    format!("{}{ruta}", base.trim_end_matches('/'))
}

pub fn post_agente(url: &str, token: Option<&str>, clave: &str, cuerpo: &Value) -> Result<Value, String> {
    if clave.is_empty() {
        return post_json(url, token, cuerpo);
    }
    let sobre = crate::sobre::cerrar(clave, cuerpo)?;
    post_json(url, token, &sobre)
}

pub fn post_json(url: &str, token: Option<&str>, cuerpo: &Value) -> Result<Value, String> {
    let destino = parsear_url(url)?;
    let payload = serde_json::to_vec(cuerpo).map_err(|error| error.to_string())?;
    let mut peticion = format!(
        "POST {path} HTTP/1.1\r\nHost: {host}\r\nContent-Type: application/json\r\nAccept: application/json\r\nContent-Length: {len}\r\nConnection: close\r\n",
        path = destino.path,
        host = destino.host_header,
        len = payload.len(),
    );
    if let Some(token) = token {
        if token.contains('\n') || token.contains('\r') {
            return Err("token inválido".into());
        }
        peticion.push_str(&format!("Authorization: Bearer {token}\r\n"));
    }
    peticion.push_str("\r\n");
    let mut stream = conectar(&destino, Duration::from_secs(60))?;
    stream.write_all(peticion.as_bytes()).map_err(|error| error.to_string())?;
    stream.write_all(&payload).map_err(|error| error.to_string())?;
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).map_err(|error| error.to_string())?;
    let texto = String::from_utf8(buf).map_err(|_| "respuesta no es utf-8".to_string())?;
    let (estado, cuerpo_texto) = partir_http(&texto)?;
    if !(200..300).contains(&estado) {
        return Err(format!("HTTP {estado}: {cuerpo_texto}"));
    }
    if cuerpo_texto.trim().is_empty() {
        return Ok(Value::Null);
    }
    serde_json::from_str(cuerpo_texto).map_err(|error| format!("respuesta no es JSON: {error}"))
}

pub fn get_bytes(url: &str, token: Option<&str>) -> Result<Vec<u8>, String> {
    let destino = parsear_url(url)?;
    let mut peticion = format!(
        "GET {path} HTTP/1.1\r\nHost: {host}\r\nAccept: application/octet-stream\r\nConnection: close\r\n",
        path = destino.path,
        host = destino.host_header,
    );
    if let Some(token) = token {
        if token.contains('\n') || token.contains('\r') {
            return Err("token inválido".into());
        }
        peticion.push_str(&format!("Authorization: Bearer {token}\r\n"));
    }
    peticion.push_str("\r\n");
    let mut stream = conectar(&destino, Duration::from_secs(10 * 60))?;
    stream.write_all(peticion.as_bytes()).map_err(|error| error.to_string())?;
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).map_err(|error| error.to_string())?;
    let corte = buf.windows(4).position(|marca| marca == b"\r\n\r\n").ok_or("respuesta HTTP incompleta")?;
    let cabeceras = String::from_utf8_lossy(&buf[..corte]);
    let linea = cabeceras.lines().next().ok_or("sin línea de estado")?;
    let codigo = linea.split_whitespace().nth(1).ok_or("estado HTTP inválido")?.parse::<u16>().map_err(|_| "código HTTP inválido".to_string())?;
    if !(200..300).contains(&codigo) {
        return Err(format!("HTTP {codigo}"));
    }
    Ok(buf[corte + 4..].to_vec())
}

pub fn responde(base: &str) -> bool {
    let Ok(destino) = parsear_url(&format!("{}/salud", base.trim_end_matches('/'))) else {
        return false;
    };
    let Ok(mut stream) = conectar_en(&destino, Duration::from_secs(2), true) else {
        return false;
    };
    let peticion = format!(
        "GET {path} HTTP/1.1\r\nHost: {host}\r\nAccept: application/json\r\nConnection: close\r\n\r\n",
        path = destino.path,
        host = destino.host_header,
    );
    if stream.write_all(peticion.as_bytes()).is_err() {
        return false;
    }
    let mut buf = [0u8; 512];
    let leido = stream.read(&mut buf).unwrap_or(0);
    let texto = String::from_utf8_lossy(&buf[..leido]);
    partir_http(texto.split('\0').next().unwrap_or("")).map(|(estado, _)| (200..300).contains(&estado)).unwrap_or(false)
}

fn conectar(destino: &Destino, espera: Duration) -> Result<Box<dyn ReadWrite>, String> {
    conectar_en(destino, espera, false)
}

fn conectar_en(destino: &Destino, espera: Duration, corto: bool) -> Result<Box<dyn ReadWrite>, String> {
    let tcp = if corto {
        let socket = (destino.host.as_str(), destino.port).to_socket_addrs().map_err(|error| error.to_string())?.next().ok_or("sin dirección")?;
        TcpStream::connect_timeout(&socket, espera).map_err(|error| format!("no se pudo conectar: {error}"))?
    } else {
        TcpStream::connect((destino.host.as_str(), destino.port)).map_err(|error| format!("no se pudo conectar: {error}"))?
    };
    tcp.set_read_timeout(Some(espera)).ok();
    tcp.set_write_timeout(Some(espera)).ok();
    if destino.esquema == "https" {
        let mut raices = rustls::RootCertStore::empty();
        raices.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        let config = rustls::ClientConfig::builder().with_root_certificates(raices).with_no_client_auth();
        let nombre = rustls::pki_types::ServerName::try_from(destino.host.clone()).map_err(|_| "host TLS inválido".to_string())?;
        let sesion = rustls::ClientConnection::new(std::sync::Arc::new(config), nombre).map_err(|error| error.to_string())?;
        Ok(Box::new(rustls::StreamOwned::new(sesion, tcp)))
    } else {
        Ok(Box::new(tcp))
    }
}

trait ReadWrite: Read + Write {}
impl<T: Read + Write> ReadWrite for T {}

pub fn partir_http(texto: &str) -> Result<(u16, &str), String> {
    let (cabeceras, cuerpo) = texto.split_once("\r\n\r\n").ok_or("respuesta HTTP incompleta")?;
    let linea = cabeceras.lines().next().ok_or("sin línea de estado")?;
    let codigo = linea.split_whitespace().nth(1).ok_or("estado HTTP inválido")?.parse::<u16>().map_err(|_| "código HTTP inválido".to_string())?;
    Ok((codigo, cuerpo))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn separa_http_y_https() {
        let local = parsear_url("http://127.0.0.1:7590/v1/dispositivos/latido").unwrap();
        assert_eq!(local.port, 7590);
        assert_eq!(local.path, "/v1/dispositivos/latido");
        let seguro = parsear_url("https://ejemplo.test").unwrap();
        assert_eq!(seguro.port, 443);
        assert!(parsear_url("ftp://ejemplo.test").is_err());
    }
}
