use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::Mutex;

static SALIDA: AtomicI32 = AtomicI32::new(-1);

pub fn pedir_salida(codigo: i32) {
    SALIDA.store(codigo, Ordering::SeqCst);
}

pub fn salida_pedida() -> Option<i32> {
    let codigo = SALIDA.load(Ordering::SeqCst);
    if codigo < 0 { None } else { Some(codigo) }
}

pub struct Reto {
    a: u8,
    b: u8,
    listo: bool,
}

impl Reto {
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn nuevo() -> Self {
        Self { a: 0, b: 0, listo: false }
    }

    pub fn preparar(&mut self, a: u8, b: u8) -> (u8, u8) {
        self.a = a.clamp(2, 9);
        self.b = b.clamp(2, 9);
        self.listo = true;
        (self.a, self.b)
    }

    pub fn validar(&self, respuesta: i32) -> Result<(), String> {
        if !self.listo || respuesta != i32::from(self.a) + i32::from(self.b) {
            return Err("La suma no coincide".into());
        }
        Ok(())
    }

    pub fn consumir(&mut self) {
        self.listo = false;
    }
}

pub fn ruta_marca(estado: &Path) -> PathBuf {
    estado.parent().unwrap_or(Path::new(".")).join("desinstalado")
}

pub fn esta_desinstalado(estado: &Path) -> bool {
    ruta_marca(estado).is_file()
}

pub fn quitar_marca(estado: &Path) {
    let _ = std::fs::remove_file(ruta_marca(estado));
    let _ = std::fs::remove_file(ruta_pausa(estado));
}

pub fn arranque_bloqueado(vigilancia: bool, desinstalado: bool, pausado: bool) -> bool {
    vigilancia && (desinstalado || pausado)
}

pub fn ruta_pausa(estado: &Path) -> PathBuf {
    estado.parent().unwrap_or(Path::new(".")).join("pausado")
}

pub fn pausa_vigente(guardado: u64, ahora: u64) -> bool {
    ahora.abs_diff(guardado) < 180
}

pub fn marcar_pausa(estado: &Path) {
    let _ = std::fs::write(ruta_pausa(estado), marca_arranque().to_string());
}

pub fn esta_pausado(estado: &Path) -> bool {
    let Ok(texto) = std::fs::read_to_string(ruta_pausa(estado)) else { return false };
    let Ok(guardado) = texto.trim().parse::<u64>() else { return false };
    if pausa_vigente(guardado, marca_arranque()) {
        return true;
    }
    let _ = std::fs::remove_file(ruta_pausa(estado));
    false
}

fn marca_arranque() -> u64 {
    crate::sistema::id_arranque()
}

static RETO: Mutex<Reto> = Mutex::new(Reto { a: 0, b: 0, listo: false });

pub fn preparar_reto(a: u8, b: u8) -> (u8, u8) {
    RETO.lock().expect("reto").preparar(a, b)
}

pub fn validar_reto(respuesta: i32) -> Result<(), String> {
    RETO.lock().expect("reto").validar(respuesta)
}

pub const AVISO_CONTROL: &str = "Si desinstala AIcontrol, se eliminará el control de IA por completo. Los navegadores y el sistema dejarán de usar el agente.";
pub const AVISO_IRREVERSIBLE: &str = "Se quitarán el certificado, el proxy y las políticas de los navegadores. El equipo volverá a la navegación anterior. Esta acción no se puede deshacer.";
pub const AVISO_INSTALACION: &str = "AIcontrol se instaló.\n\nPara habilitar el servicio, abra la ventana del agente y siga estos pasos:\n\n1. Defina la conexión.\n2. Instale el certificado.\n3. Complete la activación.";
pub const AVISO_DESINSTALADO: &str = "AIcontrol se desinstaló correctamente.\n\nSe quitaron el certificado, el proxy y las políticas de los navegadores.";

pub fn exige_clave(guardada: &str) -> bool {
    !guardada.trim().is_empty()
}

pub fn clave_local_correcta(guardada: &str, escrita: &str) -> bool {
    let guardada = guardada.as_bytes();
    let escrita = escrita.as_bytes();
    if guardada.is_empty() || guardada.len() != escrita.len() {
        return false;
    }
    let mut diff = 0u8;
    for (a, b) in guardada.iter().zip(escrita) {
        diff |= a ^ b;
    }
    diff == 0
}

pub fn marcar_baja(dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|error| error.to_string())?;
    std::fs::write(dir.join("baja.autorizada"), ahora_segundos().to_string()).map_err(|error| error.to_string())
}

pub fn quitar_autorizacion(dir: &Path) {
    let _ = std::fs::remove_file(dir.join("baja.autorizada"));
}

pub fn baja_vigente_en(dir: &Path) -> bool {
    let Ok(texto) = std::fs::read_to_string(dir.join("baja.autorizada")) else { return false };
    let Ok(guardado) = texto.trim().parse::<u64>() else { return false };
    baja_vigente(guardado, ahora_segundos())
}

pub fn baja_vigente(guardado: u64, ahora: u64) -> bool {
    ahora >= guardado && ahora.saturating_sub(guardado) < 900
}

fn ahora_segundos() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs()
}

pub fn consumir_reto() {
    RETO.lock().expect("reto").consumir();
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn una_suma_incorrecta_no_consume_el_reto() {
        let mut reto = Reto::nuevo();
        let (a, b) = reto.preparar(2, 3);
        assert!(reto.validar(a as i32 + b as i32 + 1).is_err());
        assert!(reto.validar(a as i32 + b as i32).is_ok());
    }

    #[test]
    fn la_marca_impide_el_filtro() {
        let dir = std::env::temp_dir().join(format!("gob-baja-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let estado = dir.join("estado.json");
        assert!(!esta_desinstalado(&estado));
        std::fs::write(ruta_marca(&estado), "desinstalado\n").unwrap();
        assert!(esta_desinstalado(&estado));
        assert!(arranque_bloqueado(true, true, false));
        assert!(!arranque_bloqueado(false, true, false));
        quitar_marca(&estado);
        assert!(!esta_desinstalado(&estado));
        assert!(pausa_vigente(1_000, 1_100));
        assert!(!pausa_vigente(1_000, 2_000));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn la_baja_exige_la_clave_de_ajustes_y_caduca() {
        assert!(!exige_clave(""));
        assert!(exige_clave("secreta"));
        assert!(clave_local_correcta("secreta", "secreta"));
        assert!(!clave_local_correcta("secreta", "otra"));
        assert!(!clave_local_correcta("", ""));
        assert!(baja_vigente(1_000, 1_500));
        assert!(!baja_vigente(1_000, 2_000));
        assert!(AVISO_CONTROL.contains("control de IA"));
        assert!(AVISO_IRREVERSIBLE.contains("no se puede deshacer"));
        assert!(AVISO_INSTALACION.contains("Defina la conexión."));
        assert!(AVISO_INSTALACION.contains("Instale el certificado."));
        assert!(AVISO_INSTALACION.contains("Complete la activación."));
        assert!(AVISO_DESINSTALADO.contains("desinstaló correctamente"));
    }
}
