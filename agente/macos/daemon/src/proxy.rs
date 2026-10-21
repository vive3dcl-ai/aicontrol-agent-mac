use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream, ToSocketAddrs};
use std::sync::{Arc, Mutex};
use std::sync::OnceLock;
use std::time::Duration;

const TIEMPO_CONEXION: Duration = Duration::from_secs(5);

pub fn tiempo_conexion_segs() -> u64 {
    TIEMPO_CONEXION.as_secs()
}

use crate::ca::{Autoridad, Hoja};
use crate::contrato;
use crate::estado::{self, Estado};
use crate::politica::{self, Decision, Regla};
use crate::red;

const DIRECCION: &str = "127.0.0.1:8079";

pub fn servir(estado_ruta: std::path::PathBuf, flags_url: String, token_archivo: String, ca: Autoridad) {
    let ca = Arc::new(ca);
    let listener = match TcpListener::bind(DIRECCION) {
        Ok(listener) => listener,
        Err(error) => {
            eprintln!("proxy detenido: {error}");
            return;
        }
    };
    println!("proxy de IA en {DIRECCION}");
    for conexion in listener.incoming() {
        let Ok(stream) = conexion else { continue };
        let ca = Arc::clone(&ca);
        let estado_ruta = estado_ruta.clone();
        let flags_url = flags_url.clone();
        let token_archivo = token_archivo.clone();
        std::thread::spawn(move || {
            if let Err(error) = atender(stream, &ca, &estado_ruta, &flags_url, &token_archivo) {
                eprintln!("proxy: {error}");
            }
        });
    }
}

fn atender(mut cliente: TcpStream, ca: &Autoridad, estado_ruta: &std::path::Path, flags_url: &str, token_archivo: &str) -> Result<(), String> {
    let _ = cliente.set_nodelay(true);
    cliente.set_read_timeout(Some(Duration::from_secs(10))).ok();
    let pedido = leer_cabeceras(&mut cliente)?;
    let mut lineas = pedido.lines();
    let primera = lineas.next().unwrap_or("");
    if !primera.starts_with("CONNECT ") {
        let _ = cliente.write_all(b"HTTP/1.1 405 Method Not Allowed\r\nConnection: close\r\n\r\n");
        return Ok(());
    }
    let host = primera.split_whitespace().nth(1).unwrap_or("");
    let nombre = host.split(':').next().unwrap_or(host).to_string();
    let estado = leer_estado_cacheado(estado_ruta);
    let reglas = estado::reglas_vivas(&estado);
    match politica::decidir(&nombre, &reglas) {
        Decision::Tunel => tunel(cliente, host, &estado),
        Decision::Aviso => mitm(cliente, host, &nombre, ca, &reglas, &estado, flags_url, token_archivo, Modo::Aviso),
        Decision::Formula => mitm(cliente, host, &nombre, ca, &reglas, &estado, flags_url, token_archivo, Modo::Formula),
        Decision::Mitm => mitm(cliente, host, &nombre, ca, &reglas, &estado, flags_url, token_archivo, Modo::Captura),
    }
}

fn leer_estado_cacheado(ruta: &std::path::Path) -> Estado {
    type Entrada = (std::path::PathBuf, Option<std::time::SystemTime>, u64, Estado);
    static CACHE: OnceLock<Mutex<Option<Entrada>>> = OnceLock::new();
    let metadata = std::fs::metadata(ruta).ok();
    let modificada = metadata.as_ref().and_then(|dato| dato.modified().ok());
    let largo = metadata.as_ref().map(|dato| dato.len()).unwrap_or(0);
    let cache = CACHE.get_or_init(|| Mutex::new(None));
    if let Ok(guardada) = cache.lock() {
        if let Some((ruta_guardada, fecha_guardada, largo_guardado, estado)) = guardada.as_ref() {
            if ruta_guardada == ruta && *fecha_guardada == modificada && *largo_guardado == largo {
                return estado.clone();
            }
        }
    }
    let actual = estado::leer(ruta);
    if let Ok(mut guardada) = cache.lock() {
        *guardada = Some((ruta.to_path_buf(), modificada, largo, actual.clone()));
    }
    actual
}

#[derive(Clone, Copy)]
enum Modo {
    Aviso,
    Formula,
    Captura,
}

fn tunel(mut cliente: TcpStream, host: &str, estado: &Estado) -> Result<(), String> {
    let destino = match dial(host, estado) {
        Ok(stream) => stream,
        Err(error) => {
            let _ = cliente.write_all(b"HTTP/1.1 502 Bad Gateway\r\nConnection: close\r\n\r\n");
            return Err(error);
        }
    };
    cliente.write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n").map_err(|error| error.to_string())?;
    cliente.set_read_timeout(None).ok();
    destino.set_read_timeout(None).ok();
    copiar_ambos(cliente, destino);
    Ok(())
}

fn mitm(
    mut cliente: TcpStream,
    host: &str,
    nombre: &str,
    ca: &Autoridad,
    reglas: &[Regla],
    estado: &Estado,
    flags_url: &str,
    token_archivo: &str,
    modo: Modo,
) -> Result<(), String> {
    let upstream = match dial(host, estado) {
        Ok(stream) => stream,
        Err(error) => {
            let _ = cliente.write_all(b"HTTP/1.1 502 Bad Gateway\r\nConnection: close\r\n\r\n");
            return Err(error);
        }
    };
    if cliente.write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n").is_err() {
        return Ok(());
    }
    let hoja = match ca.hoja(nombre) {
        Ok(hoja) => hoja,
        Err(_) => {
            copiar_ambos(cliente, upstream);
            return Ok(());
        }
    };
    let mut tls_cliente = match envolver_servidor(cliente, &hoja) {
        Ok(stream) => stream,
        Err(_) => return Ok(()),
    };
    let mut tls_upstream = match envolver_cliente(upstream, nombre) {
        Ok(stream) => stream,
        Err(_) => return Ok(()),
    };
    let cabeceras = match leer_cabeceras(&mut tls_cliente) {
        Ok(texto) => texto,
        Err(_) => return Ok(()),
    };
    let primera = cabeceras.lines().next().unwrap_or("");
    let uri = primera.split_whitespace().nth(1).unwrap_or("");
    match modo {
        Modo::Formula | Modo::Aviso => {
            if let Some(regla) = politica::buscar_regla(nombre, reglas) {
                if matches!(modo, Modo::Formula) && !politica::cumple_receta(uri, &regla.receta) {
                    pasar_solicitud_normal(&mut tls_cliente, &mut tls_upstream, &cabeceras);
                    return Ok(());
                }
                let pagina = if regla.accion == "redirigir" {
                    politica::pagina_redireccion(nombre, &regla.redireccion_nombre, &regla.redireccion_url)
                } else {
                    politica::pagina_bloqueo(nombre)
                };
                let _ = escribir_html(&mut tls_cliente, &pagina);
                informar_bloqueo(&regla, nombre, estado, flags_url, token_archivo);
            } else {
                pasar_solicitud_normal(&mut tls_cliente, &mut tls_upstream, &cabeceras);
            }
        }
        Modo::Captura => {
            let mut cuerpo = Vec::new();
            if let Some(largo) = longitud(&cabeceras) {
                cuerpo.resize(largo.min(1_048_576), 0);
                let _ = std::io::Read::read_exact(&mut tls_cliente, &mut cuerpo);
            }
            let _ = tls_upstream.write_all(cabeceras.as_bytes());
            if !cuerpo.is_empty() {
                let _ = tls_upstream.write_all(&cuerpo);
            }
            let (consulta, modelo) = crate::captura::extraer_entrada(&cuerpo);
            relevar(tls_cliente, tls_upstream);
            informar_conexion(nombre, estado, flags_url, token_archivo);
            if !consulta.is_empty() {
                informar_evento(nombre, &consulta, &modelo, estado, flags_url, token_archivo);
            }
        }
    }
    Ok(())
}

fn pasar_solicitud_normal<C: Read + Write, S: Read + Write>(cliente: &mut C, servidor: &mut S, cabeceras: &str) {
    let cabeceras = forzar_cierre(cabeceras);
    if servidor.write_all(cabeceras.as_bytes()).is_err() {
        return;
    }
    if let Some(largo) = longitud(&cabeceras) {
        let mut cuerpo = cliente.take(largo as u64);
        if std::io::copy(&mut cuerpo, servidor).is_err() {
            return;
        }
    }
    let _ = servidor.flush();
    let _ = std::io::copy(servidor, cliente);
}

fn forzar_cierre(cabeceras: &str) -> String {
    let mut salida = String::new();
    for (indice, linea) in cabeceras.trim_end_matches("\r\n").split("\r\n").enumerate() {
        let nombre = linea.split_once(':').map(|(nombre, _)| nombre.trim()).unwrap_or("");
        if indice > 0 && (nombre.eq_ignore_ascii_case("connection") || nombre.eq_ignore_ascii_case("proxy-connection")) {
            continue;
        }
        salida.push_str(linea);
        salida.push_str("\r\n");
    }
    salida.push_str("Connection: close\r\n\r\n");
    salida
}

fn escribir_html(cliente: &mut impl Write, pagina: &str) -> Result<(), String> {
    let cuerpo = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nCache-Control: no-store\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{pagina}",
        pagina.len()
    );
    cliente.write_all(cuerpo.as_bytes()).map_err(|error| error.to_string())
}

fn informar_bloqueo(regla: &Regla, host: &str, estado: &Estado, flags_url: &str, token_archivo: &str) {
    let Ok(url) = estado::url_servidor(flags_url, estado) else { return };
    let Ok(token) = std::fs::read_to_string(token_archivo) else { return };
    let cuerpo = serde_json::json!({
        "contrato": 1,
        "politicaId": regla.id,
        "destino": host,
        "accion": regla.accion,
        "resultado": "aplicado",
        "ocurridoEn": estado::ahora_rfc3339(),
    });
    if contrato::validar_contrato("resultado-bloqueo", &cuerpo).is_err() {
        return;
    }
    publicar(&red::unir(&url, "/v1/ingesta/bloqueos"), token.trim(), &cuerpo);
}

fn publicar(url: &str, token: &str, cuerpo: &serde_json::Value) {
    let _ = crate::sesion::publicar(url, token, cuerpo);
}

fn longitud(cabeceras: &str) -> Option<usize> {
    cabeceras.lines().find_map(|linea| {
        let (nombre, valor) = linea.split_once(':')?;
        if nombre.eq_ignore_ascii_case("content-length") { valor.trim().parse().ok() } else { None }
    })
}

fn informar_evento(host: &str, consulta: &str, modelo: &str, estado: &Estado, flags_url: &str, token_archivo: &str) {
    let Ok(url) = estado::url_servidor(flags_url, estado) else { return };
    let Ok(token) = std::fs::read_to_string(token_archivo) else { return };
    let mut cuerpo = serde_json::json!({
        "contrato": 1,
        "proveedor": if politica::es_host_ia(host) { "openai" } else { host },
        "superficie": "web",
        "consulta": consulta,
        "ocurridoEn": estado::ahora_rfc3339(),
    });
    if !modelo.is_empty() && modelo != "auto" {
        cuerpo["modelo"] = serde_json::Value::String(modelo.to_string());
    }
    if contrato::validar_contrato("evento-ia", &cuerpo).is_err() {
        return;
    }
    publicar(&red::unir(&url, "/v1/ingesta/eventos-ia"), token.trim(), &cuerpo);
}

fn informar_conexion(host: &str, estado: &Estado, flags_url: &str, token_archivo: &str) {
    let Ok(url) = estado::url_servidor(flags_url, estado) else { return };
    let Ok(token) = std::fs::read_to_string(token_archivo) else { return };
    let cuerpo = serde_json::json!({
        "contrato": 1,
        "destino": format!("{host}:443"),
        "sni": host,
        "proceso": "navegador",
        "ocurridoEn": estado::ahora_rfc3339(),
    });
    if contrato::validar_contrato("conexion", &cuerpo).is_err() {
        return;
    }
    publicar(&red::unir(&url, "/v1/ingesta/conexiones"), token.trim(), &cuerpo);
}

fn dial(host: &str, estado: &Estado) -> Result<TcpStream, String> {
    let destino = if host.contains(':') { host.to_string() } else { format!("{host}:443") };
    if let Some(proxy) = proxy_anterior(estado) {
        return conectar_por_proxy(&proxy, &destino);
    }
    let (nombre, puerto) = destino.rsplit_once(':').ok_or("destino inválido")?;
    conectar_tcp(nombre, puerto.parse::<u16>().map_err(|_| "puerto inválido".to_string())?)
}

fn conectar_tcp(host: &str, puerto: u16) -> Result<TcpStream, String> {
    let host = host.to_string();
    let (envio, recibo) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = envio.send((host.as_str(), puerto).to_socket_addrs().map(|dirs| dirs.take(4).collect::<Vec<_>>()));
    });
    let direcciones = match recibo.recv_timeout(Duration::from_secs(3)) {
        Ok(Ok(direcciones)) => direcciones,
        Ok(Err(error)) => return Err(error.to_string()),
        Err(_) => return Err("el nombre tardó demasiado".into()),
    };
    if direcciones.is_empty() {
        return Err("sin dirección".into());
    }
    let (envio, recibo) = std::sync::mpsc::channel();
    for direccion in direcciones {
        let envio = envio.clone();
        std::thread::spawn(move || {
            let _ = envio.send(TcpStream::connect_timeout(&direccion, TIEMPO_CONEXION));
        });
    }
    drop(envio);
    let mut ultimo = "sin dirección".to_string();
    let limite = std::time::Instant::now() + TIEMPO_CONEXION + Duration::from_millis(400);
    while std::time::Instant::now() < limite {
        match recibo.recv_timeout(Duration::from_millis(200)) {
            Ok(Ok(stream)) => {
                let _ = stream.set_nodelay(true);
                return Ok(stream);
            }
            Ok(Err(error)) => ultimo = error.to_string(),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    Err(ultimo)
}

fn conectar_por_proxy(proxy: &str, destino: &str) -> Result<TcpStream, String> {
    let (host, puerto) = proxy.rsplit_once(':').ok_or("proxy inválido")?;
    let mut stream = conectar_tcp(host, puerto.parse::<u16>().map_err(|_| "puerto inválido".to_string())?)?;
    let pedido = format!("CONNECT {destino} HTTP/1.1\r\nHost: {destino}\r\n\r\n");
    stream.write_all(pedido.as_bytes()).map_err(|error| error.to_string())?;
    let respuesta = leer_cabeceras(&mut stream)?;
    if !respuesta.starts_with("HTTP/1.1 200") && !respuesta.starts_with("HTTP/1.0 200") {
        return Err("el proxy anterior rechazó el túnel".into());
    }
    Ok(stream)
}

pub fn proxy_anterior(estado: &Estado) -> Option<String> {
    for clave in ["HTTPS_PROXY", "https_proxy", "HTTP_PROXY", "http_proxy"] {
        if let Ok(valor) = std::env::var(clave) {
            if let Some(normal) = politica::normalizar_proxy(&valor) {
                return Some(normal);
            }
        }
    }
    let _ = estado;
    None
}

fn envolver_servidor(tcp: TcpStream, hoja: &Hoja) -> Result<rustls::StreamOwned<rustls::ServerConnection, TcpStream>, String> {
    let certs = hoja.cadena.iter().map(|der| rustls::pki_types::CertificateDer::from(der.clone())).collect();
    let clave = rustls::pki_types::PrivateKeyDer::Pkcs8(rustls::pki_types::PrivatePkcs8KeyDer::from(hoja.clave.clone()));
    let mut config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, clave)
        .map_err(|error| error.to_string())?;
    config.alpn_protocols = vec![b"http/1.1".to_vec()];
    let sesion = rustls::ServerConnection::new(std::sync::Arc::new(config)).map_err(|error| error.to_string())?;
    Ok(rustls::StreamOwned::new(sesion, tcp))
}

fn envolver_cliente(tcp: TcpStream, host: &str) -> Result<rustls::StreamOwned<rustls::ClientConnection, TcpStream>, String> {
    let mut raices = rustls::RootCertStore::empty();
    raices.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let mut config = rustls::ClientConfig::builder().with_root_certificates(raices).with_no_client_auth();
    config.alpn_protocols = vec![b"http/1.1".to_vec()];
    let nombre = rustls::pki_types::ServerName::try_from(host.to_string()).map_err(|_| "host TLS inválido")?;
    let sesion = rustls::ClientConnection::new(std::sync::Arc::new(config), nombre).map_err(|error| error.to_string())?;
    Ok(rustls::StreamOwned::new(sesion, tcp))
}

fn leer_cabeceras(stream: &mut impl Read) -> Result<String, String> {
    let mut datos = Vec::new();
    let mut byte = [0u8; 1];
    while datos.len() < 64 * 1024 {
        let n = stream.read(&mut byte).map_err(|error| error.to_string())?;
        if n == 0 {
            break;
        }
        datos.push(byte[0]);
        if datos.ends_with(b"\r\n\r\n") {
            break;
        }
    }
    String::from_utf8(datos).map_err(|_| "cabeceras no son utf-8".into())
}

fn copiar_ambos(mut izquierda: TcpStream, mut derecha: TcpStream) {
    let Ok(mut hacia_derecha) = derecha.try_clone() else { return };
    let Ok(mut hacia_izquierda) = izquierda.try_clone() else { return };
    let ida = std::thread::spawn(move || {
        let _ = std::io::copy(&mut izquierda, &mut hacia_derecha);
        let _ = hacia_derecha.shutdown(Shutdown::Both);
    });
    let vuelta = std::thread::spawn(move || {
        let _ = std::io::copy(&mut derecha, &mut hacia_izquierda);
        let _ = hacia_izquierda.shutdown(Shutdown::Both);
    });
    let _ = ida.join();
    let _ = vuelta.join();
}

fn relevar(mut cliente: rustls::StreamOwned<rustls::ServerConnection, TcpStream>, mut servidor: rustls::StreamOwned<rustls::ClientConnection, TcpStream>) {
    aflojar(cliente.get_mut());
    aflojar(servidor.get_mut());
    copiar_ambos_tls(cliente, servidor);
}

fn aflojar(tcp: &TcpStream) {
    let _ = tcp.set_nodelay(true);
    let _ = tcp.set_read_timeout(Some(Duration::from_millis(80)));
    let _ = tcp.set_write_timeout(Some(Duration::from_secs(8)));
}

fn copiar_ambos_tls<L, R>(izquierda: L, derecha: R)
where
    L: Read + Write + Send + 'static,
    R: Read + Write + Send + 'static,
{
    let derecha = Arc::new(Mutex::new(derecha));
    let izquierda = Arc::new(Mutex::new(izquierda));
    let (a, b) = (Arc::clone(&izquierda), Arc::clone(&derecha));
    let (c, d) = (izquierda, derecha);
    let ida = std::thread::spawn(move || copiar_uno(a, b));
    let vuelta = std::thread::spawn(move || copiar_uno(d, c));
    let _ = ida.join();
    let _ = vuelta.join();
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::io::{Read, Write};
    use std::time::Duration;

    #[test]
    fn la_conexion_del_tunel_tiene_limite() {
        assert_eq!(tiempo_conexion_segs(), 5);
    }

    #[test]
    fn una_formula_cierra_la_conexion_para_revisar_la_siguiente_url() {
        let pedido = "GET /search?q=hola HTTP/1.1\r\nHost: www.google.com\r\nConnection: keep-alive\r\nProxy-Connection: keep-alive\r\n\r\n";
        let cerrado = forzar_cierre(pedido);
        assert!(cerrado.ends_with("Connection: close\r\n\r\n"));
        assert!(!cerrado.contains("keep-alive"));
        assert_eq!(cerrado.matches("Connection:").count(), 1);
    }

    #[test]
    fn el_tunel_copia_en_las_dos_direcciones() {
        let escucha_a = TcpListener::bind("127.0.0.1:0").unwrap();
        let escucha_b = TcpListener::bind("127.0.0.1:0").unwrap();
        let izquierda = TcpStream::connect(escucha_a.local_addr().unwrap()).unwrap();
        let derecha = TcpStream::connect(escucha_b.local_addr().unwrap()).unwrap();
        let (mut lado_a, _) = escucha_a.accept().unwrap();
        let (mut lado_b, _) = escucha_b.accept().unwrap();
        lado_a.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        lado_b.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        let copia = std::thread::spawn(move || copiar_ambos(izquierda, derecha));
        lado_a.write_all(b"ping").unwrap();
        let mut buf = [0u8; 4];
        lado_b.read_exact(&mut buf).unwrap();
        assert_eq!(&buf, b"ping");
        lado_b.write_all(b"pong").unwrap();
        lado_a.read_exact(&mut buf).unwrap();
        assert_eq!(&buf, b"pong");
        drop(lado_a);
        drop(lado_b);
        copia.join().unwrap();
    }

    #[test]
    fn una_espera_no_corta_la_copia() {
        struct Paso {
            esperas: u32,
            datos: &'static [u8],
            pos: usize,
        }
        impl Read for Paso {
            fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
                if self.esperas > 0 {
                    self.esperas -= 1;
                    return Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "espera"));
                }
                if self.pos >= self.datos.len() {
                    return Ok(0);
                }
                let n = buf.len().min(self.datos.len() - self.pos);
                buf[..n].copy_from_slice(&self.datos[self.pos..self.pos + n]);
                self.pos += n;
                Ok(n)
            }
        }
        struct Caja(Vec<u8>);
        impl Write for Caja {
            fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
                self.0.extend_from_slice(buf);
                Ok(buf.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let origen = Arc::new(Mutex::new(Paso { esperas: 2, datos: b"hola", pos: 0 }));
        let destino = Arc::new(Mutex::new(Caja(Vec::new())));
        let copia = Arc::clone(&destino);
        copiar_uno(origen, destino);
        assert_eq!(copia.lock().unwrap().0, b"hola");
    }
}

fn es_espera(error: &std::io::Error) -> bool {
    matches!(error.kind(), std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock) || error.raw_os_error() == Some(10060)
}

fn copiar_uno<A: Read, B: Write>(origen: Arc<Mutex<A>>, destino: Arc<Mutex<B>>) {
    let mut buf = [0u8; 8192];
    let mut silencio = std::time::Instant::now();
    loop {
        let lectura = {
            let mut origen = origen.lock().expect("origen");
            origen.read(&mut buf)
        };
        let n = match lectura {
            Ok(0) => break,
            Err(error) if es_espera(&error) => {
                if silencio.elapsed() > Duration::from_secs(120) {
                    break;
                }
                continue;
            }
            Err(_) => break,
            Ok(n) => n,
        };
        silencio = std::time::Instant::now();
        let mut escrito = 0;
        while escrito < n {
            let resultado = {
                let mut destino = destino.lock().expect("destino");
                destino.write(&buf[escrito..n])
            };
            match resultado {
                Ok(0) => return,
                Ok(cantidad) => escrito += cantidad,
                Err(error) if es_espera(&error) => {
                    if silencio.elapsed() > Duration::from_secs(120) {
                        return;
                    }
                }
                Err(_) => return,
            }
        }
    }
}
