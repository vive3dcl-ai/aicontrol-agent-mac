use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use crate::aviso;
use crate::baja;
use crate::ca;
use crate::configuracion;
use crate::contrato;
use crate::estado;
use crate::paquete;
use crate::programas;
use crate::proxy;
use crate::red;
use crate::servicio;
use crate::sesion;
use crate::sesion::Rutas;
use crate::sistema;
use crate::tarjeta;

pub fn ejecutar() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        args.push("servicio".into());
    }
    let comando = args.remove(0);
    let mut flags = match leer_flags(&args) {
        Ok(flags) => flags,
        Err(error) => salir(&error),
    };
    if comando != "empaquetar" {
        if let Err(error) = configuracion::aplicar(&mut flags) {
            salir(&error);
        }
    }
    let silencioso = matches!(comando.as_str(), "revertir" | "aplicar-local" | "desinstalar-pasos");
    let resultado = match comando.as_str() {
        "enrolar" => enrolar(&flags),
        "latido" => latido(&flags),
        "servicio" => servicio(&flags),
        "config" | "estado" => config(&flags),
        "aplicar-local" => aplicar_local(&flags),
        "empaquetar" => empaquetar(&flags),
        "aviso-instalacion" => {
            aviso::informar_instalacion();
            Ok(())
        }
        "desinstalar" => desinstalar(&flags),
        "desinstalar-pasos" => desinstalar_pasos(&flags),
        "revertir" => revertir(&flags),
        _ => Err(format!("comando desconocido: {comando}")),
    };
    if let Err(error) = resultado {
        if error == "cancelado" {
            std::process::exit(1);
        }
        if silencioso {
            eprintln!("error: {error}");
            std::process::exit(1);
        }
        salir(&error);
    }
}

fn salir(error: &str) -> ! {
    eprintln!("error: {error}");
    std::process::exit(1);
}

fn leer_flags(args: &[String]) -> Result<HashMap<String, String>, String> {
    let mut flags = HashMap::new();
    let mut i = 0;
    while i < args.len() {
        let clave = args[i].strip_prefix("--").ok_or_else(|| format!("argumento inesperado: {}", args[i]))?;
        let valor = args.get(i + 1).ok_or_else(|| format!("falta el valor de --{clave}"))?;
        flags.insert(clave.to_string(), valor.clone());
        i += 2;
    }
    Ok(flags)
}

fn rutas(flags: &HashMap<String, String>) -> Rutas {
    let carpeta = configuracion::carpeta_instalacion();
    Rutas {
        url: flags.get("url").cloned().unwrap_or_default(),
        token: PathBuf::from(flags.get("token-archivo").cloned().unwrap_or_else(|| carpeta.join("dispositivo.token").display().to_string())),
        estado: PathBuf::from(flags.get("estado").cloned().unwrap_or_else(|| carpeta.join("estado.json").display().to_string())),
        tenant: flags.get("tenant").cloned().unwrap_or_default(),
        clave: flags.get("clave").cloned().unwrap_or_default(),
    }
}

fn clonar(rutas: &Rutas) -> Rutas {
    Rutas { url: rutas.url.clone(), token: rutas.token.clone(), estado: rutas.estado.clone(), tenant: rutas.tenant.clone(), clave: rutas.clave.clone() }
}

fn enrolar(flags: &HashMap<String, String>) -> Result<(), String> {
    sesion::enrolar(&rutas(flags))
}

fn latido(flags: &HashMap<String, String>) -> Result<(), String> {
    sesion::latido(&rutas(flags))
}

fn config(flags: &HashMap<String, String>) -> Result<(), String> {
    let rutas = rutas(flags);
    let estado = estado::leer(&rutas.estado);
    println!("conexion {}", estado.conexion);
    println!("certificado {}", estado.certificado);
    println!("enlazado {}", estado.enlazado);
    Ok(())
}

fn servicio(flags: &HashMap<String, String>) -> Result<(), String> {
    let rutas = rutas(flags);
    let _ = crate::actualizacion::aplicar_en_caliente(&rutas.dir());
    if baja::arranque_bloqueado(false, baja::esta_desinstalado(&rutas.estado), baja::esta_pausado(&rutas.estado)) {
        return Ok(());
    }
    baja::quitar_marca(&rutas.estado);
    let segundos = flags.get("intervalo").and_then(|valor| valor.parse::<u64>().ok()).unwrap_or(60);
    if segundos < 5 {
        return Err("intervalo inválido".into());
    }
    sistema::permitir_datos_usuario(&rutas.dir());
    if !sistema::primera_instancia() {
        return Ok(());
    }
    sesion::bloquear_ajustes(&rutas.estado);
    let _ = sesion::registrar_persistencia(&rutas);
    let mut estado = estado::leer(&rutas.estado);
    if estado.host_modo != "personalizado" {
        estado.direccion_global = crate::ui::url_visible(&estado.direccion_global, &rutas.url);
    }
    let tiene_token = rutas.token.is_file();
    estado.conexion = if !estado.endpoint_elegido {
        "eligiendo".into()
    } else if estado.enlazado && tiene_token {
        "conectando".into()
    } else {
        "desconectado".into()
    };
    estado::guardar(&rutas.estado, &estado)?;
    println!("servicio de latido cada {segundos}s");
    let copia = clonar(&rutas);
    std::thread::spawn(move || tarjeta::servir(copia));
    let copia = clonar(&rutas);
    std::thread::spawn(move || {
        if let Ok(autoridad) = ca::cargar_o_crear(&copia.dir().join("ca")) {
            proxy::servir(copia.estado.clone(), copia.url.clone(), copia.token.display().to_string(), autoridad);
        }
    });
    std::thread::spawn(|| loop {
        servicio::asegurar_menu();
        std::thread::sleep(Duration::from_secs(3));
    });
    sesion::sincronizar_entorno(&rutas);
    let mut vigilancia = programas::Vigilancia::default();
    let mut fallos = 0u32;
    let mut proximo_latido = std::time::Instant::now();
    let mut proximo_entorno = std::time::Instant::now();
    let mut proximo_consumo = std::time::Instant::now();
    loop {
        if let Some(codigo) = baja::salida_pedida() {
            std::process::exit(codigo);
        }
        if std::time::Instant::now() >= proximo_latido {
            let estado = estado::leer(&rutas.estado);
            let mut espera = segundos;
            if estado.endpoint_elegido && estado.enlazado {
                if !rutas.token.is_file() {
                    if !configuracion::clave_definitiva().is_empty() {
                        if let Err(error) = sesion::renovar_sesion(&rutas) {
                            fallos += 1;
                            eprintln!("renovación falló: {error}");
                            estado::poner_conexion(&rutas.estado, "desconectado");
                            espera = segundos.saturating_mul(u64::from(fallos.min(4)).max(1)).min(120);
                        }
                    } else if rutas.clave.is_empty() {
                        estado::poner_conexion(&rutas.estado, "desconectado");
                    } else if let Err(error) = sesion::enrolar(&rutas) {
                        fallos += 1;
                        eprintln!("enrolamiento falló: {error}");
                        estado::poner_conexion(&rutas.estado, "desconectado");
                        espera = segundos.saturating_mul(u64::from(fallos.min(4)).max(1)).min(120);
                    }
                }
                if rutas.token.is_file() {
                    if let Err(error) = sesion::latido(&rutas) {
                        fallos += 1;
                        eprintln!("latido falló: {error}");
                        estado::poner_conexion(&rutas.estado, "desconectado");
                        eprintln!("fail-open: se siguen aplicando las políticas guardadas");
                        let servidor = error.starts_with("HTTP 5") || error.contains("no se pudo conectar") || error.contains("respuesta HTTP incompleta");
                        espera = if servidor { 10 } else { segundos.saturating_mul(u64::from(fallos.min(4)).max(1)).min(120) };
                    } else {
                        fallos = 0;
                    }
                }
            }
            proximo_latido = std::time::Instant::now() + Duration::from_secs(espera);
        }
        if std::time::Instant::now() >= proximo_entorno {
            sesion::sincronizar_entorno(&rutas);
            proximo_entorno = std::time::Instant::now() + Duration::from_secs(10);
        }
        vigilar(&rutas, &mut vigilancia);
        if std::time::Instant::now() >= proximo_consumo {
            publicar_consumo(&rutas);
            proximo_consumo = std::time::Instant::now() + Duration::from_secs(5);
        }
        dormir(Duration::from_secs(1));
    }
}

fn vigilar(rutas: &Rutas, vigilancia: &mut programas::Vigilancia) {
    let estado = estado::leer(&rutas.estado);
    let programas = estado::programas_vivos(&estado);
    let procesos = sistema::listar_procesos();
    if estado.certificado == "instalado" {
        sesion::vigilar_navegadores(&procesos);
    }
    let actos = vigilancia.revisar(&programas, &procesos, std::time::SystemTime::now(), std::process::id());
    for acto in actos {
        if acto.avisar {
            servicio::aviso(&acto.programa.nombre);
        }
        if acto.cerrar {
            let _ = sistema::cerrar_proceso(acto.proceso.pid);
        }
        if acto.publicar {
            publicar_programa(rutas, &acto);
        }
    }
}

fn publicar_consumo(rutas: &Rutas) {
    let estado = estado::leer(&rutas.estado);
    if !estado.enlazado || !rutas.token.is_file() {
        return;
    }
    let Ok(url) = estado::url_servidor(&rutas.url, &estado) else { return };
    let Ok(token) = std::fs::read_to_string(&rutas.token) else { return };
    let (cpu, ram) = sistema::consumo_propio();
    let cuerpo = serde_json::json!({ "contrato": 1, "cpuPorcentaje": cpu, "ramMb": ram });
    if contrato::validar_contrato("consumo", &cuerpo).is_err() {
        return;
    }
    let _ = sesion::publicar(&red::unir(&url, "/v1/ingesta/consumo"), token.trim(), &cuerpo);
}

fn publicar_programa(rutas: &Rutas, acto: &programas::Acto) {
    let Ok(url) = estado::url_servidor(&rutas.url, &estado::leer(&rutas.estado)) else { return };
    let Ok(token) = std::fs::read_to_string(&rutas.token) else { return };
    let cuerpo = serde_json::json!({
        "contrato": 1,
        "programaId": acto.programa.id,
        "pid": acto.proceso.pid,
        "hijos": acto.hijos,
        "ruta": programas::ruta_del_programa(&acto.proceso),
        "linea": acto.proceso.cmdline,
        "usuario": sistema::usuario(),
        "ocurridoEn": estado::ahora_rfc3339(),
    });
    if contrato::validar_contrato("programa", &cuerpo).is_err() {
        return;
    }
    let _ = sesion::publicar(&red::unir(&url, "/v1/ingesta/programas"), token.trim(), &cuerpo);
}

fn dormir(total: Duration) {
    let paso = Duration::from_millis(200);
    let mut pasado = Duration::ZERO;
    while pasado < total {
        if baja::salida_pedida().is_some() {
            return;
        }
        std::thread::sleep(paso);
        pasado += paso;
    }
}

fn aplicar_local(flags: &HashMap<String, String>) -> Result<(), String> {
    let dir = PathBuf::from(flags.get("dir").cloned().unwrap_or_default());
    if dir.as_os_str().is_empty() {
        return Err("falta la carpeta de la actualización".into());
    }
    sistema::reemplazar_desde(&dir)
}

fn empaquetar(flags: &HashMap<String, String>) -> Result<(), String> {
    let dir = PathBuf::from(flags.get("dir").cloned().unwrap_or_default());
    let salida = PathBuf::from(flags.get("salida").cloned().unwrap_or_else(|| "AIcontrol-actualizacion.zip".into()));
    if !dir.join("AIcontrol.app").is_dir() && dir.file_name().and_then(|nombre| nombre.to_str()) != Some("AIcontrol.app") {
        return Err("falta AIcontrol.app".into());
    }
    let raiz = if dir.file_name().and_then(|nombre| nombre.to_str()) == Some("AIcontrol.app") { dir.parent().unwrap_or(std::path::Path::new(".")).to_path_buf() } else { dir };
    let mut archivos = Vec::new();
    recorrer(&raiz.join("AIcontrol.app"), &raiz, &mut archivos)?;
    let bytes = paquete::empaquetar(&archivos)?;
    if let Some(padre) = salida.parent() {
        if !padre.as_os_str().is_empty() {
            std::fs::create_dir_all(padre).map_err(|error| error.to_string())?;
        }
    }
    std::fs::write(&salida, bytes).map_err(|error| error.to_string())?;
    println!("{}", salida.display());
    Ok(())
}

fn recorrer(dir: &std::path::Path, raiz: &std::path::Path, salida: &mut Vec<(String, Vec<u8>)>) -> Result<(), String> {
    for entrada in std::fs::read_dir(dir).map_err(|error| error.to_string())? {
        let entrada = entrada.map_err(|error| error.to_string())?;
        let ruta = entrada.path();
        if ruta.is_dir() {
            recorrer(&ruta, raiz, salida)?;
        } else {
            let relativo = ruta.strip_prefix(raiz).map_err(|error| error.to_string())?;
            let nombre = relativo.to_string_lossy().replace('\\', "/");
            let datos = std::fs::read(&ruta).map_err(|error| error.to_string())?;
            salida.push((nombre, datos));
        }
    }
    Ok(())
}

fn desinstalar(flags: &HashMap<String, String>) -> Result<(), String> {
    aviso::advertir_desinstalacion()?;
    let clave = aviso::pedir_clave_ajustes()?;
    let rutas = rutas(flags);
    sesion::autorizar_baja(&rutas, &clave)?;
    baja::marcar_baja(&rutas.dir())?;
    sistema::frenar_reinicio_servicio();
    sistema::lanzar_desprendido(&["desinstalar-pasos".into()])?;
    std::process::exit(0);
}

fn desinstalar_pasos(flags: &HashMap<String, String>) -> Result<(), String> {
    std::thread::sleep(Duration::from_secs(1));
    let rutas = rutas(flags);
    if !baja::baja_vigente_en(&rutas.dir()) {
        return Ok(());
    }
    sesion::paso_navegadores(&rutas)?;
    sesion::paso_certificado()?;
    sesion::paso_agente(&rutas)?;
    aviso::informar_desinstalacion();
    Ok(())
}

fn revertir(flags: &HashMap<String, String>) -> Result<(), String> {
    sesion::ejecutar_reversion(&rutas(flags), false)
}
