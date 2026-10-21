#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DestinoFiltro {
    Chromium(&'static str),
    Firefox(&'static str),
}

pub fn mismo_nombre(a: &str, b: &str) -> bool {
    let nombre = nombre_exe(a);
    !nombre.is_empty() && nombre == nombre_exe(b)
}

pub fn argumentos_reapertura(destino: DestinoFiltro) -> String {
    match destino {
        DestinoFiltro::Chromium(_) => format!("--disable-quic --proxy-pac-url={PAC} --restore-last-session"),
        DestinoFiltro::Firefox(_) => String::new(),
    }
}

fn nombre_exe(ruta: &str) -> String {
    ruta.replace('/', "\\").rsplit('\\').next().unwrap_or(ruta).to_ascii_lowercase()
}

pub fn destino_de(exe: &str) -> Option<DestinoFiltro> {
    let ruta = exe.replace('/', "\\").to_ascii_lowercase();
    let nombre = ruta.rsplit('\\').next().unwrap_or(&ruta);
    let chromium = DestinoFiltro::Chromium;
    let firefox = DestinoFiltro::Firefox;
    Some(match nombre {
        "chrome.exe" => chromium(r"HKCU\Software\Policies\Google\Chrome"),
        "msedge.exe" => chromium(r"HKCU\Software\Policies\Microsoft\Edge"),
        "brave.exe" if ruta.contains("brave-beta") => chromium(r"HKCU\Software\Policies\BraveSoftware\Brave-Beta"),
        "brave.exe" => chromium(r"HKCU\Software\Policies\BraveSoftware\Brave"),
        "opera.exe" if ruta.contains("opera gx") => chromium(r"HKCU\Software\Policies\Opera GX"),
        "opera.exe" => chromium(r"HKCU\Software\Policies\Opera"),
        "vivaldi.exe" => chromium(r"HKCU\Software\Policies\Vivaldi"),
        "chromium.exe" => chromium(r"HKCU\Software\Policies\Chromium"),
        "yandex.exe" => chromium(r"HKCU\Software\Policies\YandexBrowser"),
        "firefox.exe" => firefox(r"HKCU\Software\Policies\Mozilla\Firefox"),
        "librewolf.exe" => firefox(r"HKCU\Software\Policies\LibreWolf"),
        "waterfox.exe" => firefox(r"HKCU\Software\Policies\Waterfox"),
        "floorp.exe" => firefox(r"HKCU\Software\Policies\Floorp"),
        "google chrome" | "chrome" => chromium(r"HKCU\Software\Policies\Google\Chrome"),
        "microsoft edge" | "msedge" => chromium(r"HKCU\Software\Policies\Microsoft\Edge"),
        "brave browser" | "brave" => chromium(r"HKCU\Software\Policies\BraveSoftware\Brave"),
        "opera gx" => chromium(r"HKCU\Software\Policies\Opera GX"),
        "opera" => chromium(r"HKCU\Software\Policies\Opera"),
        "vivaldi" => chromium(r"HKCU\Software\Policies\Vivaldi"),
        "chromium" => chromium(r"HKCU\Software\Policies\Chromium"),
        "yandex" => chromium(r"HKCU\Software\Policies\YandexBrowser"),
        "firefox" => firefox(r"HKCU\Software\Policies\Mozilla\Firefox"),
        "librewolf" => firefox(r"HKCU\Software\Policies\LibreWolf"),
        "waterfox" => firefox(r"HKCU\Software\Policies\Waterfox"),
        "floorp" => firefox(r"HKCU\Software\Policies\Floorp"),
        _ => return None,
    })
}

pub const MARCA_INICIO: &str = "// gobernanza-proxy inicio";
pub const MARCA_FIN: &str = "// gobernanza-proxy fin";
pub const PAC: &str = "http://127.0.0.1:4765/proxy.pac";

pub fn bloque_proxy() -> String {
    format!(
        "{MARCA_INICIO}\nuser_pref(\"network.proxy.type\", 2);\nuser_pref(\"network.proxy.autoconfig_url\", \"{PAC}\");\nuser_pref(\"security.enterprise_roots.enabled\", true);\nuser_pref(\"network.http.http3.enable\", false);\n{MARCA_FIN}\n"
    )
}

pub fn escribir_user_js(actual: &str) -> String {
    let limpio = quitar_bloque(actual);
    format!("{limpio}{}", bloque_proxy())
}

pub fn perfil_tiene_proxy(texto: &str) -> bool {
    texto.contains(MARCA_INICIO) && texto.contains(PAC) && texto.contains("security.enterprise_roots.enabled") && texto.contains("network.http.http3.enable")
}

pub fn rutas_de_perfiles(ini: &str) -> Vec<(bool, String)> {
    let mut relativo = true;
    let mut rutas = Vec::new();
    for linea in ini.lines() {
        let linea = linea.trim();
        if linea.starts_with('[') {
            relativo = true;
            continue;
        }
        if let Some(valor) = linea.strip_prefix("IsRelative=") {
            relativo = valor.trim() != "0";
        } else if let Some(valor) = linea.strip_prefix("Path=") {
            let valor = valor.trim();
            if !valor.is_empty() {
                rutas.push((relativo, valor.to_string()));
            }
        }
    }
    rutas
}

pub fn listar_perfiles(raiz: &std::path::Path) -> Vec<std::path::PathBuf> {
    use std::collections::BTreeSet;
    let mut vistos = BTreeSet::new();
    let mut perfiles = Vec::new();
    let agregar = |perfiles: &mut Vec<std::path::PathBuf>, vistos: &mut BTreeSet<String>, ruta: std::path::PathBuf| {
        if !ruta.join("prefs.js").is_file() && !ruta.join("user.js").is_file() {
            return;
        }
        let clave = ruta.to_string_lossy().replace('/', "\\").to_ascii_lowercase();
        if vistos.insert(clave) {
            perfiles.push(ruta);
        }
    };
    if let Ok(texto) = std::fs::read_to_string(raiz.join("profiles.ini")) {
        for (es_relativo, ruta) in rutas_de_perfiles(&texto) {
            let path = if es_relativo { raiz.join(ruta) } else { std::path::PathBuf::from(ruta) };
            agregar(&mut perfiles, &mut vistos, path);
        }
    }
    if perfiles.is_empty() {
        if let Ok(entradas) = std::fs::read_dir(raiz.join("Profiles")) {
            for entrada in entradas.flatten() {
                agregar(&mut perfiles, &mut vistos, entrada.path());
            }
        }
    }
    perfiles
}

pub fn asegurar_firefox(raiz: &std::path::Path) -> bool {
    let perfiles = listar_perfiles(raiz);
    if perfiles.is_empty() {
        return true;
    }
    for perfil in &perfiles {
        let user_js = perfil.join("user.js");
        let actual = std::fs::read_to_string(&user_js).unwrap_or_default();
        if !perfil_tiene_proxy(&actual) && std::fs::write(&user_js, escribir_user_js(&actual)).is_err() {
            return false;
        }
    }
    firefox_vigente(raiz)
}

pub fn firefox_tiene_marca(raiz: &std::path::Path) -> bool {
    listar_perfiles(raiz).iter().any(|perfil| {
        std::fs::read_to_string(perfil.join("user.js")).unwrap_or_default().contains(MARCA_INICIO)
    })
}

pub fn firefox_vigente(raiz: &std::path::Path) -> bool {
    listar_perfiles(raiz).iter().all(|perfil| {
        perfil_tiene_proxy(&std::fs::read_to_string(perfil.join("user.js")).unwrap_or_default())
    })
}

pub fn raices_firefox(appdata: &std::path::Path) -> Vec<std::path::PathBuf> {
    ["Mozilla/Firefox", "Mozilla/Firefox Nightly", "Firefox", "Firefox Nightly", "librewolf", "LibreWolf", "Waterfox", "Floorp"]
        .into_iter()
        .map(|ruta| appdata.join(ruta))
        .collect()
}

pub fn asegurar_familia_firefox(appdata: &std::path::Path) -> bool {
    raices_firefox(appdata).iter().all(|raiz| !raiz.exists() || asegurar_firefox(raiz))
}

pub fn familia_firefox_vigente(appdata: &std::path::Path) -> bool {
    raices_firefox(appdata).iter().all(|raiz| !raiz.exists() || firefox_vigente(raiz))
}

pub fn quitar_familia_firefox(appdata: &std::path::Path) {
    for raiz in raices_firefox(appdata) {
        quitar_firefox(&raiz);
    }
}

pub fn quitar_firefox(raiz: &std::path::Path) {
    for perfil in listar_perfiles(raiz) {
        let user_js = perfil.join("user.js");
        let Ok(actual) = std::fs::read_to_string(&user_js) else { continue };
        let _ = std::fs::write(&user_js, quitar_bloque(&actual));
    }
}

pub fn aplicar_perfiles(raiz: &std::path::Path) -> usize {
    let mut n = 0;
    let Ok(entradas) = std::fs::read_dir(raiz) else { return 0 };
    for entrada in entradas.flatten() {
        let user_js = entrada.path().join("user.js");
        if !entrada.path().join("prefs.js").is_file() && !user_js.is_file() {
            continue;
        }
        let actual = std::fs::read_to_string(&user_js).unwrap_or_default();
        if std::fs::write(&user_js, escribir_user_js(&actual)).is_ok() {
            n += 1;
        }
    }
    n
}

pub fn quitar_perfiles(raiz: &std::path::Path) -> usize {
    let mut n = 0;
    let Ok(entradas) = std::fs::read_dir(raiz) else { return 0 };
    for entrada in entradas.flatten() {
        let user_js = entrada.path().join("user.js");
        let Ok(actual) = std::fs::read_to_string(&user_js) else { continue };
        if std::fs::write(&user_js, quitar_bloque(&actual)).is_ok() {
            n += 1;
        }
    }
    n
}

pub fn quitar_bloque(texto: &str) -> String {
    let Some(inicio) = texto.find(MARCA_INICIO) else {
        return texto.to_string();
    };
    let Some(fin_rel) = texto[inicio..].find(MARCA_FIN) else {
        return texto[..inicio].to_string();
    };
    let fin = inicio + fin_rel + MARCA_FIN.len();
    let mut salida = String::new();
    salida.push_str(&texto[..inicio]);
    salida.push_str(texto[fin..].trim_start_matches('\n'));
    salida
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_user_js_se_puede_repetir_y_quitar() {
        let uno = escribir_user_js("user_pref(\"a\", 1);\n");
        let dos = escribir_user_js(&uno);
        assert_eq!(dos.matches(MARCA_INICIO).count(), 1);
        assert!(dos.contains("enterprise_roots"));
        assert_eq!(destino_de(r"C:\Program Files\Google\Chrome\Application\chrome.exe"), Some(DestinoFiltro::Chromium(r"HKCU\Software\Policies\Google\Chrome")));
        assert_eq!(destino_de(r"C:\Users\dev\AppData\Local\Programs\Opera GX\opera.exe"), Some(DestinoFiltro::Chromium(r"HKCU\Software\Policies\Opera GX")));
        assert_eq!(destino_de(r"C:\Program Files\Mozilla Firefox\firefox.exe"), Some(DestinoFiltro::Firefox(r"HKCU\Software\Policies\Mozilla\Firefox")));
        assert_eq!(destino_de("Google Chrome"), Some(DestinoFiltro::Chromium(r"HKCU\Software\Policies\Google\Chrome")));
        assert_eq!(destino_de("/Applications/Firefox.app/Contents/MacOS/firefox"), Some(DestinoFiltro::Firefox(r"HKCU\Software\Policies\Mozilla\Firefox")));
        assert_eq!(destino_de(r"C:\Windows\explorer.exe"), None);
        assert!(mismo_nombre(r"C:\Program Files\Google\Chrome\Application\chrome.exe", r"C:\Program Files\Google\Chrome\Application\chrome.exe"));
        assert!(!mismo_nombre(r"C:\Program Files\Google\Chrome\Application\chrome.exe", r"C:\Windows\explorer.exe"));
        let chrome = argumentos_reapertura(DestinoFiltro::Chromium(r"HKCU\Software\Policies\Google\Chrome"));
        assert!(chrome.contains("--proxy-pac-url="));
        assert!(chrome.contains("--disable-quic"));
        assert!(argumentos_reapertura(DestinoFiltro::Firefox(r"HKCU\Software\Policies\Mozilla\Firefox")).is_empty());
        let limpio = quitar_bloque(&dos);
        assert!(!limpio.contains(MARCA_INICIO));
        assert!(limpio.contains("user_pref(\"a\", 1);"));
    }

    #[test]
    fn aplica_el_proxy_en_cada_perfil() {
        let dir = std::env::temp_dir().join(format!("gob-ff-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let perfil = dir.join("perfil.default");
        std::fs::create_dir_all(&perfil).unwrap();
        std::fs::write(perfil.join("prefs.js"), "").unwrap();
        assert_eq!(aplicar_perfiles(&dir), 1);
        assert!(std::fs::read_to_string(perfil.join("user.js")).unwrap().contains(PAC));
        assert_eq!(quitar_perfiles(&dir), 1);
        assert!(!std::fs::read_to_string(perfil.join("user.js")).unwrap().contains(MARCA_INICIO));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn el_perfil_solo_cuenta_si_tiene_el_proxy_y_la_raiz() {
        assert!(perfil_tiene_proxy(&bloque_proxy()));
        assert!(!perfil_tiene_proxy("user_pref(\"a\", 1);\n"));
    }

    #[test]
    fn los_perfiles_salen_de_profiles_ini() {
        let ini = "[Profile0]\nName=release\nIsRelative=1\nPath=Profiles/abc.default-release\n[Profile1]\nName=otro\nIsRelative=0\nPath=D:/Firefox/otro\n";
        assert_eq!(
            rutas_de_perfiles(ini),
            vec![
                (true, "Profiles/abc.default-release".into()),
                (false, "D:/Firefox/otro".into()),
            ]
        );
    }
}
