use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use crate::politica::{self, Grupo, Politica, Receta, Regla, Sitio};
use crate::programas::Programa;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Estado {
    #[serde(default = "conexion_vacia")]
    pub conexion: String,
    #[serde(default)]
    pub host_modo: String,
    #[serde(default)]
    pub host_personalizado: String,
    #[serde(default)]
    pub direccion_global: String,
    #[serde(default)]
    pub endpoint_elegido: bool,
    #[serde(default)]
    pub correo: String,
    #[serde(default)]
    pub enlazado: bool,
    #[serde(default)]
    pub usuario_cuenta: String,
    #[serde(default)]
    pub cuenta_id: String,
    #[serde(default)]
    pub clave_autorizacion: String,
    #[serde(default)]
    pub ajustes_autorizados: bool,
    #[serde(default)]
    pub ajustes_hasta: String,
    #[serde(default)]
    pub certificado: String,
    #[serde(default)]
    pub proxy: String,
    #[serde(default)]
    pub dispositivo_id: String,
    #[serde(default)]
    pub usuario_sesion: String,
    #[serde(default)]
    pub reglas: Vec<ReglaGuardada>,
    #[serde(default)]
    pub programas: Vec<ProgramaGuardado>,
    #[serde(default)]
    pub sitios: Vec<String>,
    #[serde(default = "modo_web_defecto")]
    pub modo_web: String,
}

fn modo_web_defecto() -> String { "aceptar".into() }

fn conexion_vacia() -> String {
    "eligiendo".into()
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReglaGuardada {
    pub id: String,
    pub host: String,
    pub accion: String,
    #[serde(default)]
    pub nombre: String,
    #[serde(default)]
    pub redireccion_nombre: String,
    #[serde(default)]
    pub redireccion_url: String,
    #[serde(default)]
    pub receta: Receta,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgramaGuardado {
    pub id: String,
    pub nombre: String,
    pub binario: String,
    pub accion: String,
}

impl Default for Estado {
    fn default() -> Self {
        Self {
            conexion: "eligiendo".into(),
            host_modo: String::new(),
            host_personalizado: String::new(),
            direccion_global: String::new(),
            endpoint_elegido: false,
            correo: String::new(),
            enlazado: false,
            usuario_cuenta: String::new(),
            cuenta_id: String::new(),
            clave_autorizacion: String::new(),
            ajustes_autorizados: false,
            ajustes_hasta: String::new(),
            certificado: String::new(),
            proxy: String::new(),
            dispositivo_id: String::new(),
            usuario_sesion: String::new(),
            reglas: Vec::new(),
            programas: Vec::new(),
            sitios: Vec::new(),
            modo_web: modo_web_defecto(),
        }
    }
}

impl Estado {
    pub fn ajustes_abiertos(&self) -> bool {
        if !self.ajustes_autorizados || self.ajustes_hasta.is_empty() {
            return false;
        }
        let Ok(hasta) = chrono_no(&self.ajustes_hasta) else { return false };
        hasta > std::time::SystemTime::now()
    }
}

fn chrono_no(texto: &str) -> Result<std::time::SystemTime, ()> {
    let Ok(dt) = time_parse(texto) else { return Err(()) };
    Ok(dt)
}

fn time_parse(texto: &str) -> Result<std::time::SystemTime, ()> {
    let texto = texto.trim().trim_end_matches('Z');
    let (fecha, hora) = texto.split_once('T').ok_or(())?;
    let mut partes = fecha.split('-');
    let _ano: i64 = partes.next().ok_or(())?.parse().map_err(|_| ())?;
    let _mes: u32 = partes.next().ok_or(())?.parse().map_err(|_| ())?;
    let _dia: u32 = partes.next().ok_or(())?.parse().map_err(|_| ())?;
    let mut reloj = hora.split(':');
    let horas: u64 = reloj.next().ok_or(())?.parse().map_err(|_| ())?;
    let minutos: u64 = reloj.next().ok_or(())?.parse().map_err(|_| ())?;
    let segundos: u64 = reloj.next().unwrap_or("0").split(['+', '.', '-']).next().unwrap_or("0").parse().unwrap_or(0);
    let dias = dias_desde_epoch(fecha).ok_or(())?;
    Ok(std::time::UNIX_EPOCH + std::time::Duration::from_secs(dias * 86400 + horas * 3600 + minutos * 60 + segundos))
}

fn dias_desde_epoch(fecha: &str) -> Option<u64> {
    let mut partes = fecha.split('-');
    let ano: i32 = partes.next()?.parse().ok()?;
    let mes: u32 = partes.next()?.parse().ok()?;
    let dia: u32 = partes.next()?.parse().ok()?;
    if !(1..=12).contains(&mes) || dia == 0 {
        return None;
    }
    let mut dias = 0u64;
    for anio in 1970..ano {
        dias += if es_bisiesto(anio) { 366 } else { 365 };
    }
    let meses = [0, 31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    for m in 1..mes {
        dias += meses[m as usize] as u64;
        if m == 2 && es_bisiesto(ano) {
            dias += 1;
        }
    }
    dias += u64::from(dia - 1);
    Some(dias)
}

fn es_bisiesto(ano: i32) -> bool {
    ano % 4 == 0 && (ano % 100 != 0 || ano % 400 == 0)
}

pub fn ahora_rfc3339() -> String {
    let segundos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs();
    let dias = segundos / 86400;
    let resto = segundos % 86400;
    let (ano, mes, dia) = fecha_de_dias(dias);
    format!("{ano:04}-{mes:02}-{dia:02}T{:02}:{:02}:{:02}Z", resto / 3600, (resto % 3600) / 60, resto % 60)
}

pub(crate) fn fecha_de_dias(mut dias: u64) -> (i32, u32, u32) {
    let mut ano = 1970i32;
    loop {
        let largo = if es_bisiesto(ano) { 366 } else { 365 };
        if dias < largo {
            break;
        }
        dias -= largo;
        ano += 1;
    }
    let meses = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut mes = 1u32;
    for largo in meses {
        let mut largo = largo as u64;
        if mes == 2 && es_bisiesto(ano) {
            largo += 1;
        }
        if dias < largo {
            return (ano, mes, (dias as u32) + 1);
        }
        dias -= largo;
        mes += 1;
    }
    (ano, 12, 31)
}

pub fn dentro_de_horas(horas: u64) -> String {
    let segundos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() + horas * 3600;
    let dias = segundos / 86400;
    let resto = segundos % 86400;
    let (ano, mes, dia) = fecha_de_dias(dias);
    format!("{ano:04}-{mes:02}-{dia:02}T{:02}:{:02}:{:02}Z", resto / 3600, (resto % 3600) / 60, resto % 60)
}

pub fn leer(ruta: &Path) -> Estado {
    let Ok(texto) = fs::read_to_string(ruta) else { return Estado::default() };
    serde_json::from_str(&texto).unwrap_or_default()
}

pub fn poner_conexion(ruta: &Path, valor: &str) {
    let mut estado = leer(ruta);
    if estado.conexion == valor {
        return;
    }
    estado.conexion = valor.to_string();
    let _ = guardar(ruta, &estado);
}

pub fn guardar(ruta: &Path, estado: &Estado) -> Result<(), String> {
    if let Some(dir) = ruta.parent() {
        fs::create_dir_all(dir).map_err(|error| error.to_string())?;
    }
    let texto = serde_json::to_string_pretty(estado).map_err(|error| error.to_string())?;
    fs::write(ruta, texto).map_err(|error| error.to_string())
}

pub const SERVIDOR_PUBLICO: &str = "https://aai.latinrouter.ai";

pub fn url_servidor(flags_url: &str, estado: &Estado) -> Result<String, String> {
    if !estado.endpoint_elegido {
        return Err("falta elegir el servidor".into());
    }
    if estado.host_modo == "personalizado" {
        if estado.host_personalizado.is_empty() {
            return Err("falta la dirección personalizada".into());
        }
        return Ok(estado.host_personalizado.trim_end_matches('/').to_string());
    }
    let _ = flags_url;
    Ok(SERVIDOR_PUBLICO.to_string())
}

pub fn recordar_direccion(actual: &str, flags_url: &str) -> String {
    for candidato in [actual, flags_url] {
        let limpio = candidato.trim().trim_end_matches('/');
        if !limpio.is_empty() && !es_direccion_local(limpio) {
            return limpio.to_string();
        }
    }
    SERVIDOR_PUBLICO.to_string()
}

fn recordar_servidor(actual: &str, nueva: &str) -> String {
    let nueva = nueva.trim().trim_end_matches('/');
    if nueva.is_empty() || es_direccion_local(nueva) {
        return recordar_direccion(actual, "");
    }
    nueva.to_string()
}

pub fn es_direccion_local(url: &str) -> bool {
    let texto = url.trim().trim_end_matches('/').to_ascii_lowercase();
    if texto.contains("aai.latinrouter.ai") {
        return false;
    }
    if es_loopback_url(&texto) {
        return true;
    }
    let Ok(destino) = crate::red::parsear_url(&texto) else {
        return false;
    };
    ipv4_privada(&destino.host)
}

fn es_loopback_url(url: &str) -> bool {
    let texto = url.trim().to_ascii_lowercase();
    texto.contains("://127.0.0.1") || texto.contains("://localhost") || texto.contains("://[::1]") || texto.contains("://0.0.0.0")
}

fn ipv4_privada(host: &str) -> bool {
    let Ok(ip) = host.parse::<std::net::Ipv4Addr>() else {
        return false;
    };
    let [a, b, _, _] = ip.octets();
    a == 10 || (a == 172 && (16..32).contains(&b)) || (a == 192 && b == 168) || (a == 169 && b == 254)
}

pub fn normalizar_endpoint(valor: &str) -> Result<String, String> {
    let mut valor = valor.trim().to_string();
    if valor.is_empty() {
        return Err("falta la dirección".into());
    }
    if !valor.contains("://") {
        valor = format!("http://{valor}");
    }
    let destino = crate::red::parsear_url(&valor)?;
    Ok(format!("{}://{}{}", destino.esquema, destino.host_header, ""))
}

pub fn aplicar_respuesta(estado: &mut Estado, respuesta: &Value) {
    if let Some(enlazado) = respuesta.get("enlazado").and_then(Value::as_bool) {
        estado.enlazado = enlazado;
    }
    if let Some(correo) = respuesta.get("correo").and_then(Value::as_str) {
        estado.correo = correo.to_string();
    }
    if let Some(id) = respuesta.get("cuentaId").and_then(Value::as_str) {
        estado.cuenta_id = id.to_string();
    }
    if let Some(usuario) = respuesta.get("usuarioCuenta").and_then(Value::as_str) {
        estado.usuario_cuenta = usuario.to_string();
    }
    if let Some(clave) = respuesta.get("claveAutorizacion").and_then(Value::as_str) {
        estado.clave_autorizacion = clave.to_string();
    }
    if let Some(direccion) = respuesta.get("direccionGlobal").and_then(Value::as_str) {
        if estado.host_modo != "personalizado" {
            estado.direccion_global = recordar_servidor(&estado.direccion_global, direccion);
        }
    }
    if let Some(id) = respuesta.get("dispositivoId").and_then(Value::as_str) {
        estado.dispositivo_id = id.to_string();
    }
    if let Some(reglas) = respuesta.get("reglas").and_then(Value::as_array) {
        estado.reglas = reglas.iter().filter_map(regla_de).collect();
    }
    if let Some(programas) = respuesta.get("programas").and_then(Value::as_array) {
        estado.programas = programas.iter().filter_map(programa_de).collect();
    }
    if let Some(paquete) = respuesta.get("paquete") {
        if let Some((reglas, sitios, modo_web)) = resolver_paquete(paquete, &estado.dispositivo_id, &estado.usuario_cuenta, &estado.correo) {
            if estado.enlazado {
                estado.reglas = reglas;
                estado.sitios = sitios;
                estado.modo_web = modo_web;
            }
        }
    }
    if !estado.enlazado {
        estado.reglas.clear();
        estado.programas.clear();
    }
    estado.conexion = "estable".into();
}

fn regla_de(valor: &Value) -> Option<ReglaGuardada> {
    Some(ReglaGuardada {
        id: valor.get("id")?.as_str()?.to_string(),
        host: valor.get("host")?.as_str()?.to_ascii_lowercase(),
        accion: valor.get("accion")?.as_str()?.to_string(),
        nombre: valor.get("nombre").and_then(Value::as_str).unwrap_or("").to_string(),
        redireccion_nombre: valor.pointer("/redireccion/nombre").and_then(Value::as_str).unwrap_or("").to_string(),
        redireccion_url: valor.pointer("/redireccion/url").and_then(Value::as_str).unwrap_or("").to_string(),
        receta: valor.get("receta").cloned().and_then(|valor| serde_json::from_value(valor).ok()).unwrap_or_default(),
    })
}

fn programa_de(valor: &Value) -> Option<ProgramaGuardado> {
    let accion = valor.get("accion")?.as_str()?;
    if accion != "permitir" && accion != "bloquear" {
        return None;
    }
    Some(ProgramaGuardado {
        id: valor.get("id")?.as_str()?.to_string(),
        nombre: valor.get("nombre")?.as_str()?.to_string(),
        binario: valor.get("binario")?.as_str()?.to_string(),
        accion: accion.to_string(),
    })
}

fn sombra_de(paquete: &Value) -> crate::politica::Sombra {
    let nodo = paquete.get("sombra");
    crate::politica::Sombra {
        accion: nodo.and_then(|valor| valor.get("accion")).and_then(Value::as_str).unwrap_or("permitir").to_string(),
        nombre: nodo.and_then(|valor| valor.pointer("/redireccion/nombre")).and_then(Value::as_str).unwrap_or("").to_string(),
        url: nodo.and_then(|valor| valor.pointer("/redireccion/url")).and_then(Value::as_str).unwrap_or("").to_string(),
    }
}

fn resolver_paquete(paquete: &Value, dispositivo: &str, usuario: &str, correo: &str) -> Option<(Vec<ReglaGuardada>, Vec<String>, String)> {
    let modo_web = paquete.get("modoWeb").and_then(Value::as_str).filter(|modo| *modo == "aceptar" || *modo == "bloquear").unwrap_or("aceptar").to_string();
    let sitios = paquete.get("sitios")?.as_array()?;
    let mut sitios: Vec<Sitio> = sitios
        .iter()
        .filter_map(|sitio| {
            Some(Sitio {
                host: sitio.get("host")?.as_str()?.to_string(),
                proveedor_id: sitio.get("proveedorId").and_then(Value::as_str).unwrap_or("").to_string(),
                endpoint_id: sitio.get("endpointId").and_then(Value::as_str).unwrap_or("").to_string(),
                superficie: sitio.get("superficie").and_then(Value::as_str).unwrap_or("").to_string(),
                receta: sitio.get("receta").cloned().and_then(|valor| serde_json::from_value(valor).ok()).unwrap_or_default(),
            })
        })
        .collect();
    if let Some(feed) = paquete.get("feedDominios").and_then(Value::as_array) {
        for host in feed.iter().filter_map(Value::as_str) {
            sitios.push(Sitio {
                host: host.to_string(),
                proveedor_id: format!("feed:{host}"),
                endpoint_id: format!("feed:{host}"),
                superficie: "web".into(),
                receta: Receta::default(),
            });
        }
    }
    let politicas = paquete.get("politicas").and_then(Value::as_array).map(|lista| {
        lista.iter().filter_map(|politica| {
            Some(Politica {
                id: politica.get("id")?.as_str()?.to_string(),
                nombre: politica.get("nombre").and_then(Value::as_str).unwrap_or("").to_string(),
                accion: politica.get("accion")?.as_str()?.to_string(),
                alcance: politica.get("alcance")?.as_str()?.to_string(),
                sujeto: politica.get("sujeto").and_then(Value::as_str).unwrap_or("").to_string(),
                destino: politica.get("destino").and_then(Value::as_str).unwrap_or("").to_string(),
                proveedor_id: politica.get("proveedorId").and_then(Value::as_str).unwrap_or("").to_string(),
                endpoint_id: politica.get("endpointId").and_then(Value::as_str).unwrap_or("").to_string(),
                actualizada_en: politica.get("actualizadaEn").and_then(Value::as_f64).unwrap_or(0.0),
                redireccion_nombre: politica.pointer("/redireccion/nombre").and_then(Value::as_str).unwrap_or("").to_string(),
                redireccion_url: politica.pointer("/redireccion/url").and_then(Value::as_str).unwrap_or("").to_string(),
            })
        }).collect::<Vec<_>>()
    }).unwrap_or_default();
    let grupos = paquete.get("grupos").and_then(Value::as_array).map(|lista| {
        lista.iter().filter_map(|grupo| {
            Some(Grupo {
                id: grupo.get("id")?.as_str()?.to_string(),
                usuarios: grupo.get("usuarios").and_then(Value::as_array).map(|usuarios| usuarios.iter().filter_map(|usuario| usuario.as_str().map(str::to_string)).collect()).unwrap_or_default(),
            })
        }).collect::<Vec<_>>()
    }).unwrap_or_default();
    let mut claves = Vec::new();
    if !usuario.is_empty() {
        claves.push(usuario.to_string());
    }
    if !correo.is_empty() {
        claves.push(correo.to_string());
    }
    let sombra = sombra_de(paquete);
    let (reglas, sitios) = politica::resolver(&sitios, &politicas, &grupos, dispositivo, dispositivo, &claves, &modo_web, &sombra);
    Some((
        reglas.into_iter().map(|regla| ReglaGuardada {
            id: regla.id,
            host: regla.host,
            accion: regla.accion,
            nombre: regla.nombre,
            redireccion_nombre: regla.redireccion_nombre,
            redireccion_url: regla.redireccion_url,
            receta: regla.receta,
        }).collect(),
        sitios,
        modo_web,
    ))
}

pub fn reglas_vivas(estado: &Estado) -> Vec<Regla> {
    estado.reglas.iter().map(|regla| Regla {
        id: regla.id.clone(),
        host: regla.host.clone(),
        accion: regla.accion.clone(),
        nombre: regla.nombre.clone(),
        redireccion_nombre: regla.redireccion_nombre.clone(),
        redireccion_url: regla.redireccion_url.clone(),
        receta: regla.receta.clone(),
    }).collect()
}

pub fn programas_vivos(estado: &Estado) -> Vec<Programa> {
    if !estado.enlazado {
        return Vec::new();
    }
    estado.programas.iter().map(|programa| Programa {
        id: programa.id.clone(),
        nombre: programa.nombre.clone(),
        binario: programa.binario.clone(),
        accion: programa.accion.clone(),
    }).collect()
}

pub fn vista_tarjeta(estado: &Estado, url_principal: &str, tiene_token: bool) -> Value {
    json!({
        "conexion": estado.conexion,
        "certificado": if estado.certificado == "instalado" { "instalado" } else { "pendiente" },
        "enlazado": estado.enlazado,
        "correo": estado.correo,
        "ajustes": estado.ajustes_abiertos(),
        "hostModo": estado.host_modo,
        "hostPersonalizado": estado.host_personalizado,
        "direccionGlobal": estado.direccion_global,
        "endpointElegido": estado.endpoint_elegido,
        "urlPrincipal": url_principal,
        "tieneToken": tiene_token,
        "versionAgente": crate::sistema::VERSION_AGENTE,
    })
}

#[allow(dead_code)]
pub fn ruta_por_defecto(base: &Path) -> PathBuf {
    base.join("Gobernanza").join("estado.json")
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use serde_json::json;

    #[test]
    fn sin_enlace_no_hay_reglas_ni_programas() {
        let mut estado = Estado::default();
        estado.enlazado = false;
        aplicar_respuesta(&mut estado, &json!({
            "enlazado": false,
            "reglas": [{"id": "1", "host": "ia.example", "accion": "bloquear"}],
            "programas": [{"id": "p", "nombre": "Hermes", "binario": "hermes.exe", "accion": "bloquear"}]
        }));
        assert!(estado.reglas.is_empty());
        assert!(programas_vivos(&estado).is_empty());
    }

    #[test]
    fn el_personalizado_no_cae_en_la_url_de_arranque() {
        let mut estado = Estado::default();
        estado.endpoint_elegido = true;
        estado.host_modo = "personalizado".into();
        estado.host_personalizado = "http://10.0.0.5:7590".into();
        assert_eq!(url_servidor("http://127.0.0.1:7590", &estado).unwrap(), "http://10.0.0.5:7590");
        estado.host_personalizado.clear();
        assert!(url_servidor("http://127.0.0.1:7590", &estado).is_err());
    }

    #[test]
    fn el_global_usa_solo_aicontrol() {
        let mut estado = Estado::default();
        estado.endpoint_elegido = true;
        estado.host_modo = "global".into();
        estado.direccion_global = "http://192.168.1.20:7590".into();
        assert_eq!(url_servidor("http://10.1.1.8:7590", &estado).unwrap(), SERVIDOR_PUBLICO);
        assert_eq!(recordar_direccion("http://192.168.20.253:7590", "http://127.0.0.1:7590"), SERVIDOR_PUBLICO);
        aplicar_respuesta(&mut estado, &json!({ "direccionGlobal": SERVIDOR_PUBLICO }));
        assert_eq!(estado.direccion_global, SERVIDOR_PUBLICO);
        estado.host_modo = "personalizado".into();
        estado.direccion_global.clear();
        aplicar_respuesta(&mut estado, &json!({ "direccionGlobal": SERVIDOR_PUBLICO }));
        assert!(estado.direccion_global.is_empty());
    }
}
