use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};
use sha2::{Digest, Sha256};

#[derive(Clone, Debug, PartialEq)]
pub struct Marcador {
    pub version: String,
    pub sha256: String,
    pub msi: String,
    pub arranque: u64,
    pub intento: Option<u64>,
}

fn puede_lanzar(intento: Option<u64>) -> bool {
    intento.is_none()
}

pub fn anotar_latido(cuerpo: &mut Value, dir: &Path) {
    let Some(marca) = leer(dir) else { return };
    let mapa = cuerpo.as_object_mut();
    let Some(mapa) = mapa else { return };
    if marca.version == env!("CARGO_PKG_VERSION") {
        mapa.insert("estadoActualizacion".into(), json!("aplicada"));
        mapa.insert("versionPendiente".into(), json!(marca.version));
        let _ = fs::remove_file(ruta(dir));
        return;
    }
    mapa.insert("estadoActualizacion".into(), json!("pendiente_reinicio"));
    mapa.insert("versionPendiente".into(), json!(marca.version));
}

pub fn preparar(dir: &Path, base: &str, token: &str, respuesta: &Value) -> Result<(), String> {
    let Some(pedido) = respuesta.get("actualizacion").filter(|valor| !valor.is_null()) else { return Ok(()) };
    let version = pedido.get("version").and_then(Value::as_str).unwrap_or("").trim().to_string();
    let suma = pedido.get("sha256").and_then(Value::as_str).unwrap_or("").trim().to_ascii_lowercase();
    let ruta_relativa = pedido.get("url").and_then(Value::as_str).unwrap_or("").trim().to_string();
    if version.is_empty() || suma.len() != 64 || ruta_relativa.is_empty() {
        return Err("el paquete de actualización está incompleto".into());
    }
    if version == env!("CARGO_PKG_VERSION") {
        let _ = fs::remove_file(ruta(dir));
        return Ok(());
    }
    if let Some(marca) = leer(dir) {
        if marca.version == version && marca.sha256 == suma && hash_de_archivo(Path::new(&marca.msi)).ok().as_deref() == Some(suma.as_str()) {
            return Ok(());
        }
    }
    let url = if ruta_relativa.starts_with("http://") || ruta_relativa.starts_with("https://") {
        ruta_relativa
    } else {
        crate::red::unir(base, &ruta_relativa)
    };
    let bytes = crate::red::get_bytes(&url, Some(token))?;
    let suma_real = sha256_hex(&bytes);
    if suma_real != suma {
        return Err("el paquete no coincide con el hash publicado".into());
    }
    let carpeta = dir.join("pendiente");
    fs::create_dir_all(&carpeta).map_err(|error| error.to_string())?;
    let msi = carpeta.join(format!("gobernanza-{version}.paquete"));
    fs::write(&msi, &bytes).map_err(|error| error.to_string())?;
    guardar(dir, &Marcador { version, sha256: suma, msi: msi.display().to_string(), arranque: id_arranque(), intento: None })?;
    Ok(())
}

pub fn aplicar_en_caliente(dir: &Path) -> bool {
    let Some(mut marca) = leer(dir) else { return false };
    if marca.version == env!("CARGO_PKG_VERSION") {
        let _ = fs::remove_file(ruta(dir));
        return false;
    }
    if !puede_lanzar(marca.intento) {
        return false;
    }
    let paquete = PathBuf::from(&marca.msi);
    let Ok(bytes) = fs::read(&paquete) else { return false };
    if sha256_hex(&bytes) != marca.sha256 {
        return false;
    }
    let Ok(archivos) = crate::paquete::leer(&bytes) else {
        marca.intento = Some(id_arranque());
        let _ = guardar(dir, &marca);
        return false;
    };
    let staging = dir.join("pendiente").join("nueva");
    if fs::create_dir_all(&staging).is_err() {
        return false;
    }
    for (nombre, datos) in &archivos {
        let destino = staging.join(nombre);
        if let Some(padre) = destino.parent() {
            if fs::create_dir_all(padre).is_err() {
                return false;
            }
        }
        if fs::write(destino, datos).is_err() {
            return false;
        }
    }
    if !lanzar_reemplazo(dir, &staging) {
        return false;
    }
    marca.intento = Some(id_arranque());
    let _ = guardar(dir, &marca);
    true
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher.finalize().iter().map(|byte| format!("{byte:02x}")).collect()
}

fn hash_de_archivo(ruta: &Path) -> Result<String, String> {
    let bytes = fs::read(ruta).map_err(|error| error.to_string())?;
    Ok(sha256_hex(&bytes))
}

fn ruta(dir: &Path) -> PathBuf {
    dir.join("pendiente").join("aplicar.json")
}

fn leer(dir: &Path) -> Option<Marcador> {
    let texto = fs::read_to_string(ruta(dir)).ok()?;
    let valor: Value = serde_json::from_str(&texto).ok()?;
    Some(Marcador {
        version: valor.get("version")?.as_str()?.to_string(),
        sha256: valor.get("sha256")?.as_str()?.to_string(),
        msi: valor.get("msi")?.as_str()?.to_string(),
        arranque: valor.get("arranque")?.as_u64()?,
        intento: valor.get("intento").and_then(Value::as_u64),
    })
}

fn guardar(dir: &Path, marca: &Marcador) -> Result<(), String> {
    let carpeta = dir.join("pendiente");
    fs::create_dir_all(&carpeta).map_err(|error| error.to_string())?;
    let cuerpo = json!({
        "version": marca.version,
        "sha256": marca.sha256,
        "msi": marca.msi,
        "arranque": marca.arranque,
        "intento": marca.intento,
    });
    fs::write(ruta(dir), cuerpo.to_string()).map_err(|error| error.to_string())
}

pub fn id_arranque() -> u64 {
    crate::sistema::id_arranque()
}

fn lanzar_reemplazo(_dir: &Path, staging: &Path) -> bool {
    let carpeta = staging.display().to_string();
    crate::sistema::lanzar_desprendido(&["aplicar-local".into(), "--dir".into(), carpeta]).is_ok()
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn un_intento_fallido_no_se_repite() {
        assert!(puede_lanzar(None));
        assert!(!puede_lanzar(Some(1)));
    }

    #[test]
    fn el_latido_informa_la_descarga_pendiente() {
        let dir = std::env::temp_dir().join(format!("gob-act-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("pendiente")).unwrap();
        guardar(&dir, &Marcador {
            version: "9.9.9".into(),
            sha256: "ab".repeat(32),
            msi: dir.join("pendiente").join("gobernanza-9.9.9.paquete").display().to_string(),
            arranque: 10,
            intento: None,
        }).unwrap();
        let mut cuerpo = json!({"contrato": 1});
        anotar_latido(&mut cuerpo, &dir);
        assert_eq!(cuerpo["estadoActualizacion"], "pendiente_reinicio");
        assert_eq!(cuerpo["versionPendiente"], "9.9.9");
        let _ = fs::remove_dir_all(&dir);
    }
}
