use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::estado::Estado;

const MAGIA: &[u8] = b"GOBCFG01";
const ARCHIVO: &str = "config.dat";

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Parametros {
    #[serde(default)]
    pub contrato: u32,
    #[serde(default)]
    pub url: String,
    #[serde(default, rename = "tokenArchivo")]
    pub token_archivo: String,
    #[serde(default)]
    pub estado: String,
    #[serde(default)]
    pub intervalo: String,
    #[serde(default)]
    pub tenant: String,
    #[serde(default)]
    pub token: String,
    #[serde(default, rename = "claveDefinitiva")]
    pub clave_definitiva: String,
}

pub fn carpeta_instalacion() -> PathBuf {
    if let Ok(base) = std::env::var("GOBERNANZA_DATOS") {
        if !base.trim().is_empty() {
            return PathBuf::from(base);
        }
    }
    if cfg!(target_os = "macos") {
        return PathBuf::from("/Library/Application Support/Gobernanza");
    }
    PathBuf::from(".").join("Gobernanza")
}

pub fn aplicar(flags: &mut HashMap<String, String>) -> Result<(), String> {
    aplicar_en(&carpeta_instalacion(), flags)
}

pub fn aplicar_en(carpeta: &Path, flags: &mut HashMap<String, String>) -> Result<(), String> {
    let (mut params, escribir) = match cargar(carpeta) {
        Ok(params) => (params, true),
        Err(ErrorCarga::Ausente) => (Parametros::default(), true),
        Err(ErrorCarga::Rota(_motivo)) => (Parametros::default(), false),
    };
    params = fusionar(params, flags);
    let estado_defecto = carpeta.join("estado.json");
    let token_defecto = carpeta.join("dispositivo.token");
    let ruta_estado = if params.estado.is_empty() { estado_defecto.clone() } else { PathBuf::from(&params.estado) };
    completar(&mut params, &crate::estado::leer(&ruta_estado), &token_defecto.display().to_string(), &estado_defecto.display().to_string());
    if let Ok(texto) = fs::read_to_string(&params.token_archivo) {
        let texto = texto.trim();
        if !texto.is_empty() {
            params.token = texto.to_string();
        }
    }
    rellenar_flags(flags, &params);
    if !params.token.is_empty() && !params.token_archivo.is_empty() && !Path::new(&params.token_archivo).is_file() {
        if let Some(dir) = Path::new(&params.token_archivo).parent() {
            let _ = fs::create_dir_all(dir);
        }
        let _ = fs::write(&params.token_archivo, &params.token);
    }
    if escribir {
        guardar(carpeta, &params)?;
    }
    Ok(())
}

pub fn sincronizar(url: &str, token: &Path, estado: &Path, intervalo: &str, tenant: &str) {
    let carpeta = carpeta_instalacion();
    let mut params = match cargar(&carpeta) {
        Ok(params) => params,
        Err(ErrorCarga::Ausente) => Parametros::default(),
        Err(ErrorCarga::Rota(_motivo)) => return,
    };
    if !url.trim().is_empty() {
        params.url = url.trim().trim_end_matches('/').to_string();
    }
    if !token.as_os_str().is_empty() {
        params.token_archivo = token.display().to_string();
        if let Ok(texto) = fs::read_to_string(token) {
            let texto = texto.trim();
            if !texto.is_empty() {
                params.token = texto.to_string();
            }
        }
    }
    if !estado.as_os_str().is_empty() {
        params.estado = estado.display().to_string();
    }
    if !intervalo.trim().is_empty() {
        params.intervalo = intervalo.trim().to_string();
    }
    if !tenant.trim().is_empty() {
        params.tenant = tenant.trim().to_string();
    }
    let _ = guardar(&carpeta, &params);
}

pub fn clave_definitiva() -> String {
    match cargar(&carpeta_instalacion()) {
        Ok(params) => params.clave_definitiva,
        Err(_) => String::new(),
    }
}

pub fn guardar_credenciales(token: &str, clave: &str) {
    let token = token.trim();
    let clave = clave.trim();
    if !token.starts_with("gdv_") || !clave.starts_with("gdv_") {
        return;
    }
    let carpeta = carpeta_instalacion();
    let mut params = match cargar(&carpeta) {
        Ok(params) => params,
        Err(ErrorCarga::Ausente) => Parametros::default(),
        Err(ErrorCarga::Rota(_)) => return,
    };
    if !params.token_archivo.is_empty() {
        if let Some(dir) = Path::new(&params.token_archivo).parent() {
            let _ = fs::create_dir_all(dir);
        }
        let _ = fs::write(&params.token_archivo, token);
    }
    if params.token == token && params.clave_definitiva == clave {
        return;
    }
    params.token = token.to_string();
    params.clave_definitiva = clave.to_string();
    let _ = guardar(&carpeta, &params);
}

pub fn guardar_clave_definitiva(valor: &str) {
    let valor = valor.trim();
    if valor.is_empty() {
        return;
    }
    let carpeta = carpeta_instalacion();
    let mut params = match cargar(&carpeta) {
        Ok(params) => params,
        Err(ErrorCarga::Ausente) => Parametros::default(),
        Err(ErrorCarga::Rota(_)) => return,
    };
    if params.clave_definitiva == valor {
        return;
    }
    params.clave_definitiva = valor.to_string();
    let _ = guardar(&carpeta, &params);
}

pub fn borrar() {
    let carpeta = carpeta_instalacion();
    let _ = fs::remove_file(carpeta.join(ARCHIVO));
    let _ = fs::remove_file(carpeta.join("config.key"));
    borrar_llavero();
}

pub fn fusionar(mut base: Parametros, flags: &HashMap<String, String>) -> Parametros {
    copiar_si_hay(&mut base.url, flags.get("url"));
    copiar_si_hay(&mut base.token_archivo, flags.get("token-archivo"));
    copiar_si_hay(&mut base.estado, flags.get("estado"));
    copiar_si_hay(&mut base.intervalo, flags.get("intervalo"));
    copiar_si_hay(&mut base.tenant, flags.get("tenant"));
    base
}

pub fn rellenar_flags(flags: &mut HashMap<String, String>, params: &Parametros) {
    poner_si_falta(flags, "url", &params.url);
    poner_si_falta(flags, "token-archivo", &params.token_archivo);
    poner_si_falta(flags, "estado", &params.estado);
    poner_si_falta(flags, "intervalo", &params.intervalo);
    poner_si_falta(flags, "tenant", &params.tenant);
}

pub fn completar(params: &mut Parametros, estado: &Estado, token_defecto: &str, estado_defecto: &str) {
    if params.url.trim().is_empty() {
        params.url = url_de_estado(estado);
    }
    if estado.endpoint_elegido && estado.host_modo != "personalizado" && crate::estado::es_direccion_local(&params.url) {
        params.url = crate::estado::SERVIDOR_PUBLICO.to_string();
    }
    if params.token_archivo.trim().is_empty() {
        params.token_archivo = token_defecto.to_string();
    }
    if params.estado.trim().is_empty() {
        params.estado = estado_defecto.to_string();
    }
    if params.intervalo.trim().is_empty() {
        params.intervalo = "60".to_string();
    }
    params.url = params.url.trim().trim_end_matches('/').to_string();
}

pub fn url_de_estado(estado: &Estado) -> String {
    if !estado.endpoint_elegido {
        return String::new();
    }
    if estado.host_modo == "personalizado" {
        return estado.host_personalizado.trim().trim_end_matches('/').to_string();
    }
    crate::estado::SERVIDOR_PUBLICO.to_string()
}

fn copiar_si_hay(destino: &mut String, valor: Option<&String>) {
    if let Some(valor) = valor {
        if !valor.trim().is_empty() {
            *destino = valor.trim().to_string();
        }
    }
}

fn poner_si_falta(flags: &mut HashMap<String, String>, clave: &str, valor: &str) {
    if valor.trim().is_empty() {
        return;
    }
    let vacio = flags.get(clave).map(|actual| actual.trim().is_empty()).unwrap_or(true);
    if vacio {
        flags.insert(clave.to_string(), valor.trim().to_string());
    }
}

enum ErrorCarga {
    Ausente,
    Rota(String),
}

fn cargar(carpeta: &Path) -> Result<Parametros, ErrorCarga> {
    let raw = fs::read(carpeta.join(ARCHIVO)).map_err(|_| ErrorCarga::Ausente)?;
    if raw.len() < MAGIA.len() || &raw[..MAGIA.len()] != MAGIA {
        return Err(ErrorCarga::Rota("la configuración no es de este agente".into()));
    }
    let plano = descifrar(carpeta, &raw[MAGIA.len()..]).map_err(ErrorCarga::Rota)?;
    serde_json::from_slice(&plano).map_err(|error| ErrorCarga::Rota(error.to_string()))
}

fn guardar(carpeta: &Path, params: &Parametros) -> Result<(), String> {
    fs::create_dir_all(carpeta).map_err(|error| error.to_string())?;
    let mut cuerpo = params.clone();
    cuerpo.contrato = 1;
    let plano = serde_json::to_vec(&cuerpo).map_err(|error| error.to_string())?;
    let cifrado = cifrar(carpeta, &plano)?;
    let mut archivo = Vec::with_capacity(MAGIA.len() + cifrado.len());
    archivo.extend_from_slice(MAGIA);
    archivo.extend_from_slice(&cifrado);
    let ruta = carpeta.join(ARCHIVO);
    fs::write(&ruta, archivo).map_err(|error| error.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&ruta, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

#[cfg(windows)]
fn cifrar(_carpeta: &Path, plano: &[u8]) -> Result<Vec<u8>, String> {
    dpapi(true, plano)
}

#[cfg(windows)]
fn descifrar(_carpeta: &Path, blob: &[u8]) -> Result<Vec<u8>, String> {
    dpapi(false, blob)
}

#[cfg(windows)]
fn dpapi(proteger: bool, datos: &[u8]) -> Result<Vec<u8>, String> {
    if proteger {
        // El servicio es SYSTEM y la bandeja es el usuario. La clave de equipo
        // permite que los dos abran el mismo config.dat.
        return dpapi_flags(0x5, true, datos);
    }
    dpapi_flags(0x1, false, datos).or_else(|_| dpapi_flags(0x5, false, datos))
}

#[cfg(windows)]
fn dpapi_flags(flags: u32, proteger: bool, datos: &[u8]) -> Result<Vec<u8>, String> {
    #[repr(C)]
    struct Blob {
        cb_data: u32,
        pb_data: *mut u8,
    }
    #[link(name = "crypt32")]
    extern "system" {
        fn CryptProtectData(entrada: *const Blob, descripcion: *const u16, entropia: *const Blob, reservado: *mut core::ffi::c_void, aviso: *const core::ffi::c_void, flags: u32, salida: *mut Blob) -> i32;
        fn CryptUnprotectData(entrada: *const Blob, descripcion: *mut *mut u16, entropia: *const Blob, reservado: *mut core::ffi::c_void, aviso: *const core::ffi::c_void, flags: u32, salida: *mut Blob) -> i32;
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn LocalFree(memoria: *mut core::ffi::c_void) -> *mut core::ffi::c_void;
    }
    let entrada = Blob { cb_data: datos.len() as u32, pb_data: datos.as_ptr() as *mut u8 };
    let mut salida = Blob { cb_data: 0, pb_data: std::ptr::null_mut() };
    let descripcion: Vec<u16> = "Gobernanza\0".encode_utf16().collect();
    let extra = b"gobernanza-agente-config";
    let entropia = Blob { cb_data: extra.len() as u32, pb_data: extra.as_ptr() as *mut u8 };
    let ok = unsafe {
        if proteger {
            CryptProtectData(&entrada, descripcion.as_ptr(), &entropia, std::ptr::null_mut(), std::ptr::null(), flags, &mut salida)
        } else {
            CryptUnprotectData(&entrada, std::ptr::null_mut(), &entropia, std::ptr::null_mut(), std::ptr::null(), flags, &mut salida)
        }
    };
    if ok == 0 || salida.pb_data.is_null() {
        return Err("no se pudo abrir la configuración cifrada".into());
    }
    let plano = unsafe { std::slice::from_raw_parts(salida.pb_data, salida.cb_data as usize).to_vec() };
    unsafe { LocalFree(salida.pb_data as *mut core::ffi::c_void); }
    Ok(plano)
}

#[cfg(not(windows))]
fn cifrar(carpeta: &Path, plano: &[u8]) -> Result<Vec<u8>, String> {
    use ring::aead::{Aad, LessSafeKey, Nonce, UnboundKey, AES_256_GCM};
    use ring::rand::{SecureRandom, SystemRandom};
    let clave = clave_local(carpeta)?;
    let llave = LessSafeKey::new(UnboundKey::new(&AES_256_GCM, &clave).map_err(|_| "clave de configuración inválida".to_string())?);
    let azar = SystemRandom::new();
    let mut nonce = [0u8; 12];
    azar.fill(&mut nonce).map_err(|_| "no hay aleatorio para cifrar".to_string())?;
    let mut cuerpo = plano.to_vec();
    llave.seal_in_place_append_tag(Nonce::assume_unique_for_key(nonce), Aad::empty(), &mut cuerpo).map_err(|_| "no se pudo cifrar la configuración".to_string())?;
    let mut salida = nonce.to_vec();
    salida.extend_from_slice(&cuerpo);
    Ok(salida)
}

#[cfg(not(windows))]
fn descifrar(carpeta: &Path, blob: &[u8]) -> Result<Vec<u8>, String> {
    use ring::aead::{Aad, LessSafeKey, Nonce, UnboundKey, AES_256_GCM};
    if blob.len() < 12 {
        return Err("configuración incompleta".into());
    }
    let clave = clave_local(carpeta)?;
    let llave = LessSafeKey::new(UnboundKey::new(&AES_256_GCM, &clave).map_err(|_| "clave de configuración inválida".to_string())?);
    let (nonce, cifrado) = blob.split_at(12);
    let mut cuerpo = cifrado.to_vec();
    let plano = llave.open_in_place(Nonce::assume_unique_for_key(nonce.try_into().unwrap()), Aad::empty(), &mut cuerpo).map_err(|_| "no se pudo abrir la configuración cifrada".to_string())?;
    Ok(plano.to_vec())
}

#[cfg(not(windows))]
fn clave_local(carpeta: &Path) -> Result<[u8; 32], String> {
    if let Some(clave) = clave_llavero() {
        return Ok(clave);
    }
    use ring::rand::{SecureRandom, SystemRandom};
    fs::create_dir_all(carpeta).map_err(|error| error.to_string())?;
    let ruta = carpeta.join("config.key");
    if let Ok(bytes) = fs::read(&ruta) {
        if bytes.len() == 32 {
            let mut clave = [0u8; 32];
            clave.copy_from_slice(&bytes);
            return Ok(clave);
        }
    }
    let mut clave = [0u8; 32];
    SystemRandom::new().fill(&mut clave).map_err(|_| "no hay aleatorio para la clave".to_string())?;
    fs::write(&ruta, clave).map_err(|error| error.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&ruta, fs::Permissions::from_mode(0o600));
    }
    Ok(clave)
}

fn clave_llavero() -> Option<[u8; 32]> {
    #[cfg(not(target_os = "macos"))]
    {
        return None;
    }
    #[cfg(target_os = "macos")]
    {
        if let Some(clave) = leer_llavero() {
            return Some(clave);
        }
        let mut clave = [0u8; 32];
        if ring::rand::SystemRandom::new().fill(&mut clave).is_err() {
            return None;
        }
        let hex: String = clave.iter().map(|byte| format!("{byte:02x}")).collect();
        let estado = std::process::Command::new("security")
            .args(["add-generic-password", "-U", "-a", "aicontrol", "-s", "GobernanzaAgenteConfig", "-w", &hex, "/Library/Keychains/System.keychain"])
            .status()
            .ok()?;
        if estado.success() { Some(clave) } else { None }
    }
}

#[cfg(target_os = "macos")]
fn leer_llavero() -> Option<[u8; 32]> {
    let salida = std::process::Command::new("security")
        .args(["find-generic-password", "-a", "aicontrol", "-s", "GobernanzaAgenteConfig", "-w", "/Library/Keychains/System.keychain"])
        .output()
        .ok()?;
    if !salida.status.success() {
        return None;
    }
    let hex = String::from_utf8_lossy(&salida.stdout).trim().to_string();
    if hex.len() != 64 {
        return None;
    }
    let mut clave = [0u8; 32];
    for (indice, byte) in clave.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hex[indice * 2..indice * 2 + 2], 16).ok()?;
    }
    Some(clave)
}

fn borrar_llavero() {
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("security")
            .args(["delete-generic-password", "-a", "aicontrol", "-s", "GobernanzaAgenteConfig", "/Library/Keychains/System.keychain"])
            .status();
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn un_argumento_nuevo_pisa_y_el_vacio_conserva() {
        let mut base = Parametros { url: "http://10.0.0.8:7590".into(), intervalo: "60".into(), ..Parametros::default() };
        let mut flags = HashMap::new();
        flags.insert("url".into(), "http://127.0.0.1:7590".into());
        flags.insert("intervalo".into(), String::new());
        base = fusionar(base, &flags);
        assert_eq!(base.url, "http://127.0.0.1:7590");
        assert_eq!(base.intervalo, "60");
        rellenar_flags(&mut HashMap::new(), &base);
        let mut vacios = HashMap::new();
        rellenar_flags(&mut vacios, &base);
        assert_eq!(vacios.get("url").map(String::as_str), Some("http://127.0.0.1:7590"));
        assert_eq!(vacios.get("intervalo").map(String::as_str), Some("60"));
    }

    #[test]
    fn sin_archivo_toma_el_servidor_guardado_en_el_estado() {
        let dir = std::env::temp_dir().join(format!("gobernanza-config-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let estado = Estado {
            endpoint_elegido: true,
            host_modo: "global".into(),
            direccion_global: "http://192.168.20.253:7590".into(),
            ..Estado::default()
        };
        fs::write(dir.join("estado.json"), serde_json::to_vec(&estado).unwrap()).unwrap();
        fs::write(dir.join("dispositivo.token"), "token-de-prueba").unwrap();
        let mut flags = HashMap::new();
        aplicar_en(&dir, &mut flags).unwrap();
        assert_eq!(flags.get("url").map(String::as_str), Some(crate::estado::SERVIDOR_PUBLICO));
        assert!(dir.join(ARCHIVO).is_file());
        let mut otra = HashMap::new();
        aplicar_en(&dir, &mut otra).unwrap();
        assert_eq!(otra.get("url").map(String::as_str), Some(crate::estado::SERVIDOR_PUBLICO));
        let raw = fs::read(dir.join(ARCHIVO)).unwrap();
        assert!(!raw.windows(8).any(|ventana| ventana == b"token-de"));
        assert!(!String::from_utf8_lossy(&raw).contains("192.168.20.253"));
        let _ = fs::remove_dir_all(&dir);
    }
}
