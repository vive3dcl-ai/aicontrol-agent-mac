use std::fs;
use std::path::{Path, PathBuf};

use crate::baja;
use crate::ca;
use crate::contrato;
use crate::estado;
use crate::navegador;
use crate::red;
use crate::sistema;

#[derive(Clone)]
pub struct Rutas {
    pub url: String,
    pub token: PathBuf,
    pub estado: PathBuf,
    pub tenant: String,
    pub clave: String,
}

impl Rutas {
    pub fn dir(&self) -> PathBuf {
        self.estado.parent().unwrap_or(Path::new(".")).to_path_buf()
    }
}

fn enviar(url: &str, token: &str, cuerpo: &serde_json::Value) -> Result<serde_json::Value, String> {
    publicar(url, token, cuerpo)
}

pub fn publicar(url: &str, token: &str, cuerpo: &serde_json::Value) -> Result<serde_json::Value, String> {
    let clave = crate::configuracion::clave_definitiva();
    let auth = if token.trim().is_empty() { None } else { Some(token.trim()) };
    match red::post_agente(url, auth, &clave, cuerpo) {
        Ok(respuesta) => {
            absorber_renovacion(&clave, &respuesta);
            Ok(respuesta)
        }
        Err(error) if token_rechazado(&error) && !clave.is_empty() && !url.contains("/dispositivos/renovar") => {
            let (nuevo, clave_nueva) = renovar_contra(url, &clave)?;
            let respuesta = red::post_agente(url, Some(nuevo.trim()), &clave_nueva, cuerpo)?;
            absorber_renovacion(&clave_nueva, &respuesta);
            Ok(respuesta)
        }
        Err(error) => Err(error),
    }
}

pub fn renovar_sesion(rutas: &Rutas) -> Result<(), String> {
    let estado = estado::leer(&rutas.estado);
    let url = estado::url_servidor(&rutas.url, &estado)?;
    let clave = crate::configuracion::clave_definitiva();
    if clave.is_empty() {
        return Err("no hay clave para renovar el token".into());
    }
    renovar_contra(&url, &clave)?;
    Ok(())
}

fn renovar_contra(url: &str, clave: &str) -> Result<(String, String), String> {
    let machine_id = crate::sistema::inventario_base()
        .get("machineId")
        .and_then(|valor| valor.as_str())
        .unwrap_or("")
        .to_string();
    if machine_id.len() < 8 {
        return Err("falta la identidad de la máquina".into());
    }
    let cerrado = crate::sobre::cerrar(clave, &serde_json::json!({ "contrato": 1, "machineId": machine_id }))?;
    let sobre = cerrado.get("sobre").and_then(|valor| valor.as_str()).unwrap_or("");
    let raiz = url.split_once("/v1/").map(|(base, _)| base).unwrap_or(url.trim_end_matches('/'));
    let respuesta = red::post_json(
        &red::unir(raiz, "/v1/dispositivos/renovar"),
        None,
        &serde_json::json!({ "machineId": machine_id, "sobre": sobre }),
    )?;
    let cred = credenciales_de(clave, &respuesta).ok_or("el servidor no renovó el token")?;
    crate::configuracion::guardar_credenciales(&cred.0, &cred.1);
    Ok(cred)
}

fn absorber_renovacion(clave: &str, respuesta: &serde_json::Value) {
    if let Some(cred) = credenciales_de(clave, respuesta) {
        crate::configuracion::guardar_credenciales(&cred.0, &cred.1);
    }
}

fn credenciales_de(clave: &str, respuesta: &serde_json::Value) -> Option<(String, String)> {
    let sobre = respuesta.get("renovacion").and_then(|valor| valor.as_str())?;
    let plano = crate::sobre::abrir(clave, sobre).ok()?;
    let token = plano.get("token").and_then(|valor| valor.as_str()).unwrap_or("").trim().to_string();
    let clave_nueva = plano.get("claveDefinitiva").and_then(|valor| valor.as_str()).unwrap_or("").trim().to_string();
    if token.starts_with("gdv_") && clave_nueva.starts_with("gdv_") {
        Some((token, clave_nueva))
    } else {
        None
    }
}

fn token_rechazado(error: &str) -> bool {
    error.starts_with("HTTP 401")
}

fn recordar_clave(respuesta: &serde_json::Value) {
    let Some(clave) = respuesta.get("claveDefinitiva").and_then(|valor| valor.as_str()) else { return };
    if clave.starts_with("gdv_") {
        crate::configuracion::guardar_clave_definitiva(clave);
    }
}

pub fn asegurar_token(rutas: &Rutas, clave: &str) -> Result<(), String> {
    if fs::read_to_string(&rutas.token).map(|texto| !texto.trim().is_empty()).unwrap_or(false) {
        return Ok(());
    }
    if rutas.tenant.trim().len() < 2 {
        return Err("Falta la organización del equipo. Vuelve a elegir el servidor.".into());
    }
    let clave = clave.trim();
    if clave.len() < 8 {
        return Err("Escribe la clave de enrolamiento.".into());
    }
    let mut altas = rutas.clone();
    altas.clave = clave.to_string();
    enrolar(&altas)
}

fn sincronizar(rutas: &Rutas) {
    let estado = estado::leer(&rutas.estado);
    let url = estado::url_servidor(&rutas.url, &estado).unwrap_or_else(|_| rutas.url.clone());
    crate::configuracion::sincronizar(&url, &rutas.token, &rutas.estado, "", &rutas.tenant);
}

pub fn enrolar(rutas: &Rutas) -> Result<(), String> {
    let mut estado = estado::leer(&rutas.estado);
    let url = estado::url_servidor(&rutas.url, &estado)?;
    let mut cuerpo = sistema::inventario_base();
    cuerpo["contrato"] = serde_json::json!(1);
    cuerpo["tenantSlug"] = serde_json::Value::String(rutas.tenant.clone());
    if !rutas.clave.is_empty() {
        cuerpo["clave"] = serde_json::Value::String(rutas.clave.clone());
    }
    contrato::validar_contrato("enrolamiento", &cuerpo)?;
    let respuesta = red::post_json(&red::unir(&url, "/v1/dispositivos/enrolar"), None, &cuerpo)?;
    let token = respuesta.get("token").and_then(|valor| valor.as_str()).ok_or("el API no devolvió token")?;
    if let Some(dir) = rutas.token.parent() {
        fs::create_dir_all(dir).map_err(|error| error.to_string())?;
    }
    fs::write(&rutas.token, token).map_err(|error| error.to_string())?;
    recordar_clave(&respuesta);
    if estado.host_modo != "personalizado" {
        estado.host_modo = "global".into();
    }
    if let Some(clave) = respuesta.get("claveAutorizacion").and_then(|valor| valor.as_str()) {
        estado.clave_autorizacion = clave.to_string();
    }
    estado::aplicar_respuesta(&mut estado, &respuesta);
    estado::guardar(&rutas.estado, &estado)?;
    sincronizar(rutas);
    Ok(())
}

pub fn latido(rutas: &Rutas) -> Result<(), String> {
    let estado_previo = estado::leer(&rutas.estado);
    let url = estado::url_servidor(&rutas.url, &estado_previo)?;
    let token = fs::read_to_string(&rutas.token).map_err(|error| format!("no se pudo leer el token: {error}"))?;
    let mut cuerpo = sistema::inventario_base();
    cuerpo["contrato"] = serde_json::json!(1);
    cuerpo["estadoExtension"] = serde_json::json!("no_instalada");
    let ca_vigente = sistema::certificado_instalado();
    cuerpo["estadoCa"] = serde_json::json!(if ca_vigente { "vigente" } else { "no_instalada" });
    cuerpo.as_object_mut().map(|mapa| mapa.remove("sistema"));
    cuerpo["renuevaTokens"] = serde_json::json!(true);
    crate::actualizacion::anotar_latido(&mut cuerpo, &rutas.dir());
    contrato::validar_contrato("latido", &cuerpo)?;
    let respuesta = enviar(&red::unir(&url, "/v1/dispositivos/latido"), token.trim(), &cuerpo)?;
    let token = fs::read_to_string(&rutas.token).unwrap_or(token);
    recordar_clave(&respuesta);
    let mut estado = estado::leer(&rutas.estado);
    estado::aplicar_respuesta(&mut estado, &respuesta);
    estado.certificado = if ca_vigente { "instalado" } else { "pendiente" }.into();
    estado::guardar(&rutas.estado, &estado)?;
    if let Err(error) = crate::actualizacion::preparar(&rutas.dir(), &url, token.trim(), &respuesta) {
        eprintln!("actualización: {error}");
    }
    let _ = crate::actualizacion::aplicar_en_caliente(&rutas.dir());
    sincronizar(rutas);
    println!("latido ok");
    Ok(())
}

pub fn activar(rutas: &Rutas, correo: &str, clave: &str) -> Result<(), String> {
    let estado = estado::leer(&rutas.estado);
    let url = estado::url_servidor(&rutas.url, &estado)?;
    let mut cuerpo = sistema::inventario_base();
    if cuerpo.get("machineId").and_then(|valor| valor.as_str()).unwrap_or("").is_empty() {
        return Err("falta la identidad de la máquina".into());
    }
    cuerpo["correo"] = serde_json::Value::String(correo.trim().to_ascii_lowercase());
    cuerpo["clave"] = serde_json::Value::String(clave.to_string());
    let token = fs::read_to_string(&rutas.token).unwrap_or_default();
    let token = token.trim();
    let respuesta = if token.is_empty() {
        red::post_json(&red::unir(&url, "/v1/dispositivos/activar"), None, &cuerpo)
    } else {
        enviar(&red::unir(&url, "/v1/dispositivos/activar"), token, &cuerpo)
    }
    .map_err(mensaje_enlace)?;
    recordar_clave(&respuesta);
    if let Some(nuevo) = respuesta.get("token").and_then(|valor| valor.as_str()) {
        if !nuevo.is_empty() {
            fs::write(&rutas.token, nuevo).map_err(|error| error.to_string())?;
        }
    }
    let mut estado = estado::leer(&rutas.estado);
    estado.enlazado = true;
    estado.correo = correo.trim().to_string();
    estado::aplicar_respuesta(&mut estado, &respuesta);
    estado.enlazado = true;
    estado::guardar(&rutas.estado, &estado)?;
    sincronizar(rutas);
    Ok(())
}

fn mensaje_enlace(error: String) -> String {
    let Some(inicio) = error.find('{') else { return "no se pudo enlazar la cuenta".into() };
    let Ok(cuerpo) = serde_json::from_str::<serde_json::Value>(&error[inicio..]) else { return "no se pudo enlazar la cuenta".into() };
    cuerpo.get("error").and_then(|valor| valor.as_str()).unwrap_or("no se pudo enlazar la cuenta").to_string()
}

fn normalizar_clave_config(clave: &str) -> String {
    clave
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
        .collect::<String>()
        .to_ascii_uppercase()
}

pub fn autorizar(rutas: &Rutas, clave: &str) -> Result<(), String> {
    let clave = normalizar_clave_config(clave);
    if clave.is_empty() {
        return Err("Escribe la clave de Config.".into());
    }
    let estado = estado::leer(&rutas.estado);
    let guardada = normalizar_clave_config(&estado.clave_autorizacion);
    let token = fs::read_to_string(&rutas.token).unwrap_or_default();
    let remoto = estado::url_servidor(&rutas.url, &estado).ok().and_then(|url| {
        if token.trim().is_empty() {
            return None;
        }
        match enviar(
            &red::unir(&url, "/v1/dispositivos/ajustes/autorizar-local"),
            token.trim(),
            &serde_json::json!({ "clave": clave }),
        ) {
            Ok(_) => Some(Ok::<(), String>(())),
            Err(error) if error.starts_with("HTTP 401") => Some(Err("Clave de Config incorrecta".into())),
            Err(error) if error.starts_with("HTTP ") => {
                Some(Err("No se pudo comprobar la clave con el servidor.".into()))
            }
            Err(_) => None,
        }
    });
    if let Some(resultado) = remoto {
        resultado?;
    } else if guardada.is_empty() || !iguales(clave.as_bytes(), guardada.as_bytes()) {
        return Err("Clave de Config incorrecta".into());
    }
    let mut estado = estado::leer(&rutas.estado);
    estado.ajustes_autorizados = true;
    estado.ajustes_hasta = estado::dentro_de_horas(12);
    estado.clave_autorizacion = clave;
    // La bandeja corre como usuario y ProgramData puede ser de solo lectura;
    // si no puede guardar, la sesión de ajustes igual queda abierta en memoria.
    let _ = estado::guardar(&rutas.estado, &estado);
    Ok(())
}

fn iguales(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() { return false; }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b) { diff |= x ^ y; }
    diff == 0
}

pub fn bloquear_ajustes(ruta: &Path) {
    let mut estado = estado::leer(ruta);
    if estado.ajustes_hasta.is_empty() { return; }
    estado.ajustes_hasta.clear();
    estado.ajustes_autorizados = false;
    let _ = estado::guardar(ruta, &estado);
}

pub fn elegir_endpoint(rutas: &Rutas, modo: &str, host: &str) -> Result<(), String> {
    let (modo, host) = if modo.trim() == "personalizado" {
        let host = estado::normalizar_endpoint(host)?;
        let _ = fs::remove_file(&rutas.token);
        ("personalizado".to_string(), host)
    } else {
        ("global".to_string(), String::new())
    };
    let mut estado = estado::leer(&rutas.estado);
    estado.endpoint_elegido = true;
    estado.host_modo = modo;
    estado.host_personalizado = host;
    if estado.host_modo == "personalizado" {
        estado.direccion_global.clear();
    } else if estado.direccion_global.trim().is_empty() || crate::ui::es_loopback(&estado.direccion_global) {
        estado.direccion_global = crate::ui::url_visible(&estado.direccion_global, &rutas.url);
    }
    estado.conexion = "conectando".into();
    estado::guardar(&rutas.estado, &estado)?;
    sincronizar(rutas);
    Ok(())
}

pub fn instalar_certificado(rutas: &Rutas, cerrar_navegadores: bool) -> Result<Vec<String>, String> {
    let mut pasos = vec!["Generando el certificado del equipo".to_string()];
    let dir = rutas.dir().join("ca");
    let autoridad = ca::cargar_o_crear(&dir)?;
    if cerrar_navegadores {
        pasos.push("Cerrando los navegadores que lo usan".into());
        sistema::cerrar_navegadores();
    }
    pasos.push("Instalando el certificado en el equipo".into());
    sistema::instalar_ca(&dir.join("gobernanza-ca.crt"))?;
    if !sistema::certificado_instalado() {
        anotar_certificado(&rutas.estado, false);
        return Err("el certificado no quedó en el almacén del equipo".into());
    }
    aplicar_filtro(rutas)?;
    if !filtro_vigente() {
        anotar_certificado(&rutas.estado, false);
        return Err("el filtro no quedó en los navegadores".into());
    }
    sistema::cerrar_navegadores();
    pasos.push("Filtro aplicado en Chrome, Edge, Brave, Opera, Firefox y el resto de navegadores Chromium".into());
    anotar_certificado(&rutas.estado, true);
    let _ = autoridad;
    pasos.push("Certificado instalado".into());
    Ok(pasos)
}

pub fn aplicar_filtro(rutas: &Rutas) -> Result<(), String> {
    let _ = rutas;
    sistema::aplicar_politica_navegadores(navegador::PAC)?;
    sistema::escribir_pac(navegador::PAC)?;
    for raiz in sistema::raices_firefox() {
        navegador::asegurar_familia_firefox(&raiz);
    }
    Ok(())
}

pub fn vigilar_navegadores(procesos: &[crate::programas::Proceso]) {
    use std::collections::HashMap;
    if crate::baja::salida_pedida().is_some() || !sistema::locales_escuchando() {
        return;
    }
    let mut vistos: HashMap<navegador::DestinoFiltro, String> = HashMap::new();
    for proceso in procesos {
        let Some(destino) = navegador::destino_de(&proceso.exe) else { continue };
        vistos.entry(destino).or_insert_with(|| proceso.exe.clone());
    }
    for (destino, exe) in vistos {
        if !filtro_de(destino) {
            if aplicar_destino(destino).is_err() || !filtro_de(destino) {
                continue;
            }
            marcar_reapertura(destino, &exe);
        }
    }
    for (destino, exe) in tomar_reaperturas() {
        if sistema::reabrir_navegador(&exe, &navegador::argumentos_reapertura(destino)).is_ok() {
            olvidar_reapertura(destino);
        }
    }
}

fn reaberturas() -> &'static std::sync::Mutex<std::collections::HashMap<navegador::DestinoFiltro, String>> {
    static REAPERTURAS: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<navegador::DestinoFiltro, String>>> = std::sync::OnceLock::new();
    REAPERTURAS.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
}

fn marcar_reapertura(destino: navegador::DestinoFiltro, exe: &str) {
    if exe.is_empty() {
        return;
    }
    if let Ok(mut mapa) = reaberturas().lock() {
        mapa.entry(destino).or_insert_with(|| exe.to_string());
    }
}

fn tomar_reaperturas() -> Vec<(navegador::DestinoFiltro, String)> {
    reaberturas().lock().map(|mapa| mapa.iter().map(|(destino, exe)| (*destino, exe.clone())).collect()).unwrap_or_default()
}

fn olvidar_reapertura(destino: navegador::DestinoFiltro) {
    if let Ok(mut mapa) = reaberturas().lock() {
        mapa.remove(&destino);
    }
}

fn filtro_de(destino: navegador::DestinoFiltro) -> bool {
    match destino {
        navegador::DestinoFiltro::Chromium(clave) => {
            sistema::filtro_chromium_listo(clave, navegador::PAC) && (sistema::es_servicio() || sistema::leer_pac().trim() == navegador::PAC)
        }
        navegador::DestinoFiltro::Firefox(clave) => sistema::filtro_firefox_listo(clave, navegador::PAC),
    }
}

fn aplicar_destino(destino: navegador::DestinoFiltro) -> Result<(), String> {
    sistema::escribir_pac(navegador::PAC)?;
    match destino {
        navegador::DestinoFiltro::Chromium(clave) => sistema::aplicar_chromium(clave, navegador::PAC),
        navegador::DestinoFiltro::Firefox(clave) => {
            sistema::aplicar_firefox_clave(clave, navegador::PAC)?;
            for raiz in sistema::raices_firefox() {
                navegador::asegurar_familia_firefox(&raiz);
            }
            Ok(())
        }
    }
}

pub fn sincronizar_entorno(rutas: &Rutas) {
    if crate::baja::salida_pedida().is_some() {
        return;
    }
    if sistema::es_servicio() {
        let dir_ca = rutas.dir().join("ca");
        let crt = dir_ca.join("gobernanza-ca.crt");
        if crt.is_file() && (ca::requiere_instalacion(&dir_ca) || !sistema::certificado_instalado()) {
            if let Err(error) = sistema::instalar_ca(&crt) {
                let _ = std::fs::write(rutas.dir().join("filtro-error.txt"), &error);
            } else {
                ca::marcar_instalada(&dir_ca);
            }
        }
    }
    let presente = sistema::certificado_instalado();
    anotar_certificado(&rutas.estado, presente);
    if presente {
        if !sistema::locales_escuchando() {
            return;
        }
        let antes = filtro_vigente();
        if !antes {
            if let Err(error) = aplicar_filtro(rutas) {
                let _ = std::fs::write(rutas.dir().join("filtro-error.txt"), &error);
                return;
            }
        }
        if filtro_vigente() {
            let _ = std::fs::remove_file(rutas.dir().join("filtro-error.txt"));
        }
        return;
    }
    if filtro_presente() || sistema::leer_pac().trim() == navegador::PAC {
        restaurar_filtro(rutas);
    }
}

fn anotar_certificado(ruta: &Path, presente: bool) {
    let mut estado = estado::leer(ruta);
    let texto = if presente { "instalado" } else { "pendiente" };
    if estado.certificado != texto {
        estado.certificado = texto.into();
        let _ = estado::guardar(ruta, &estado);
    }
}

fn filtro_vigente() -> bool {
    if !sistema::politicas_navegador_vigentes(navegador::PAC) {
        return false;
    }
    if sistema::leer_pac().trim() != navegador::PAC {
        return false;
    }
    sistema::raices_firefox().iter().all(|raiz| navegador::familia_firefox_vigente(raiz))
}

fn filtro_presente() -> bool {
    if sistema::leer_pac().trim() == navegador::PAC || sistema::politicas_navegador_presentes(navegador::PAC) {
        return true;
    }
    sistema::raices_firefox().iter().any(|raiz| navegador::raices_firefox(raiz).iter().any(|item| navegador::firefox_tiene_marca(item)))
}

pub fn restaurar_filtro(rutas: &Rutas) {
    let marca = rutas.dir().join("proxy-anterior.txt");
    if let Ok(anterior) = fs::read_to_string(&marca) {
        if anterior.trim().is_empty() {
            let _ = sistema::quitar_pac();
        } else {
            let _ = sistema::escribir_pac(anterior.trim());
        }
        let _ = fs::remove_file(&marca);
    } else {
        let _ = sistema::quitar_pac();
    }
    sistema::quitar_politica_navegadores();
    for raiz in sistema::raices_firefox() {
        navegador::quitar_familia_firefox(&raiz);
    }
}

pub fn avisar_desinstalacion(rutas: &Rutas) -> Result<(), String> {
    let mut ultimo = String::from("no se pudo avisar al panel");
    for intento in 0..3 {
        match avisar_una_vez(rutas) {
            Ok(()) => return Ok(()),
            Err(error) => ultimo = error,
        }
        if intento < 2 {
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
    }
    Err(ultimo)
}

fn avisar_una_vez(rutas: &Rutas) -> Result<(), String> {
    let estado = estado::leer(&rutas.estado);
    let url = estado::url_servidor(&rutas.url, &estado)?;
    let token = fs::read_to_string(&rutas.token).map_err(|_| "el equipo todavía no está conectado".to_string())?;
    enviar(&red::unir(&url, "/v1/dispositivos/desinstalar"), token.trim(), &serde_json::json!({}))?;
    Ok(())
}

pub fn autorizar_baja(rutas: &Rutas, clave: &str) -> Result<(), String> {
    if clave.trim().is_empty() {
        return Err("Hace falta la clave de Config.".into());
    }
    autorizar(rutas, clave)
}

pub fn paso_navegadores(rutas: &Rutas) -> Result<String, String> {
    sistema::detener_y_quitar_servicio();
    sistema::terminar_otros_agentes();
    std::thread::sleep(std::time::Duration::from_millis(400));
    sistema::cerrar_navegadores();
    std::thread::sleep(std::time::Duration::from_millis(600));
    sistema::cerrar_navegadores();
    restaurar_filtro(rutas);
    sistema::quitar_politica_navegadores();
    sistema::quitar_proxy_de_usuarios();
    quitar_firefox_de_usuarios();
    if sistema::agente_en_ejecucion() {
        return Err("el agente sigue en ejecución".into());
    }
    if sistema::filtro_gobernanza_activo() {
        return Err("los navegadores siguen usando el proxy de AIcontrol".into());
    }
    if sistema::navegadores_abiertos() {
        return Err("quedó un navegador abierto".into());
    }
    Ok("Verificado: navegadores cerrados, proxy quitado y agente detenido.".into())
}

pub fn paso_certificado() -> Result<String, String> {
    quitar_certificado_con_reintentos()?;
    if sistema::certificado_instalado() {
        return Err("el certificado sigue en el almacén del equipo".into());
    }
    Ok("Verificado: el certificado ya no está en el equipo.".into())
}

pub fn paso_agente(rutas: &Rutas) -> Result<String, String> {
    let aviso = match avisar_desinstalacion(rutas) {
        Ok(()) => "El panel quedó avisado de la desinstalación.".to_string(),
        Err(error) => format!("No se pudo avisar al panel: {error}."),
    };
    sistema::detener_y_quitar_servicio();
    sistema::terminar_otros_agentes();
    let _ = sistema::quitar_inicio();
    let _ = sistema::borrar_tarea();
    sistema::quitar_de_aplicaciones();
    sistema::quitar_producto_windows();
    olvidar_cuenta(&rutas.dir())?;
    sistema::borrar_carpeta_despues(&sistema::carpeta_programa());
    if sistema::certificado_instalado() {
        return Err("el certificado volvió a aparecer".into());
    }
    if sistema::agente_en_ejecucion() {
        return Err("el proceso del agente sigue activo".into());
    }
    Ok(format!("Verificado: se quitó la cuenta vinculada, el token y los datos del equipo. {aviso}"))
}

fn olvidar_cuenta(dir: &std::path::Path) -> Result<(), String> {
    sistema::detener_y_quitar_servicio();
    sistema::terminar_otros_agentes();
    for nombre in ["estado.json", "dispositivo.token", "config.dat", "config.key", "baja.autorizada"] {
        let _ = std::fs::remove_file(dir.join(nombre));
    }
    let _ = std::fs::remove_dir_all(dir.join("ca"));
    let _ = sistema::borrar_carpeta_ahora(dir);
    olvidar_copias_de_usuario();
    if dir.join("estado.json").exists() || dir.join("dispositivo.token").exists() || dir.join("config.dat").exists() {
        return Err(format!("quedó la cuenta vinculada en {}", dir.display()));
    }
    let _ = sistema::borrar_carpeta_ahora(dir);
    Ok(())
}

fn olvidar_copias_de_usuario() {
    for raiz in sistema::raices_firefox() {
        if let Some(usuario) = raiz.parent() {
            let _ = std::fs::remove_dir_all(usuario.join("Gobernanza"));
        }
    }
}

pub fn preparar_certificado_equipo(rutas: &Rutas) -> Result<String, String> {
    let dir = rutas.dir().join("ca");
    let _ = ca::cargar_o_crear(&dir)?;
    sistema::instalar_ca(&dir.join("gobernanza-ca.crt"))?;
    ca::marcar_instalada(&dir);
    if !sistema::certificado_instalado() {
        return Err("el certificado no quedó en el almacén del equipo".into());
    }
    anotar_certificado(&rutas.estado, true);
    Ok("Verificado: certificado instalado en el equipo.".into())
}

pub fn ejecutar_reversion(rutas: &Rutas, desde_msi: bool) -> Result<(), String> {
    if !crate::baja::baja_vigente_en(&rutas.dir()) {
        return Ok(());
    }
    paso_navegadores(rutas)?;
    paso_certificado()?;
    if desde_msi {
        let _ = avisar_desinstalacion(rutas);
        let _ = sistema::quitar_inicio();
        let _ = sistema::borrar_tarea();
        sistema::quitar_de_aplicaciones();
        crate::configuracion::borrar();
        olvidar_cuenta(&rutas.dir())?;
        return Ok(());
    }
    paso_agente(rutas)?;
    Ok(())
}

fn quitar_certificado_con_reintentos() -> Result<(), String> {
    let mut ultimo = String::new();
    for _ in 0..8 {
        match sistema::quitar_ca() {
            Ok(()) if !sistema::certificado_instalado() => return Ok(()),
            Ok(()) => ultimo = "el certificado sigue en el almacén del equipo".into(),
            Err(error) => ultimo = error,
        }
        std::thread::sleep(std::time::Duration::from_millis(250));
    }
    if sistema::certificado_instalado() {
        Err(ultimo)
    } else {
        Ok(())
    }
}

pub fn programar_desinstalacion(rutas: &Rutas) {
    let _ = rutas;
    sistema::frenar_reinicio_servicio();
    let _ = sistema::lanzar_desprendido(&["desinstalar-pasos".into()]);
    std::thread::spawn(|| {
        std::thread::sleep(std::time::Duration::from_millis(400));
        std::process::exit(0);
    });
}

fn quitar_firefox_de_usuarios() {
    for raiz in sistema::raices_firefox() {
        navegador::quitar_familia_firefox(&raiz);
    }
}

pub fn intentar_desinstalar(rutas: &Rutas, respuesta: i32) -> Result<(), String> {
    baja::validar_reto(respuesta)?;
    avisar_desinstalacion(rutas)?;
    baja::consumir_reto();
    let rutas = Rutas { url: rutas.url.clone(), token: rutas.token.clone(), estado: rutas.estado.clone(), tenant: rutas.tenant.clone(), clave: String::new() };
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(200));
        completar_desinstalacion(&rutas);
    });
    Ok(())
}

pub fn completar_desinstalacion(rutas: &Rutas) {
    let carpeta = rutas.dir();
    let _ = sistema::quitar_ca();
    restaurar_filtro(rutas);
    crate::configuracion::borrar();
    let _ = sistema::quitar_inicio();
    let _ = sistema::borrar_tarea();
    sistema::borrar_carpeta_despues(&carpeta);
    baja::pedir_salida(0);
}

pub fn cerrar_hasta_inicio(rutas: &Rutas) {
    restaurar_filtro(rutas);
    baja::marcar_pausa(&rutas.estado);
    baja::pedir_salida(0);
}

pub fn asegurar_copia(argumentos: &[String]) -> Result<(), String> {
    let _ = argumentos;
    Ok(())
}

pub fn registrar_persistencia(_rutas: &Rutas) -> Result<(), String> {
    if cfg!(windows) {
        let _ = sistema::quitar_inicio();
        let _ = sistema::borrar_tarea();
    }
    Ok(())
}
