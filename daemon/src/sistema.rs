use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::configuracion;
use crate::navegador;
use crate::programas::Proceso;

pub const ETIQUETA_DAEMON: &str = "local.itsec.aicontrol";
pub const ETIQUETA_MENU: &str = "local.itsec.aicontrol.menu";
pub const PAC: &str = navegador::PAC;

const CHROMIUM: &[(&str, &str)] = &[
    (r"HKLM\Software\Policies\Google\Chrome", "com.google.Chrome"),
    (r"HKCU\Software\Policies\Google\Chrome", "com.google.Chrome"),
    (r"HKLM\Software\Policies\Microsoft\Edge", "com.microsoft.Edge"),
    (r"HKCU\Software\Policies\Microsoft\Edge", "com.microsoft.Edge"),
    (r"HKLM\Software\Policies\BraveSoftware\Brave", "com.brave.Browser"),
    (r"HKCU\Software\Policies\BraveSoftware\Brave", "com.brave.Browser"),
    (r"HKLM\Software\Policies\BraveSoftware\Brave-Beta", "com.brave.Browser.beta"),
    (r"HKCU\Software\Policies\BraveSoftware\Brave-Beta", "com.brave.Browser.beta"),
    (r"HKLM\Software\Policies\Opera", "com.operasoftware.Opera"),
    (r"HKCU\Software\Policies\Opera", "com.operasoftware.Opera"),
    (r"HKLM\Software\Policies\Opera GX", "com.operasoftware.OperaGX"),
    (r"HKCU\Software\Policies\Opera GX", "com.operasoftware.OperaGX"),
    (r"HKLM\Software\Policies\Vivaldi", "com.vivaldi.Vivaldi"),
    (r"HKCU\Software\Policies\Vivaldi", "com.vivaldi.Vivaldi"),
    (r"HKLM\Software\Policies\Chromium", "org.chromium.Chromium"),
    (r"HKCU\Software\Policies\Chromium", "org.chromium.Chromium"),
    (r"HKLM\Software\Policies\YandexBrowser", "ru.yandex.desktop.yandex-browser"),
    (r"HKCU\Software\Policies\YandexBrowser", "ru.yandex.desktop.yandex-browser"),
];

const FIREFOX: &[(&str, &str)] = &[
    (r"HKLM\Software\Policies\Mozilla\Firefox", "Firefox"),
    (r"HKCU\Software\Policies\Mozilla\Firefox", "Firefox"),
    (r"HKLM\Software\Policies\LibreWolf", "LibreWolf"),
    (r"HKCU\Software\Policies\LibreWolf", "LibreWolf"),
    (r"HKLM\Software\Policies\Waterfox", "Waterfox"),
    (r"HKCU\Software\Policies\Waterfox", "Waterfox"),
    (r"HKLM\Software\Policies\Floorp", "Floorp"),
    (r"HKCU\Software\Policies\Floorp", "Floorp"),
];

const APPS_NAVEGADOR: &[&str] = &[
    "Google Chrome",
    "Microsoft Edge",
    "Brave Browser",
    "Opera",
    "Vivaldi",
    "Chromium",
    "Firefox",
    "LibreWolf",
    "Waterfox",
    "Floorp",
    "Safari",
    "Yandex",
];

pub fn es_servicio() -> bool {
    true
}

pub fn carpeta_programa() -> PathBuf {
    raiz().join("Applications").join("AIcontrol.app")
}

pub fn inventario_base() -> serde_json::Value {
    let mut cuerpo = serde_json::json!({
        "hostname": hostname(),
        "sistema": sistema(),
        "versionSo": version_so(),
        "versionAgente": env!("CARGO_PKG_VERSION"),
        "cpu": cpu_texto(),
        "ramMb": ram_mb(),
        "usuarioSesion": usuario(),
    });
    if let Some(id) = machine_id() {
        cuerpo["machineId"] = serde_json::Value::String(id);
    }
    cuerpo
}

pub fn sistema() -> &'static str {
    if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else {
        "windows"
    }
}

pub fn usuario() -> String {
    if let Some(nombre) = usuario_consola() {
        if let Some(persona) = nombre_de_persona(&nombre) {
            return persona;
        }
    }
    "usuario".into()
}

fn usuario_consola() -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        let salida = Command::new("stat").args(["-f", "%Su", "/dev/console"]).output().ok()?;
        let nombre = String::from_utf8_lossy(&salida.stdout).trim().to_string();
        if nombre.is_empty() { None } else { Some(nombre) }
    }
    #[cfg(not(target_os = "macos"))]
    {
        if let Ok(nombre) = std::env::var("SUDO_USER") {
            if !nombre.trim().is_empty() && !nombre.eq_ignore_ascii_case("root") {
                return Some(nombre);
            }
        }
        std::env::var("USER").ok().filter(|nombre| !nombre.trim().is_empty())
    }
}

pub fn nombre_de_persona(nombre: &str) -> Option<String> {
    let limpio = nombre.trim();
    if limpio.is_empty() || limpio.eq_ignore_ascii_case("root") || limpio.eq_ignore_ascii_case("system") || limpio.ends_with('$') {
        return None;
    }
    Some(limpio.to_string())
}

pub fn hostname() -> String {
    let mut buf = [0u8; 256];
    extern "C" {
        fn gethostname(buf: *mut i8, len: usize) -> i32;
    }
    let ok = unsafe { gethostname(buf.as_mut_ptr() as *mut i8, buf.len() - 1) } == 0;
    if !ok {
        return "equipo".into();
    }
    let fin = buf.iter().position(|byte| *byte == 0).unwrap_or(buf.len());
    String::from_utf8_lossy(&buf[..fin]).trim().to_string()
}

pub fn cpu_texto() -> String {
    let n = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1);
    format!("{n} núcleos")
}

pub fn ram_mb() -> i64 {
    #[cfg(target_os = "linux")]
    {
        if let Ok(texto) = fs::read_to_string("/proc/meminfo") {
            for linea in texto.lines() {
                if let Some(resto) = linea.strip_prefix("MemTotal:") {
                    let kb: i64 = resto.split_whitespace().next().unwrap_or("0").parse().unwrap_or(0);
                    return (kb / 1024).max(1);
                }
            }
        }
    }
    #[cfg(target_os = "macos")]
    {
        if let Some(bytes) = sysctl_u64(c"hw.memsize") {
            return (bytes / (1024 * 1024)).max(1) as i64;
        }
    }
    1024
}

pub fn version_so() -> String {
    #[cfg(target_os = "macos")]
    {
        if let Ok(salida) = Command::new("sw_vers").arg("-productVersion").output() {
            let texto = String::from_utf8_lossy(&salida.stdout).trim().to_string();
            if !texto.is_empty() {
                return texto;
            }
        }
    }
    std::env::consts::OS.to_string()
}

pub fn machine_id() -> Option<String> {
    let crudo = machine_id_crudo()?;
    let limpio: String = crudo.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-').collect();
    if (8..=128).contains(&limpio.len()) {
        Some(limpio)
    } else {
        None
    }
}

fn machine_id_crudo() -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        let salida = Command::new("ioreg").args(["-rd1", "-c", "IOPlatformExpertDevice"]).output().ok()?;
        let texto = String::from_utf8_lossy(&salida.stdout);
        for linea in texto.lines() {
            if linea.contains("IOPlatformUUID") {
                return linea.split('"').nth(3).map(str::to_string);
            }
        }
        return None;
    }
    #[cfg(not(target_os = "macos"))]
    {
        fs::read_to_string("/etc/machine-id").ok().map(|texto| texto.trim().to_string()).filter(|texto| !texto.is_empty())
    }
}

pub fn id_arranque() -> u64 {
    boot_epoch().unwrap_or_else(|| SystemTime::now().duration_since(UNIX_EPOCH).map(|valor| valor.as_secs()).unwrap_or(0))
}

fn boot_epoch() -> Option<u64> {
    #[cfg(target_os = "linux")]
    {
        let texto = fs::read_to_string("/proc/stat").ok()?;
        for linea in texto.lines() {
            if let Some(resto) = linea.strip_prefix("btime ") {
                return resto.trim().parse().ok();
            }
        }
        None
    }
    #[cfg(target_os = "macos")]
    {
        sysctl_time(c"kern.boottime")
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    None
}

pub fn consumo_propio() -> (f64, u32) {
    let rss = rss_mb();
    let cpu = cpu_propio();
    (cpu, rss)
}

fn rss_mb() -> u32 {
    #[cfg(target_os = "linux")]
    {
        if let Ok(texto) = fs::read_to_string("/proc/self/status") {
            for linea in texto.lines() {
                if let Some(resto) = linea.strip_prefix("VmRSS:") {
                    let kb: u32 = resto.split_whitespace().next().unwrap_or("0").parse().unwrap_or(0);
                    return (kb / 1024).max(1);
                }
            }
        }
    }
    1
}

fn cpu_propio() -> f64 {
    #[cfg(target_os = "linux")]
    {
        let ahora = Instant::now();
        let ticks = ticks_propios();
        static PREVIO: std::sync::Mutex<Option<(Instant, u64)>> = std::sync::Mutex::new(None);
        let mut guardia = PREVIO.lock().unwrap_or_else(|error| error.into_inner());
        let porcentaje = if let Some((antes, ticks_antes)) = *guardia {
            let segundos = ahora.saturating_duration_since(antes).as_secs_f64();
            if segundos > 0.2 {
                let delta = ticks.saturating_sub(ticks_antes) as f64 / 100.0;
                (delta / segundos * 100.0).clamp(0.0, 100.0)
            } else {
                0.0
            }
        } else {
            0.0
        };
        *guardia = Some((ahora, ticks));
        return porcentaje;
    }
    #[cfg(not(target_os = "linux"))]
    0.0
}

#[cfg(target_os = "linux")]
fn ticks_propios() -> u64 {
    let Ok(texto) = fs::read_to_string("/proc/self/stat") else { return 0 };
    let Some(resto) = texto.rsplit_once(')') else { return 0 };
    let mut campos = resto.1.split_whitespace();
    let utime: u64 = campos.nth(11).unwrap_or("0").parse().unwrap_or(0);
    let stime: u64 = campos.next().unwrap_or("0").parse().unwrap_or(0);
    utime + stime
}

pub fn listar_procesos() -> Vec<Proceso> {
    #[cfg(target_os = "linux")]
    {
        return listar_proc();
    }
    #[cfg(target_os = "macos")]
    {
        return listar_ps();
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    Vec::new()
}

#[cfg(target_os = "linux")]
fn listar_proc() -> Vec<Proceso> {
    let mut salida = Vec::new();
    let Ok(entradas) = fs::read_dir("/proc") else { return salida };
    for entrada in entradas.flatten() {
        let Ok(pid) = entrada.file_name().to_string_lossy().parse::<u32>() else { continue };
        let base = entrada.path();
        let exe = fs::read_link(base.join("exe")).map(|ruta| ruta.display().to_string()).unwrap_or_default();
        let cmdline = fs::read(base.join("cmdline")).map(|bytes| bytes.split(|b| *b == 0).filter(|parte| !parte.is_empty()).map(|parte| String::from_utf8_lossy(parte).into_owned()).collect::<Vec<_>>().join(" ")).unwrap_or_default();
        let ppid = ppid_de(&base.join("stat"));
        let comm = fs::read_to_string(base.join("comm")).unwrap_or_default().trim().to_string();
        salida.push(Proceso { pid, ppid, comm, exe, cmdline });
    }
    salida
}

#[cfg(target_os = "linux")]
fn ppid_de(ruta: &Path) -> u32 {
    let Ok(texto) = fs::read_to_string(ruta) else { return 0 };
    let Some((_, resto)) = texto.rsplit_once(')') else { return 0 };
    resto.split_whitespace().nth(1).unwrap_or("0").parse().unwrap_or(0)
}

#[cfg(target_os = "macos")]
fn listar_ps() -> Vec<Proceso> {
    let Ok(salida) = Command::new("ps").args(["-axww", "-o", "pid=,ppid=,command="]).output() else { return Vec::new() };
    let texto = String::from_utf8_lossy(&salida.stdout);
    let mut procesos = Vec::new();
    for linea in texto.lines() {
        let linea = linea.trim();
        let mut partes = linea.split_whitespace();
        let Some(pid) = partes.next().and_then(|valor| valor.parse::<u32>().ok()) else { continue };
        let Some(ppid) = partes.next().and_then(|valor| valor.parse::<u32>().ok()) else { continue };
        let cmdline = partes.collect::<Vec<_>>().join(" ");
        let exe = cmdline.split_whitespace().next().unwrap_or("").to_string();
        let comm = exe.rsplit('/').next().unwrap_or(&exe).to_string();
        procesos.push(Proceso { pid, ppid, comm, exe, cmdline });
    }
    procesos
}

pub fn cerrar_proceso(pid: u32) -> bool {
    if pid == 0 || pid == std::process::id() {
        return false;
    }
    extern "C" {
        fn kill(pid: i32, sig: i32) -> i32;
    }
    unsafe { kill(pid as i32, 9) == 0 }
}

pub fn es_navegador(exe: &str) -> bool {
    navegador::destino_de(exe).is_some() || exe.to_ascii_lowercase().contains("safari")
}

pub fn cerrar_navegadores() {
    for nombre in APPS_NAVEGADOR {
        #[cfg(target_os = "macos")]
        {
            let _ = Command::new("killall").arg(nombre).status();
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = nombre;
        }
    }
}

pub fn navegadores_abiertos() -> bool {
    listar_procesos().iter().any(|proceso| es_navegador(&proceso.exe) || es_navegador(&proceso.comm))
}

pub fn agente_en_ejecucion() -> bool {
    let yo = std::process::id();
    listar_procesos().iter().any(|proceso| proceso.pid != yo && es_nuestro(proceso))
}

fn es_nuestro(proceso: &Proceso) -> bool {
    let texto = format!("{} {} {}", proceso.exe, proceso.comm, proceso.cmdline).to_ascii_lowercase();
    texto.contains("aicontrol-daemon") || texto.contains("gobernanza-daemon") || texto.contains("aicontrol.app")
}

pub fn reabrir_navegador(exe: &str, argumentos: &str) -> Result<(), String> {
    let Some(app) = app_de(exe) else { return Ok(()) };
    #[cfg(target_os = "macos")]
    {
        let mut comando = Command::new("open");
        comando.args(["-a", app]);
        if !argumentos.trim().is_empty() {
            comando.arg("--args");
            comando.args(argumentos.split_whitespace());
        }
        return comando.status().map_err(|error| error.to_string()).and_then(|estado| {
            if estado.success() { Ok(()) } else { Err(format!("no se pudo abrir {app}")) }
        });
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (app, argumentos);
        Ok(())
    }
}

fn app_de(exe: &str) -> Option<&'static str> {
    let nombre = exe.replace('\\', "/").rsplit('/').next().unwrap_or(exe).to_ascii_lowercase();
    Some(match nombre.as_str() {
        "google chrome" | "chrome" | "chrome.exe" => "Google Chrome",
        "microsoft edge" | "msedge" | "msedge.exe" => "Microsoft Edge",
        "brave browser" | "brave" | "brave.exe" => "Brave Browser",
        "opera" | "opera.exe" => "Opera",
        "vivaldi" | "vivaldi.exe" => "Vivaldi",
        "chromium" | "chromium.exe" => "Chromium",
        "firefox" | "firefox.exe" => "Firefox",
        "librewolf" | "librewolf.exe" => "LibreWolf",
        "waterfox" | "waterfox.exe" => "Waterfox",
        "floorp" | "floorp.exe" => "Floorp",
        "yandex" | "yandex.exe" => "Yandex",
        _ => return None,
    })
}

pub fn locales_escuchando() -> bool {
    puerto_abierto(4765) && puerto_abierto(8079)
}

fn puerto_abierto(puerto: u16) -> bool {
    std::net::TcpStream::connect_timeout(&std::net::SocketAddr::from(([127, 0, 0, 1], puerto)), Duration::from_millis(200)).is_ok()
}

pub fn primera_instancia() -> bool {
    let carpeta = configuracion::carpeta_instalacion();
    let _ = fs::create_dir_all(&carpeta);
    let ruta = carpeta.join("agente.lock");
    let Ok(archivo) = fs::OpenOptions::new().create(true).read(true).write(true).open(&ruta) else {
        return true;
    };
    #[cfg(unix)]
    {
        use std::os::fd::AsRawFd;
        extern "C" {
            fn flock(fd: i32, op: i32) -> i32;
        }
        let tomado = unsafe { flock(archivo.as_raw_fd(), 2 | 4) } == 0;
        if tomado {
            let _ = CANDADO.set(archivo);
        }
        return tomado;
    }
    #[cfg(not(unix))]
    {
        let _ = archivo;
        true
    }
}

static CANDADO: std::sync::OnceLock<std::fs::File> = std::sync::OnceLock::new();

pub fn permitir_datos_usuario(dir: &Path) {
    let _ = fs::create_dir_all(dir);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(dir, fs::Permissions::from_mode(0o755));
    }
}

pub fn registrar_en_aplicaciones() {}
pub fn fijar_desinstalador_arp() {}
pub fn quitar_inicio() -> Result<(), String> { Ok(()) }
pub fn borrar_tarea() -> Result<(), String> { Ok(()) }
pub fn quitar_de_aplicaciones() {}

pub fn quitar_producto_windows() {
    let _ = fs::remove_file(raiz().join("Library/LaunchDaemons").join(format!("{ETIQUETA_DAEMON}.plist")));
    let _ = fs::remove_file(raiz().join("Library/LaunchAgents").join(format!("{ETIQUETA_MENU}.plist")));
}

pub fn codigo_producto() -> Option<String> {
    None
}

pub fn frenar_reinicio_servicio() {
    if !en_mac_real() {
        return;
    }
    let _ = Command::new("launchctl").args(["disable", &format!("system/{ETIQUETA_DAEMON}")]).status();
}

pub fn detener_y_quitar_servicio() {
    frenar_reinicio_servicio();
    if en_mac_real() {
        let _ = Command::new("launchctl").args(["bootout", &format!("system/{ETIQUETA_DAEMON}")]).status();
    }
    let yo = std::process::id();
    for proceso in listar_procesos() {
        if proceso.pid != yo && es_nuestro(&proceso) && !proceso.cmdline.contains("desinstalar-pasos") && !proceso.cmdline.contains("aplicar-local") {
            let _ = cerrar_proceso(proceso.pid);
        }
    }
}

pub fn arrancar_servicio_gobernanza() -> Result<(), String> {
    if !en_mac_real() {
        return Ok(());
    }
    let plist = instalar_plist()?;
    let _ = Command::new("launchctl").args(["enable", &format!("system/{ETIQUETA_DAEMON}")]).status();
    let _ = Command::new("launchctl").args(["bootout", &format!("system/{ETIQUETA_DAEMON}")]).status();
    let _ = Command::new("launchctl").args(["bootstrap", "system", &plist]).status();
    let estado = Command::new("launchctl").args(["kickstart", "-k", &format!("system/{ETIQUETA_DAEMON}")]).status().map_err(|error| error.to_string())?;
    if estado.success() { Ok(()) } else { Err("no se pudo arrancar el agente".into()) }
}

fn instalar_plist() -> Result<String, String> {
    let destino = raiz().join("Library/LaunchDaemons").join(format!("{ETIQUETA_DAEMON}.plist"));
    let origen = carpeta_programa().join("Contents/Resources").join(format!("{ETIQUETA_DAEMON}.plist"));
    if origen.is_file() {
        if let Some(dir) = destino.parent() {
            fs::create_dir_all(dir).map_err(|error| error.to_string())?;
        }
        fs::copy(&origen, &destino).map_err(|error| error.to_string())?;
    }
    Ok(destino.display().to_string())
}

pub fn terminar_otros_agentes() {
    detener_y_quitar_servicio();
}

pub fn lanzar_desprendido(args: &[String]) -> Result<(), String> {
    let origen = std::env::current_exe().map_err(|error| error.to_string())?;
    let copia = configuracion::carpeta_instalacion().join("pendiente").join("auxiliar");
    if let Some(dir) = copia.parent() {
        fs::create_dir_all(dir).map_err(|error| error.to_string())?;
    }
    fs::copy(&origen, &copia).map_err(|error| error.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        use std::os::unix::process::CommandExt;
        let _ = fs::set_permissions(&copia, fs::Permissions::from_mode(0o755));
        let mut comando = Command::new(&copia);
        comando.args(args);
        comando.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
        unsafe {
            comando.pre_exec(|| {
                extern "C" {
                    fn setsid() -> i32;
                }
                setsid();
                Ok(())
            });
        }
        comando.spawn().map_err(|error| error.to_string())?;
        return Ok(());
    }
    #[cfg(not(unix))]
    {
        let _ = args;
        Err("no se pudo desprender el proceso".into())
    }
}

pub fn reemplazar_desde(staging: &Path) -> Result<(), String> {
    let origen = staging.join("AIcontrol.app");
    if !origen.is_dir() {
        return Err("el paquete no trae AIcontrol.app".into());
    }
    let destino = carpeta_programa();
    let anterior = destino.with_extension("app.anterior");
    let _ = fs::remove_dir_all(&anterior);
    if destino.exists() && fs::rename(&destino, &anterior).is_err() {
        return Err("no se pudo apartar la aplicación anterior".into());
    }
    if let Err(error) = copiar_dir(&origen, &destino) {
        let _ = fs::remove_dir_all(&destino);
        let _ = fs::rename(&anterior, &destino);
        return Err(error);
    }
    let _ = arrancar_servicio_gobernanza();
    let _ = fs::remove_dir_all(&anterior);
    Ok(())
}

fn copiar_dir(origen: &Path, destino: &Path) -> Result<(), String> {
    fs::create_dir_all(destino).map_err(|error| error.to_string())?;
    for entrada in fs::read_dir(origen).map_err(|error| error.to_string())? {
        let entrada = entrada.map_err(|error| error.to_string())?;
        let hacia = destino.join(entrada.file_name());
        if entrada.path().is_dir() {
            copiar_dir(&entrada.path(), &hacia)?;
        } else {
            fs::copy(entrada.path(), &hacia).map_err(|error| error.to_string())?;
            #[cfg(unix)]
            if entrada.file_name().to_string_lossy().contains("AIcontrol") {
                use std::os::unix::fs::PermissionsExt;
                let _ = fs::set_permissions(&hacia, fs::Permissions::from_mode(0o755));
            }
        }
    }
    Ok(())
}

pub fn borrar_carpeta_despues(carpeta: &Path) {
    let ruta = carpeta.display().to_string();
    if ruta.trim().is_empty() || ruta == "/" {
        return;
    }
    let _ = Command::new("sh").args(["-c", &format!("sleep 2; rm -rf -- {}", shell_cita(&ruta))]).stdin(std::process::Stdio::null()).spawn();
}

pub fn borrar_carpeta_ahora(carpeta: &Path) -> Result<(), String> {
    if carpeta.as_os_str().is_empty() || carpeta == Path::new("/") {
        return Ok(());
    }
    let _ = fs::remove_dir_all(carpeta);
    Ok(())
}

fn shell_cita(texto: &str) -> String {
    format!("'{}'", texto.replace('\'', "'\\''"))
}

pub fn raices_firefox() -> Vec<PathBuf> {
    let mut salida = Vec::new();
    if let Ok(appdata) = std::env::var("APPDATA") {
        if !appdata.trim().is_empty() {
            salida.push(PathBuf::from(appdata));
        }
    }
    let usuarios = raiz().join("Users");
    if let Ok(entradas) = fs::read_dir(usuarios) {
        for entrada in entradas.flatten() {
            let soporte = entrada.path().join("Library/Application Support");
            if soporte.is_dir() {
                salida.push(soporte);
            }
        }
    }
    salida
}

pub fn texto_plist(pac: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>ProxyMode</key><string>pac_script</string>
<key>ProxyPacUrl</key><string>{pac}</string>
<key>ProxyPacMandatory</key><true/>
<key>QuicAllowed</key><false/>
</dict></plist>
"#
    )
}

pub fn plist_vigente(texto: &str, pac: &str) -> bool {
    texto.contains(pac) && texto.contains("pac_script") && texto.contains("<key>QuicAllowed</key><false/>") && texto.contains("ProxyPacMandatory")
}

pub fn texto_policies_firefox(pac: &str) -> String {
    format!(
        r#"{{"policies":{{"Proxy":{{"Mode":"pac","AutoConfigURL":"{pac}","Locked":true}},"Certificates":{{"ImportEnterpriseRoots":true}},"Preferences":{{"network.http.http3.enable":{{"Value":false,"Status":"locked"}}}}}}}}"#
    )
}

pub fn aplicar_politica_navegadores(pac: &str) -> Result<(), String> {
    for (_, bundle) in bundles_unicos() {
        escribir_plist(bundle, pac)?;
    }
    for nombre in ["Firefox", "LibreWolf", "Waterfox", "Floorp"] {
        escribir_policies(nombre, pac)?;
    }
    Ok(())
}

fn bundles_unicos() -> Vec<(&'static str, &'static str)> {
    let mut vistos = std::collections::HashSet::new();
    CHROMIUM.iter().copied().filter(|(_, bundle)| vistos.insert(*bundle)).collect()
}

fn escribir_plist(bundle: &str, pac: &str) -> Result<(), String> {
    let ruta = raiz().join("Library/Managed Preferences").join(format!("{bundle}.plist"));
    if let Some(dir) = ruta.parent() {
        fs::create_dir_all(dir).map_err(|error| error.to_string())?;
    }
    fs::write(ruta, texto_plist(pac)).map_err(|error| error.to_string())
}

fn escribir_policies(app: &str, pac: &str) -> Result<(), String> {
    let app_dir = raiz().join("Applications").join(format!("{app}.app"));
    if en_mac_real() && !app_dir.is_dir() {
        return Ok(());
    }
    if !en_mac_real() && !app_dir.exists() {
        return Ok(());
    }
    let ruta = app_dir.join("Contents/Resources/distribution/policies.json");
    if let Some(dir) = ruta.parent() {
        fs::create_dir_all(dir).map_err(|error| error.to_string())?;
    }
    fs::write(ruta, texto_policies_firefox(pac)).map_err(|error| error.to_string())
}

pub fn politicas_navegador_vigentes(pac: &str) -> bool {
    bundles_unicos().into_iter().all(|(_, bundle)| {
        let ruta = raiz().join("Library/Managed Preferences").join(format!("{bundle}.plist"));
        fs::read_to_string(ruta).map(|texto| plist_vigente(&texto, pac)).unwrap_or(false)
    })
}

pub fn politicas_navegador_presentes(pac: &str) -> bool {
    bundles_unicos().into_iter().any(|(_, bundle)| {
        fs::read_to_string(raiz().join("Library/Managed Preferences").join(format!("{bundle}.plist"))).map(|texto| texto.contains(pac)).unwrap_or(false)
    })
}

pub fn filtro_chromium_listo(clave: &str, pac: &str) -> bool {
    let Some(bundle) = bundle_de(clave) else { return true };
    fs::read_to_string(raiz().join("Library/Managed Preferences").join(format!("{bundle}.plist"))).map(|texto| plist_vigente(&texto, pac)).unwrap_or(false)
}

pub fn filtro_firefox_listo(clave: &str, pac: &str) -> bool {
    let Some(app) = firefox_de(clave) else { return true };
    let ruta = raiz().join("Applications").join(format!("{app}.app")).join("Contents/Resources/distribution/policies.json");
    if en_mac_real() && !raiz().join("Applications").join(format!("{app}.app")).is_dir() {
        return true;
    }
    if !ruta.exists() && !en_mac_real() {
        return true;
    }
    fs::read_to_string(ruta).map(|texto| texto.contains(pac) && texto.contains("ImportEnterpriseRoots")).unwrap_or(false)
}

pub fn aplicar_chromium(clave: &str, pac: &str) -> Result<(), String> {
    if let Some(bundle) = bundle_de(clave) {
        escribir_plist(bundle, pac)?;
    }
    Ok(())
}

pub fn aplicar_firefox_clave(clave: &str, pac: &str) -> Result<(), String> {
    if let Some(app) = firefox_de(clave) {
        let dir = raiz().join("Applications").join(format!("{app}.app"));
        if !dir.exists() && !en_mac_real() {
            fs::create_dir_all(&dir).ok();
        }
        escribir_policies(app, pac)?;
    }
    Ok(())
}

fn bundle_de(clave: &str) -> Option<&'static str> {
    CHROMIUM.iter().find(|(registro, _)| *registro == clave).map(|(_, bundle)| *bundle)
}

fn firefox_de(clave: &str) -> Option<&'static str> {
    FIREFOX.iter().find(|(registro, _)| *registro == clave).map(|(_, app)| *app)
}

pub fn escribir_pac(url: &str) -> Result<(), String> {
    let marca = raiz().join("var/gobernanza/pac.url");
    if let Some(dir) = marca.parent() {
        fs::create_dir_all(dir).map_err(|error| error.to_string())?;
    }
    fs::write(&marca, url).map_err(|error| error.to_string())?;
    if en_mac_real() {
        aplicar_networksetup(url)?;
    }
    Ok(())
}

pub fn leer_pac() -> String {
    fs::read_to_string(raiz().join("var/gobernanza/pac.url")).unwrap_or_default()
}

pub fn quitar_pac() -> Result<(), String> {
    let _ = fs::remove_file(raiz().join("var/gobernanza/pac.url"));
    if en_mac_real() {
        for servicio in servicios_de_red() {
            let _ = Command::new("networksetup").args(["-setautoproxystate", &servicio, "off"]).status();
        }
    }
    Ok(())
}

fn aplicar_networksetup(url: &str) -> Result<(), String> {
    let servicios = servicios_de_red();
    if servicios.is_empty() {
        return Err("no hay servicios de red".into());
    }
    for servicio in servicios {
        let estado = Command::new("networksetup").args(["-setautoproxyurl", &servicio, url]).status().map_err(|error| error.to_string())?;
        if !estado.success() {
            return Err(format!("no se pudo fijar el PAC en {servicio}"));
        }
    }
    Ok(())
}

fn servicios_de_red() -> Vec<String> {
    let Ok(salida) = Command::new("networksetup").arg("-listallnetworkservices").output() else { return Vec::new() };
    String::from_utf8_lossy(&salida.stdout)
        .lines()
        .map(str::trim)
        .filter(|linea| !linea.is_empty() && !linea.starts_with("An asterisk") && !linea.starts_with('*'))
        .map(str::to_string)
        .collect()
}

pub fn quitar_politica_navegadores() {
    for (_, bundle) in bundles_unicos() {
        let _ = fs::remove_file(raiz().join("Library/Managed Preferences").join(format!("{bundle}.plist")));
    }
    for nombre in ["Firefox", "LibreWolf", "Waterfox", "Floorp"] {
        let _ = fs::remove_file(raiz().join("Applications").join(format!("{nombre}.app")).join("Contents/Resources/distribution/policies.json"));
    }
}

pub fn quitar_proxy_de_usuarios() {
    let _ = quitar_pac();
}

pub fn filtro_gobernanza_activo() -> bool {
    leer_pac().contains("127.0.0.1:4765") || politicas_navegador_presentes(PAC)
}

pub fn instalar_ca(certificado: &Path) -> Result<(), String> {
    if en_mac_real() {
        let estado = Command::new("security")
            .args(["add-trusted-cert", "-d", "-r", "trustRoot", "-k", "/Library/Keychains/System.keychain"])
            .arg(certificado)
            .status()
            .map_err(|error| error.to_string())?;
        if !estado.success() {
            return Err("no se pudo instalar el certificado en el llavero".into());
        }
    }
    let marca = certificado.with_file_name("instalada");
    fs::write(marca, b"1").map_err(|error| error.to_string())
}

pub fn certificado_instalado() -> bool {
    if en_mac_real() {
        return Command::new("security")
            .args(["find-certificate", "-c", "Gobernanza 2", "/Library/Keychains/System.keychain"])
            .status()
            .map(|estado| estado.success())
            .unwrap_or(false);
    }
    configuracion::carpeta_instalacion().join("ca").join("instalada").is_file()
}

pub fn quitar_ca() -> Result<(), String> {
    let _ = fs::remove_file(configuracion::carpeta_instalacion().join("ca").join("instalada"));
    if en_mac_real() {
        let _ = Command::new("security").args(["delete-certificate", "-c", "Gobernanza 2", "/Library/Keychains/System.keychain"]).status();
    }
    Ok(())
}

fn raiz() -> PathBuf {
    if let Ok(valor) = std::env::var("GOBERNANZA_RAIZ") {
        if !valor.trim().is_empty() {
            return PathBuf::from(valor);
        }
    }
    if cfg!(target_os = "macos") {
        PathBuf::from("/")
    } else {
        configuracion::carpeta_instalacion().join("raiz-simulada")
    }
}

fn en_mac_real() -> bool {
    cfg!(target_os = "macos") && std::env::var("GOBERNANZA_RAIZ").ok().filter(|valor| !valor.trim().is_empty()).is_none()
}

#[cfg(target_os = "macos")]
fn sysctl_u64(nombre: &std::ffi::CStr) -> Option<u64> {
    extern "C" {
        fn sysctlbyname(name: *const i8, oldp: *mut u8, oldlenp: *mut usize, newp: *mut u8, newlen: usize) -> i32;
    }
    let mut valor = 0u64;
    let mut largo = std::mem::size_of::<u64>();
    let rc = unsafe { sysctlbyname(nombre.as_ptr(), &mut valor as *mut u64 as *mut u8, &mut largo, std::ptr::null_mut(), 0) };
    if rc == 0 { Some(valor) } else { None }
}

#[cfg(target_os = "macos")]
fn sysctl_time(nombre: &std::ffi::CStr) -> Option<u64> {
    extern "C" {
        fn sysctlbyname(name: *const i8, oldp: *mut u8, oldlenp: *mut usize, newp: *mut u8, newlen: usize) -> i32;
    }
    #[repr(C)]
    struct Timeval {
        sec: i64,
        usec: i32,
    }
    let mut valor = Timeval { sec: 0, usec: 0 };
    let mut largo = std::mem::size_of::<Timeval>();
    let rc = unsafe { sysctlbyname(nombre.as_ptr(), &mut valor as *mut Timeval as *mut u8, &mut largo, std::ptr::null_mut(), 0) };
    if rc == 0 && valor.sec > 0 { Some(valor.sec as u64) } else { None }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_plist_obliga_el_pac_y_apaga_quic() {
        let texto = texto_plist(PAC);
        assert!(plist_vigente(&texto, PAC));
        assert!(!plist_vigente(&texto, "http://otro"));
        let policies = texto_policies_firefox(PAC);
        assert!(policies.contains(PAC));
        assert!(policies.contains("ImportEnterpriseRoots"));
    }

    #[test]
    fn el_inventario_de_esta_maquina_tiene_sistema() {
        let cuerpo = inventario_base();
        let sistema = cuerpo["sistema"].as_str().unwrap();
        assert!(sistema == "linux" || sistema == "macos");
        assert!(cuerpo["cpu"].as_str().unwrap().contains("núcleos"));
        assert!(cuerpo["ramMb"].as_i64().unwrap() >= 1);
        assert!(id_arranque() > 0);
        assert!(nombre_de_persona("root").is_none());
        assert_eq!(nombre_de_persona("ana").as_deref(), Some("ana"));
    }

    #[test]
    fn las_politicas_se_escriben_bajo_la_raiz_de_prueba() {
        let dir = std::env::temp_dir().join(format!("gob-mac-raiz-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        std::env::set_var("GOBERNANZA_RAIZ", &dir);
        aplicar_politica_navegadores(PAC).unwrap();
        escribir_pac(PAC).unwrap();
        assert!(politicas_navegador_vigentes(PAC));
        assert_eq!(leer_pac().trim(), PAC);
        assert!(filtro_chromium_listo(r"HKCU\Software\Policies\Google\Chrome", PAC));
        quitar_politica_navegadores();
        quitar_pac().unwrap();
        assert!(!politicas_navegador_vigentes(PAC));
        assert!(leer_pac().trim().is_empty());
        std::env::remove_var("GOBERNANZA_RAIZ");
        let _ = fs::remove_dir_all(&dir);
    }
}
