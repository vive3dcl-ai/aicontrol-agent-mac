use std::collections::HashMap;
use std::time::{Duration, SystemTime};

pub const ESPERA_CIERRE: Duration = Duration::from_secs(3);

#[derive(Clone, Debug)]
pub struct Programa {
    pub id: String,
    pub nombre: String,
    pub binario: String,
    pub accion: String,
}

#[derive(Clone, Debug)]
pub struct Proceso {
    pub pid: u32,
    pub ppid: u32,
    pub comm: String,
    pub exe: String,
    pub cmdline: String,
}

#[derive(Clone, Debug)]
pub struct Acto {
    pub programa: Programa,
    pub proceso: Proceso,
    pub hijos: Vec<u32>,
    pub cerrar: bool,
    pub avisar: bool,
    pub publicar: bool,
}

#[derive(Clone)]
struct Guardado {
    programa_id: String,
    #[allow(dead_code)]
    firma: String,
}

#[derive(Default)]
pub struct Vigilancia {
    permitidos: HashMap<u32, bool>,
    primeras: HashMap<u32, SystemTime>,
    avisados: HashMap<u32, bool>,
    pids: HashMap<u32, Guardado>,
    firmas: HashMap<String, String>,
    avisos: HashMap<String, SystemTime>,
}

impl Vigilancia {
    pub fn revisar(&mut self, programas: &[Programa], procesos: &[Proceso], ahora: SystemTime, pid_agente: u32) -> Vec<Acto> {
        let mut vivos = HashMap::new();
        let mut por_pid = HashMap::new();
        for proceso in procesos {
            vivos.insert(proceso.pid, true);
            por_pid.insert(proceso.pid, proceso.clone());
        }
        self.pids.retain(|pid, _| vivos.contains_key(pid));
        let mut actos = Vec::new();
        let seguidos: Vec<u32> = self.pids.keys().copied().collect();
        for pid in seguidos {
            let Some(proceso) = por_pid.get(&pid) else { continue };
            if es_agente(proceso, pid_agente) {
                continue;
            }
            let Some(guardado) = self.pids.get(&pid) else { continue };
            let Some(programa) = programa_por_id(programas, &guardado.programa_id) else { continue };
            if programa.accion != "bloquear" {
                continue;
            }
            actos.push(Acto {
                programa: programa.clone(),
                proceso: proceso.clone(),
                hijos: descendientes(pid, procesos),
                cerrar: true,
                avisar: false,
                publicar: false,
            });
        }
        for proceso in procesos {
            if es_agente(proceso, pid_agente) || self.pids.contains_key(&proceso.pid) {
                continue;
            }
            if let Some(id) = self.firmas.get(&firma_de(proceso)).cloned() {
                if let Some(programa) = programa_por_id(programas, &id) {
                    if programa.accion == "bloquear" {
                        self.guardar(proceso, &programa.id);
                        actos.push(Acto {
                            programa: programa.clone(),
                            proceso: proceso.clone(),
                            hijos: descendientes(proceso.pid, procesos),
                            cerrar: true,
                            avisar: false,
                            publicar: true,
                        });
                    }
                }
                continue;
            }
            let mut bloqueo = None;
            let mut permiso = None;
            for programa in programas {
                if !coincide(proceso, &programa.binario) {
                    continue;
                }
                if programa.accion == "bloquear" && bloqueo.is_none() {
                    bloqueo = Some(programa);
                }
                if programa.accion == "permitir" && permiso.is_none() {
                    permiso = Some(programa);
                }
            }
            if let Some(programa) = bloqueo {
                let inicio = *self.primeras.entry(proceso.pid).or_insert(ahora);
                if ahora.duration_since(inicio).unwrap_or_default() < ESPERA_CIERRE {
                    continue;
                }
                let mut avisar = self.aviso_nuevo(&programa.id, ahora);
                for generado in procesos_generados(proceso, procesos) {
                    if self.pids.contains_key(&generado.pid) {
                        continue;
                    }
                    self.guardar(&generado, &programa.id);
                    actos.push(Acto {
                        programa: programa.clone(),
                        proceso: generado.clone(),
                        hijos: descendientes(generado.pid, procesos),
                        cerrar: true,
                        avisar,
                        publicar: true,
                    });
                    self.avisados.insert(generado.pid, true);
                    avisar = false;
                }
                continue;
            }
            if let Some(programa) = permiso {
                if self.permitidos.contains_key(&proceso.pid) {
                    continue;
                }
                self.permitidos.insert(proceso.pid, true);
                actos.push(Acto {
                    programa: programa.clone(),
                    proceso: proceso.clone(),
                    hijos: descendientes(proceso.pid, procesos),
                    cerrar: false,
                    avisar: false,
                    publicar: true,
                });
            }
        }
        self.primeras.retain(|pid, _| vivos.contains_key(pid));
        self.permitidos.retain(|pid, _| vivos.contains_key(pid));
        self.avisados.retain(|pid, _| vivos.contains_key(pid));
        actos
    }

    fn guardar(&mut self, proceso: &Proceso, programa_id: &str) {
        let firma = firma_de(proceso);
        self.pids.insert(
            proceso.pid,
            Guardado { programa_id: programa_id.to_string(), firma: firma.clone() },
        );
        if !firma.is_empty() {
            self.firmas.insert(firma, programa_id.to_string());
        }
    }

    fn aviso_nuevo(&mut self, id: &str, ahora: SystemTime) -> bool {
        if let Some(ultimo) = self.avisos.get(id) {
            if ahora.duration_since(*ultimo).unwrap_or_default() < Duration::from_secs(120) {
                return false;
            }
        }
        self.avisos.insert(id.to_string(), ahora);
        true
    }
}

fn programa_por_id<'a>(programas: &'a [Programa], id: &str) -> Option<&'a Programa> {
    programas.iter().find(|programa| programa.id == id)
}

fn firma_de(proceso: &Proceso) -> String {
    let linea = proceso.cmdline.trim();
    if !linea.is_empty() {
        return linea.to_string();
    }
    if !proceso.exe.is_empty() {
        return proceso.exe.clone();
    }
    proceso.comm.trim().to_string()
}

pub fn coincide(proceso: &Proceso, binario: &str) -> bool {
    let esperado = binario.trim().to_ascii_lowercase();
    if esperado.is_empty() || es_aviso_propio(proceso) {
        return false;
    }
    let nombre = nombre_del_proceso(proceso).to_ascii_lowercase();
    if nombre == esperado {
        return true;
    }
    esperado.len() > 15 && nombre == esperado[..15]
}

fn nombre_del_proceso(proceso: &Proceso) -> String {
    let base = base_de(&proceso.exe);
    if !proceso.exe.is_empty() && !es_interprete(&base) {
        return base;
    }
    if let Some(ruta) = script_en_linea(proceso) {
        return base_de(&ruta);
    }
    let comm = proceso.comm.trim();
    if !comm.is_empty() && !es_interprete(comm) {
        return comm.to_string();
    }
    if !proceso.exe.is_empty() {
        return base;
    }
    String::new()
}

pub fn ruta_del_programa(proceso: &Proceso) -> String {
    if es_interprete(&base_de(&proceso.exe)) {
        let linea = proceso.cmdline.trim();
        if !linea.is_empty() {
            return linea.to_string();
        }
    }
    if !proceso.exe.is_empty() {
        return proceso.exe.clone();
    }
    let linea = proceso.cmdline.trim();
    if !linea.is_empty() {
        return linea.to_string();
    }
    "desconocido".into()
}

fn es_interprete(nombre: &str) -> bool {
    let nombre = nombre.to_ascii_lowercase();
    matches!(
        nombre.as_str(),
        "python" | "python.exe" | "python3" | "python3.exe" | "node" | "node.exe" | "nodejs"
            | "java" | "java.exe" | "ruby" | "perl" | "deno" | "bun"
    ) || nombre.starts_with("python3.")
        || nombre.starts_with("python2.")
}

fn script_en_linea(proceso: &Proceso) -> Option<String> {
    let campos: Vec<&str> = proceso.cmdline.split_whitespace().collect();
    let mut i = 1;
    while i < campos.len() {
        let arg = campos[i];
        if arg == "-m" || arg == "-c" || arg == "-W" {
            i += 2;
            continue;
        }
        if arg.starts_with('-') || (!arg.contains('/') && !arg.contains('\\')) {
            i += 1;
            continue;
        }
        let base = base_de(arg);
        if !es_interprete(&base) {
            return Some(arg.to_string());
        }
        i += 1;
    }
    None
}

fn es_aviso_propio(proceso: &Proceso) -> bool {
    proceso.cmdline.split_whitespace().any(|parte| parte == "aviso-gobernanza")
}

fn es_agente(proceso: &Proceso, pid_agente: u32) -> bool {
    if proceso.pid == pid_agente {
        return true;
    }
    let base = base_de(&proceso.exe).to_ascii_lowercase();
    let comm = proceso.comm.to_ascii_lowercase();
    base == "gobernanza-agente" || base == "gobernanza-agente.exe" || base.starts_with("gobernanza-agen")
        || comm == "gobernanza-agente" || comm == "gobernanza-agente.exe" || comm.starts_with("gobernanza-agen")
        || base == "aicontrol-daemon" || base == "gobernanza-daemon" || base == "aicontrol"
        || comm == "aicontrol-daemon" || comm == "gobernanza-daemon"
}

fn procesos_generados(raiz: &Proceso, procesos: &[Proceso]) -> Vec<Proceso> {
    let ids = descendientes(raiz.pid, procesos);
    if ids.is_empty() {
        return vec![raiz.clone()];
    }
    let por_pid: HashMap<u32, &Proceso> = procesos.iter().map(|proceso| (proceso.pid, proceso)).collect();
    ids.into_iter().filter_map(|id| por_pid.get(&id).map(|proceso| (*proceso).clone())).collect()
}

fn descendientes(raiz: u32, procesos: &[Proceso]) -> Vec<u32> {
    let mut por_padre: HashMap<u32, Vec<u32>> = HashMap::new();
    for proceso in procesos {
        por_padre.entry(proceso.ppid).or_default().push(proceso.pid);
    }
    let mut salida = Vec::new();
    let mut cola = por_padre.get(&raiz).cloned().unwrap_or_default();
    let mut visto = HashMap::from([(raiz, true)]);
    while let Some(id) = cola.first().copied() {
        cola.remove(0);
        if visto.contains_key(&id) {
            continue;
        }
        visto.insert(id, true);
        salida.push(id);
        if let Some(hijos) = por_padre.get(&id) {
            cola.extend(hijos);
        }
    }
    salida.truncate(64);
    salida
}

fn base_de(ruta: &str) -> String {
    ruta.rsplit(['/', '\\']).next().unwrap_or(ruta).to_string()
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn programa(id: &str, binario: &str, accion: &str) -> Programa {
        Programa { id: id.into(), nombre: id.into(), binario: binario.into(), accion: accion.into() }
    }

    #[test]
    fn cierra_los_procesos_generados_y_luego_los_sigue_por_id() {
        let ahora = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000);
        let procesos = vec![
            Proceso { pid: 10, ppid: 1, comm: "hermes.exe".into(), exe: r"C:\hermes.exe".into(), cmdline: String::new() },
            Proceso { pid: 11, ppid: 10, comm: "helper.exe".into(), exe: r"C:\helper.exe".into(), cmdline: String::new() },
            Proceso { pid: 12, ppid: 11, comm: "gpu.exe".into(), exe: r"C:\gpu.exe".into(), cmdline: String::new() },
            Proceso { pid: 20, ppid: 1, comm: "firefox.exe".into(), exe: r"C:\firefox.exe".into(), cmdline: String::new() },
        ];
        let programas = vec![
            programa("hermes", "hermes.exe", "bloquear"),
            programa("firefox", "firefox.exe", "permitir"),
            programa("agente", "gobernanza-agente.exe", "bloquear"),
        ];
        let mut vigilancia = Vigilancia::default();
        let actos = vigilancia.revisar(&programas, &procesos, ahora, 99);
        assert_eq!(actos.len(), 1);
        assert!(!actos[0].cerrar);
        assert_eq!(actos[0].programa.id, "firefox");
        let despues = ahora + ESPERA_CIERRE;
        let actos = vigilancia.revisar(&programas, &procesos, despues, 99);
        assert_eq!(actos.len(), 2);
        assert!(actos[0].cerrar && actos[0].avisar && actos[0].proceso.pid == 11);
        assert!(actos[1].cerrar && !actos[1].avisar && actos[1].proceso.pid == 12);
        let otra = vigilancia.revisar(&programas, &procesos, despues + Duration::from_secs(1), 99);
        assert_eq!(otra.len(), 2);
        assert!(otra.iter().all(|acto| acto.cerrar && !acto.avisar));
    }

    #[test]
    fn un_python_suelto_no_es_el_binario() {
        let proceso = Proceso {
            pid: 30,
            ppid: 1,
            comm: "python.exe".into(),
            exe: r"C:\Python\python.exe".into(),
            cmdline: r"C:\Python\python.exe".into(),
        };
        let mut vigilancia = Vigilancia::default();
        let actos = vigilancia.revisar(
            &[programa("hermes", "hermes.exe", "bloquear")],
            &[proceso],
            SystemTime::UNIX_EPOCH,
            1,
        );
        assert!(actos.is_empty());
    }

    #[test]
    fn el_proceso_generado_se_reconoce_al_reaparecer() {
        let ahora = SystemTime::UNIX_EPOCH + Duration::from_secs(50);
        let abierto = Proceso {
            pid: 41,
            ppid: 1,
            comm: "hermes.exe".into(),
            exe: r"C:\Python\python.exe".into(),
            cmdline: r"C:\Python\python.exe -m hermes_cli.main gateway run".into(),
        };
        assert_eq!(ruta_del_programa(&abierto), abierto.cmdline);
        let mut vigilancia = Vigilancia::default();
        let programas = vec![programa("hermes", "hermes.exe", "bloquear")];
        assert!(vigilancia.revisar(&programas, &[abierto.clone()], ahora, 1).is_empty());
        let actos = vigilancia.revisar(&programas, &[abierto.clone()], ahora + ESPERA_CIERRE, 1);
        assert_eq!(actos.len(), 1);
        assert!(actos[0].cerrar && actos[0].avisar);
        let mut reaparecio = abierto;
        reaparecio.pid = 77;
        let otra = vigilancia.revisar(&programas, &[reaparecio], ahora + ESPERA_CIERRE, 1);
        assert_eq!(otra.len(), 1);
        assert!(otra[0].cerrar && !otra[0].avisar && otra[0].proceso.pid == 77);
    }

    #[test]
    fn no_elige_al_propio_agente() {
        let proceso = Proceso {
            pid: 7,
            ppid: 1,
            comm: "hermes.exe".into(),
            exe: r"C:\hermes.exe".into(),
            cmdline: String::new(),
        };
        let mut vigilancia = Vigilancia::default();
        let actos = vigilancia.revisar(&[programa("hermes", "hermes.exe", "bloquear")], &[proceso], SystemTime::UNIX_EPOCH, 7);
        assert!(actos.is_empty());
    }
}
