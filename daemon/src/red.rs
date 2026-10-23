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
    let buf = leer_respuesta(&mut stream)?;
    let (estado, cuerpo_bytes) = desarmar_respuesta(&buf)?;
    if !(200..300).contains(&estado) {
        let cuerpo_texto = String::from_utf8_lossy(&cuerpo_bytes);
        return Err(format!("HTTP {estado}: {cuerpo_texto}"));
    }
    if cuerpo_bytes.iter().all(u8::is_ascii_whitespace) {
        return Ok(Value::Null);
    }
    serde_json::from_slice(&cuerpo_bytes).map_err(|error| format!("respuesta no es JSON: {error}"))
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
    let buf = leer_respuesta(&mut stream)?;
    let (codigo, cuerpo) = desarmar_respuesta(&buf)?;
    if !(200..300).contains(&codigo) {
        return Err(format!("HTTP {codigo}"));
    }
    Ok(cuerpo)
}

const MAX_RESPUESTA: usize = 4 * 1024 * 1024;

fn leer_respuesta(stream: &mut impl Read) -> Result<Vec<u8>, String> {
    let mut buf = Vec::new();
    loop {
        if let Some(fin) = fin_respuesta(&buf)? {
            if buf.len() >= fin {
                buf.truncate(fin);
                return Ok(buf);
            }
        }
        if buf.len() >= MAX_RESPUESTA {
            return Err("respuesta HTTP demasiado grande".into());
        }
        let mut bloque = [0u8; 8192];
        match stream.read(&mut bloque) {
            Ok(0) => return Ok(buf),
            Ok(leidos) => buf.extend_from_slice(&bloque[..leidos]),
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error.to_string()),
        }
    }
}

fn metadatos_respuesta(buf: &[u8]) -> Result<Option<(usize, u16, Option<usize>, bool)>, String> {
    let Some(corte) = buf.windows(4).position(|marca| marca == b"\r\n\r\n") else { return Ok(None) };
    let cabeceras = String::from_utf8_lossy(&buf[..corte]);
    let mut lineas = cabeceras.lines();
    let linea = lineas.next().ok_or("sin línea de estado")?;
    let estado = linea.split_whitespace().nth(1).ok_or("estado HTTP inválido")?
        .parse::<u16>().map_err(|_| "código HTTP inválido".to_string())?;
    let mut largo = None;
    let mut fragmentada = false;
    for linea in lineas {
        let Some((nombre, valor)) = linea.split_once(':') else { continue };
        if nombre.eq_ignore_ascii_case("content-length") {
            largo = Some(valor.trim().parse::<usize>().map_err(|_| "Content-Length inválido".to_string())?);
        } else if nombre.eq_ignore_ascii_case("transfer-encoding") && valor.to_ascii_lowercase().contains("chunked") {
            fragmentada = true;
        }
    }
    Ok(Some((corte + 4, estado, largo, fragmentada)))
}

fn fin_respuesta(buf: &[u8]) -> Result<Option<usize>, String> {
    let Some((cuerpo, estado, largo, fragmentada)) = metadatos_respuesta(buf)? else { return Ok(None) };
    if (100..200).contains(&estado) || estado == 204 || estado == 304 {
        return Ok(Some(cuerpo));
    }
    if fragmentada {
        return fin_chunked(buf, cuerpo);
    }
    if let Some(largo) = largo {
        return cuerpo.checked_add(largo).map(Some).ok_or_else(|| "Content-Length inválido".into());
    }
    Ok(None)
}

fn fin_chunked(buf: &[u8], mut cursor: usize) -> Result<Option<usize>, String> {
    loop {
        let Some(fin_linea) = buscar_crlf(buf, cursor) else { return Ok(None) };
        let linea = String::from_utf8_lossy(&buf[cursor..fin_linea]);
        let hexadecimal = linea.split(';').next().unwrap_or("").trim();
        let largo = usize::from_str_radix(hexadecimal, 16).map_err(|_| "tamaño de bloque HTTP inválido".to_string())?;
        cursor = fin_linea + 2;
        if largo == 0 {
            loop {
                let Some(fin_trailer) = buscar_crlf(buf, cursor) else { return Ok(None) };
                if fin_trailer == cursor { return Ok(Some(cursor + 2)) }
                cursor = fin_trailer + 2;
            }
        }
        let fin = cursor.checked_add(largo).and_then(|pos| pos.checked_add(2))
            .ok_or_else(|| "tamaño de bloque HTTP inválido".to_string())?;
        if buf.len() < fin { return Ok(None) }
        if &buf[fin - 2..fin] != b"\r\n" { return Err("bloque HTTP mal terminado".into()) }
        cursor = fin;
    }
}

fn buscar_crlf(buf: &[u8], desde: usize) -> Option<usize> {
    buf.get(desde..)?.windows(2).position(|par| par == b"\r\n").map(|offset| desde + offset)
}

fn desarmar_respuesta(buf: &[u8]) -> Result<(u16, Vec<u8>), String> {
    let Some((inicio, estado, largo, fragmentada)) = metadatos_respuesta(buf)? else { return Err("respuesta HTTP incompleta".into()) };
    let cuerpo = &buf[inicio..];
    if (100..200).contains(&estado) || estado == 204 || estado == 304 {
        return Ok((estado, Vec::new()));
    }
    if fragmentada {
        return Ok((estado, decodificar_chunked(buf, inicio)?));
    }
    if let Some(largo) = largo {
        if cuerpo.len() < largo { return Err("respuesta HTTP incompleta".into()) }
        return Ok((estado, cuerpo[..largo].to_vec()));
    }
    Ok((estado, cuerpo.to_vec()))
}

fn decodificar_chunked(buf: &[u8], mut cursor: usize) -> Result<Vec<u8>, String> {
    fin_chunked(buf, cursor)?.ok_or("respuesta HTTP fragmentada incompleta")?;
    let mut decodificado = Vec::new();
    loop {
        let fin_linea = buscar_crlf(buf, cursor).ok_or("respuesta HTTP fragmentada incompleta")?;
        let linea = String::from_utf8_lossy(&buf[cursor..fin_linea]);
        let bloque = usize::from_str_radix(linea.split(';').next().unwrap_or("").trim(), 16)
            .map_err(|_| "tamaño de bloque HTTP inválido".to_string())?;
        cursor = fin_linea + 2;
        if bloque == 0 { return Ok(decodificado) }
        let fin_bloque = cursor.checked_add(bloque).ok_or("tamaño de bloque HTTP inválido")?;
        decodificado.extend_from_slice(&buf[cursor..fin_bloque]);
        cursor = fin_bloque + 2;
    }
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

    struct LectorCierreAbrupto {
        respuesta: Vec<u8>,
        enviada: bool,
    }

    impl Read for LectorCierreAbrupto {
        fn read(&mut self, salida: &mut [u8]) -> std::io::Result<usize> {
            if self.enviada {
                return Err(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "sin close_notify"));
            }
            self.enviada = true;
            let largo = self.respuesta.len().min(salida.len());
            salida[..largo].copy_from_slice(&self.respuesta[..largo]);
            Ok(largo)
        }
    }

    #[test]
    fn separa_http_y_https() {
        let local = parsear_url("http://127.0.0.1:7590/v1/dispositivos/latido").unwrap();
        assert_eq!(local.port, 7590);
        assert_eq!(local.path, "/v1/dispositivos/latido");
        let seguro = parsear_url("https://ejemplo.test").unwrap();
        assert_eq!(seguro.port, 443);
        assert!(parsear_url("ftp://ejemplo.test").is_err());
    }

    #[test]
    fn procesa_204_aunque_el_servidor_cierre_tls_sin_close_notify() {
        let bytes = b"HTTP/1.1 204 No Content\r\nConnection: close\r\n\r\n".to_vec();
        let mut lector = LectorCierreAbrupto { respuesta: bytes, enviada: false };
        let respuesta = leer_respuesta(&mut lector).unwrap();
        let (codigo, cuerpo) = desarmar_respuesta(&respuesta).unwrap();
        assert_eq!(codigo, 204);
        assert!(cuerpo.is_empty());
    }

    #[test]
    fn respeta_content_length_sin_esperar_el_cierre_tls() {
        let bytes = b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok".to_vec();
        let mut lector = LectorCierreAbrupto { respuesta: bytes, enviada: false };
        let respuesta = leer_respuesta(&mut lector).unwrap();
        let (codigo, cuerpo) = desarmar_respuesta(&respuesta).unwrap();
        assert_eq!(codigo, 200);
        assert_eq!(cuerpo, b"ok");
    }

    #[test]
    fn decodifica_respuesta_chunked() {
        let json = r#"{"error":"rechazado"}"#;
        let bytes = format!(
            "HTTP/1.1 400 Bad Request\r\nTransfer-Encoding: chunked\r\n\r\n{:x}\r\n{}\r\n0\r\n\r\n",
            json.len(), json
        );
        let (codigo, cuerpo) = desarmar_respuesta(bytes.as_bytes()).unwrap();
        assert_eq!(codigo, 400);
        assert_eq!(String::from_utf8(cuerpo).unwrap(), json);
    }
}
