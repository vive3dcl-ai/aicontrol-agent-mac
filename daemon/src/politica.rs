#[derive(Clone, Debug, PartialEq)]
pub struct Regla {
    pub id: String,
    pub host: String,
    pub accion: String,
    pub nombre: String,
    pub redireccion_nombre: String,
    pub redireccion_url: String,
    pub receta: Receta,
}

#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Receta {
    #[serde(default)]
    pub version: u8,
    #[serde(default)]
    pub ruta: CondicionRuta,
    #[serde(default)]
    pub query: Vec<CondicionQuery>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CondicionRuta {
    #[serde(default = "operador_ruta_defecto")]
    pub operador: String,
    #[serde(default)]
    pub valor: String,
}

impl Default for CondicionRuta {
    fn default() -> Self {
        Self { operador: operador_ruta_defecto(), valor: String::new() }
    }
}

fn operador_ruta_defecto() -> String { "cualquiera".into() }

#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CondicionQuery {
    #[serde(default)]
    pub nombre: String,
    #[serde(default)]
    pub operador: String,
    #[serde(default)]
    pub valor: String,
}

#[derive(Clone, Debug)]
pub struct Sitio {
    pub host: String,
    pub proveedor_id: String,
    pub endpoint_id: String,
    pub superficie: String,
    pub receta: Receta,
}

#[derive(Clone, Debug)]
pub struct Politica {
    pub id: String,
    pub nombre: String,
    pub accion: String,
    pub alcance: String,
    pub sujeto: String,
    pub destino: String,
    pub proveedor_id: String,
    pub endpoint_id: String,
    pub actualizada_en: f64,
    pub redireccion_nombre: String,
    pub redireccion_url: String,
}

#[derive(Clone, Debug)]
pub struct Grupo {
    pub id: String,
    pub usuarios: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct Sombra {
    pub accion: String,
    pub nombre: String,
    pub url: String,
}

impl Default for Sombra {
    fn default() -> Self {
        Self { accion: "permitir".into(), nombre: String::new(), url: String::new() }
    }
}

pub fn resolver(sitios: &[Sitio], politicas: &[Politica], grupos: &[Grupo], dispositivo: &str, hostname: &str, claves: &[String], modo_web: &str, sombra: &Sombra) -> (Vec<Regla>, Vec<String>) {
    let mut hosts = Vec::new();
    for sitio in sitios {
        if !sitio.host.is_empty() && !sitio.proveedor_id.starts_with("feed:") {
            hosts.push(sitio.host.to_ascii_lowercase());
        }
    }
    let mut miembros = std::collections::HashMap::new();
    for grupo in grupos {
        if grupo.usuarios.iter().any(|usuario| claves.iter().any(|clave| clave == usuario)) {
            miembros.insert(grupo.id.clone(), true);
        }
    }
    let mut objetivos = sitios.to_vec();
    let mut vistos: std::collections::HashMap<String, bool> = sitios.iter().map(|sitio| (sitio.host.clone(), true)).collect();
    for politica in politicas {
        if !politica.proveedor_id.is_empty() || !politica.endpoint_id.is_empty() || vistos.contains_key(&politica.destino) {
            continue;
        }
        vistos.insert(politica.destino.clone(), true);
        objetivos.push(Sitio { host: politica.destino.clone(), proveedor_id: String::new(), endpoint_id: String::new(), superficie: String::new(), receta: Receta::default() });
    }
    let mut reglas = Vec::new();
    for objetivo in objetivos {
        if let Some(regla) = elegir(&objetivo, politicas, dispositivo, hostname, claves, &miembros, modo_web, sombra) {
            reglas.push(regla);
        }
    }
    reglas.sort_by(|a, b| a.host.cmp(&b.host));
    (reglas, hosts)
}

fn elegir(objetivo: &Sitio, politicas: &[Politica], dispositivo: &str, hostname: &str, claves: &[String], grupos: &std::collections::HashMap<String, bool>, modo_web: &str, sombra: &Sombra) -> Option<Regla> {
    let mut mejor: Option<&Politica> = None;
    let mut rango_mejor = -1;
    for politica in politicas {
        if !aplica(politica, dispositivo, hostname, claves, grupos) || !apunta(politica, objetivo) {
            continue;
        }
        let rango = rango_de(politica);
        let reemplaza = mejor.is_none_or(|actual| rango > rango_mejor || (rango == rango_mejor && politica.actualizada_en > actual.actualizada_en));
        if reemplaza {
            mejor = Some(politica);
            rango_mejor = rango;
        }
    }
    let Some(mejor) = mejor else {
        if objetivo.proveedor_id.starts_with("feed:") {
            return regla_sombra(objetivo, sombra);
        }
        if modo_web != "bloquear" || objetivo.superficie != "web" {
            return None;
        }
        return Some(Regla {
            id: "modo-web".into(),
            host: objetivo.host.to_ascii_lowercase(),
            accion: "bloquear".into(),
            nombre: "Modo web: bloquear".into(),
            redireccion_nombre: String::new(),
            redireccion_url: String::new(),
            receta: objetivo.receta.clone(),
        });
    };
    if mejor.accion == "permitir" || (mejor.accion == "redirigir" && mejor.redireccion_url.is_empty()) {
        return None;
    }
    Some(Regla {
        id: mejor.id.clone(),
        host: objetivo.host.to_ascii_lowercase(),
        accion: mejor.accion.clone(),
        nombre: mejor.nombre.clone(),
        redireccion_nombre: mejor.redireccion_nombre.clone(),
        redireccion_url: mejor.redireccion_url.clone(),
        receta: objetivo.receta.clone(),
    })
}

fn regla_sombra(objetivo: &Sitio, sombra: &Sombra) -> Option<Regla> {
    let accion = match sombra.accion.as_str() {
        "bloquear" => "bloquear",
        "redirigir" if !sombra.url.is_empty() => "redirigir",
        _ => return None,
    };
    Some(Regla {
        id: "shadow-ai".into(),
        host: objetivo.host.to_ascii_lowercase(),
        accion: accion.into(),
        nombre: "Shadow AI".into(),
        redireccion_nombre: sombra.nombre.clone(),
        redireccion_url: sombra.url.clone(),
        receta: objetivo.receta.clone(),
    })
}

fn rango_de(politica: &Politica) -> i32 {
    let base = match politica.alcance.as_str() {
        "equipo" => 40,
        "usuario" => 30,
        "grupo" => 20,
        _ => 10,
    };
    let especificidad = if !politica.endpoint_id.is_empty() || politica.proveedor_id.is_empty() { 2 } else { 1 };
    base * 10 + especificidad
}

fn aplica(politica: &Politica, dispositivo: &str, hostname: &str, claves: &[String], grupos: &std::collections::HashMap<String, bool>) -> bool {
    match politica.alcance.as_str() {
        "tenant" => true,
        "usuario" => !politica.sujeto.is_empty() && claves.iter().any(|clave| clave == &politica.sujeto),
        "equipo" => politica.sujeto == dispositivo || politica.sujeto == hostname,
        "grupo" => !politica.sujeto.is_empty() && grupos.get(&politica.sujeto) == Some(&true),
        _ => false,
    }
}

fn apunta(politica: &Politica, objetivo: &Sitio) -> bool {
    if !politica.endpoint_id.is_empty() {
        return politica.endpoint_id == objetivo.endpoint_id;
    }
    if !politica.proveedor_id.is_empty() {
        return politica.proveedor_id == objetivo.proveedor_id;
    }
    if politica.destino == objetivo.host {
        return true;
    }
    if objetivo.host.starts_with("*.") {
        return false;
    }
    host_cubierto(&politica.destino, &objetivo.host)
}

fn host_cubierto(patron: &str, host: &str) -> bool {
    let patron = patron.to_ascii_lowercase();
    let host = host.to_ascii_lowercase();
    if let Some(base) = patron.strip_prefix("*.") {
        host == base || host.ends_with(&format!(".{base}"))
    } else {
        patron == host
    }
}

pub fn buscar_regla(host: &str, reglas: &[Regla]) -> Option<Regla> {
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    if let Ok(indice) = reglas.binary_search_by(|regla| regla.host.as_str().cmp(host.as_str())) {
        return Some(reglas[indice].clone());
    }
    let mut comodin = None;
    for regla in reglas {
        if regla.host.starts_with("*.") && coincide_host(&regla.host, &host) {
            comodin = Some(regla.clone());
        }
    }
    comodin
}

pub fn coincide_host(patron: &str, host: &str) -> bool {
    let patron = patron.trim_end_matches('.').to_ascii_lowercase();
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    if patron == host {
        return true;
    }
    let Some(base) = patron.strip_prefix("*.") else { return false };
    host == base || host.ends_with(&format!(".{base}"))
}

pub fn es_host_ia(host: &str) -> bool {
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    host == "chatgpt.com" || host.ends_with(".chatgpt.com") || host == "openai.com" || host.ends_with(".openai.com")
}

pub fn requiere_inspeccion(receta: &Receta) -> bool {
    receta.ruta.operador != "cualquiera" || !receta.query.is_empty()
}

pub fn cumple_receta(uri: &str, receta: &Receta) -> bool {
    if receta.version > 1 {
        return false;
    }
    let (ruta, consulta) = uri.split_once('?').unwrap_or((uri, ""));
    let ruta_ok = match receta.ruta.operador.as_str() {
        "cualquiera" | "" => true,
        "igual" => ruta == receta.ruta.valor,
        "prefijo" => ruta.starts_with(&receta.ruta.valor),
        _ => false,
    };
    if !ruta_ok {
        return false;
    }
    receta.query.iter().all(|condicion| {
        consulta.split('&').any(|parte| {
            let parte = decodificar_porcentaje(parte);
            let (nombre, valor) = parte.split_once('=').unwrap_or((&parte, ""));
            if !nombre.eq_ignore_ascii_case(&condicion.nombre) {
                return false;
            }
            condicion.operador == "existe"
                || (condicion.operador == "igual" && valor == condicion.valor)
        })
    })
}

fn decodificar_porcentaje(texto: &str) -> String {
    let bytes = texto.as_bytes();
    let mut salida = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let a = (bytes[i + 1] as char).to_digit(16);
            let b = (bytes[i + 2] as char).to_digit(16);
            if let (Some(a), Some(b)) = (a, b) {
                salida.push((a * 16 + b) as u8);
                i += 3;
                continue;
            }
        }
        salida.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&salida).into_owned()
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Decision {
    Tunel,
    Mitm,
    Formula,
    Aviso,
}

pub fn decidir(host: &str, reglas: &[Regla]) -> Decision {
    if let Some(regla) = buscar_regla(host, reglas) {
        return if requiere_inspeccion(&regla.receta) { Decision::Formula } else { Decision::Aviso };
    }
    if es_host_ia(host) {
        Decision::Mitm
    } else {
        Decision::Tunel
    }
}

pub fn texto_pac(sitios: &[String], reglas: &[Regla], resto: &str, modo_web: &str) -> String {
    let mut vistos = vec!["chatgpt.com".to_string(), "*.chatgpt.com".into(), "openai.com".into(), "*.openai.com".into()];
    let mut tengo: std::collections::HashMap<String, bool> = vistos.iter().map(|patron| (patron.clone(), true)).collect();
    for host in sitios.iter().chain(reglas.iter().map(|regla| &regla.host)) {
        if !host_de_pac(host) || tengo.contains_key(host) {
            continue;
        }
        tengo.insert(host.clone(), true);
        vistos.push(host.clone());
    }
    let mut condiciones = Vec::new();
    for patron in &vistos {
        condiciones.push(condicion_de_pac(patron));
    }
    let mut filtro = if modo_web == "bloquear" { r#"url.substring(0,6) == "https:""#.into() } else { condiciones.join(" || ") };
    if filtro.is_empty() {
        filtro = "false".into();
    }
    let resto = if resto.is_empty() { "DIRECT" } else { resto };
    format!(
        "function FindProxyForURL(url, host) {{\n  if (isPlainHostName(host) || host == \"127.0.0.1\" || host == \"::1\" || shExpMatch(host, \"127.*\")) return \"DIRECT\";\n  if ({filtro}) {{\n    return \"PROXY 127.0.0.1:8079\";\n  }}\n  return \"{resto}\";\n}}\n"
    )
}

fn condicion_de_pac(patron: &str) -> String {
    if let Some(base) = patron.strip_prefix("*.") {
        format!(r#"shExpMatch(host, "{patron}") || shExpMatch(host, "{base}")"#)
    } else {
        format!(r#"shExpMatch(host, "{patron}") || shExpMatch(host, "*.{patron}")"#)
    }
}

fn host_de_pac(patron: &str) -> bool {
    let patron = patron.to_ascii_lowercase();
    let cuerpo = patron.strip_prefix("*.").unwrap_or(&patron);
    if cuerpo == patron && patron.contains('*') {
        return false;
    }
    if cuerpo.len() < 3 || cuerpo.starts_with('.') || cuerpo.ends_with('.') || cuerpo.contains("..") || cuerpo.contains([' ', '/', '"', ':']) || !cuerpo.contains('.') {
        return false;
    }
    cuerpo.chars().all(|caracter| caracter.is_ascii_alphanumeric() || caracter == '.' || caracter == '-')
}

pub fn escapar_html(texto: &str) -> String {
    texto.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

pub fn pagina_bloqueo(host: &str) -> String {
    pagina_aviso("Sitio no permitido", &format!("El sitio {} no está permitido por el administrador.", escapar_html(host)), "")
}

pub fn pagina_redireccion(host: &str, nombre: &str, destino: &str) -> String {
    let seguro = escapar_html(destino);
    let cuerpo = format!(
        "El sitio {} no está permitido. Serás redirigido a {}.",
        escapar_html(host),
        escapar_html(nombre)
    );
    let extra = format!(r#"<p><a href="{seguro}">Continuar</a></p><meta http-equiv="refresh" content="5;url={seguro}">"#);
    pagina_aviso("Redirección", &cuerpo, &extra)
}

fn pagina_aviso(titulo: &str, cuerpo: &str, extra: &str) -> String {
    format!(
        r#"<!DOCTYPE html><html lang="es"><head><meta charset="utf-8"><title>{}</title>
<style>body{{font-family:sans-serif;background:#0c0a09;color:#fafaf9;display:flex;min-height:100vh;align-items:center;justify-content:center;margin:0}}main{{max-width:32rem;padding:2rem}}a{{color:#fafaf9}}</style></head>
<body><main><h1>{}</h1><p>{}</p>{}</main></body></html>"#,
        escapar_html(titulo),
        escapar_html(titulo),
        cuerpo,
        extra
    )
}

pub fn normalizar_proxy(valor: &str) -> Option<String> {
    let valor = valor.trim();
    if valor.is_empty() || valor.to_ascii_lowercase().starts_with("socks") {
        return None;
    }
    let mut texto = valor.to_string();
    if !texto.contains("://") {
        texto = format!("http://{texto}");
    }
    let resto = texto.split_once("://")?.1;
    let autoridad = resto.split(['/', '?', '#']).next()?.trim();
    if autoridad.is_empty() || autoridad.starts_with('@') {
        return None;
    }
    let (host, puerto) = if let Some((host, puerto)) = autoridad.rsplit_once(':') {
        (host, puerto)
    } else {
        return None;
    };
    if host.eq_ignore_ascii_case("127.0.0.1") && puerto == "8079" {
        return None;
    }
    if host.eq_ignore_ascii_case("localhost") && puerto == "8079" {
        return None;
    }
    Some(format!("{host}:{puerto}"))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn politica(accion: &str, alcance: &str, sujeto: &str, destino: &str) -> Politica {
        Politica {
            id: format!("{alcance}-{destino}"),
            nombre: alcance.into(),
            accion: accion.into(),
            alcance: alcance.into(),
            sujeto: sujeto.into(),
            destino: destino.into(),
            proveedor_id: String::new(),
            endpoint_id: String::new(),
            actualizada_en: 1.0,
            redireccion_nombre: String::new(),
            redireccion_url: String::new(),
        }
    }

    fn receta_google() -> Receta {
        Receta {
            version: 1,
            ruta: CondicionRuta { operador: "igual".into(), valor: "/search".into() },
            query: vec![CondicionQuery { nombre: "udm".into(), operador: "igual".into(), valor: "50".into() }],
        }
    }

    #[test]
    fn el_equipo_pisa_al_tenant() {
        let sitios = vec![Sitio {
            host: "ia.example".into(),
            proveedor_id: String::new(),
            endpoint_id: String::new(),
            superficie: "web".into(),
            receta: Receta::default(),
        }];
        let politicas = vec![
            politica("bloquear", "tenant", "", "ia.example"),
            politica("permitir", "equipo", "pc-1", "ia.example"),
        ];
        let (reglas, _) = resolver(&sitios, &politicas, &[], "pc-1", "pc-1", &[], "aceptar", &Sombra::default());
        assert!(reglas.is_empty());
        let (reglas, _) = resolver(&sitios, &politicas[..1], &[], "pc-1", "pc-1", &[], "aceptar", &Sombra::default());
        assert_eq!(reglas[0].accion, "bloquear");
    }

    #[test]
    fn el_pac_incluye_hosts_definidos_por_el_servidor() {
        let sin = texto_pac(&[], &[], "", "aceptar");
        assert!(sin.contains("PROXY 127.0.0.1:8079"));
        assert!(!sin.contains("PROXY 127.0.0.1:8079; DIRECT"));
        let google = Regla {
            id: "g".into(),
            host: "google.com".into(),
            accion: "bloquear".into(),
            nombre: "Google".into(),
            redireccion_nombre: String::new(),
            redireccion_url: String::new(),
            receta: receta_google(),
        };
        let con = texto_pac(&[], &[google], "PROXY 10.0.0.1:8080", "aceptar");
        assert!(con.contains(r#"shExpMatch(host, "google.com")"#));
        assert!(con.contains("PROXY 10.0.0.1:8080"));
    }

    #[test]
    fn el_pac_exige_borde_de_dominio_y_direct_para_el_resto() {
        let pac = texto_pac(&[".com".into(), "chatgpt.com".into()], &[], "", "aceptar");
        assert!(!pac.contains("dnsDomainIs"));
        assert!(!pac.contains(r#"shExpMatch(host, ".com")"#));
        assert!(!pac.contains(r#"shExpMatch(host, "*.com")"#));
        assert!(pac.contains(r#"shExpMatch(host, "chatgpt.com")"#));
        assert!(pac.contains(r#"shExpMatch(host, "*.chatgpt.com")"#));
        assert!(pac.contains("return \"DIRECT\";"));
    }

    #[test]
    fn la_formula_del_servidor_decide_que_solicitud_coincide() {
        assert_eq!(decidir("www.google.com", &[]), Decision::Tunel);
        let regla = Regla {
            id: "g".into(),
            host: "www.google.com".into(),
            accion: "bloquear".into(),
            nombre: "Google".into(),
            redireccion_nombre: String::new(),
            redireccion_url: String::new(),
            receta: receta_google(),
        };
        assert_eq!(decidir("www.google.com", &[regla.clone()]), Decision::Formula);
        assert!(cumple_receta("/search?udm=50", &regla.receta));
        assert!(cumple_receta("/search?q=hola&udm%3D50", &regla.receta));
        assert!(!cumple_receta("/other?udm=50", &regla.receta));
        assert!(!cumple_receta("/search?q=hola", &regla.receta));
        assert!(!cumple_receta("/search?q=udm%3D50", &regla.receta));
        assert!(!cumple_receta("/search?xudm=50", &regla.receta));
        assert!(!cumple_receta("/search?udm=500", &regla.receta));
        assert!(pagina_bloqueo("chat.example").contains("no está permitido"));
    }

    #[test]
    fn una_version_desconocida_no_bloquea() {
        let mut receta = receta_google();
        receta.version = 2;
        assert!(!cumple_receta("/search?udm=50", &receta));
    }

    #[test]
    fn el_modo_web_bloquea_por_defecto_y_permitir_es_excepcion() {
        let sitios = vec![Sitio {
            host: "ia.example".into(), proveedor_id: "p".into(), endpoint_id: "e".into(), superficie: "web".into(), receta: Receta::default(),
        }];
        let (reglas, _) = resolver(&sitios, &[], &[], "pc", "pc", &[], "bloquear", &Sombra::default());
        assert_eq!(reglas.len(), 1);
        assert_eq!(reglas[0].id, "modo-web");
        let permitir = Politica { proveedor_id: "p".into(), ..politica("permitir", "tenant", "", "p") };
        let (reglas, _) = resolver(&sitios, &[permitir], &[], "pc", "pc", &[], "bloquear", &Sombra::default());
        assert!(reglas.is_empty());
        let pac = texto_pac(&[], &[], "", "bloquear");
        assert!(pac.contains(r#"url.substring(0,6) == "https:""#));
        let feed = vec![Sitio {
            host: "sombra.example".into(), proveedor_id: "feed:sombra.example".into(), endpoint_id: "feed:sombra.example".into(), superficie: "web".into(), receta: Receta::default(),
        }];
        let sombra = Sombra { accion: "bloquear".into(), ..Sombra::default() };
        let (reglas, _) = resolver(&feed, &[], &[], "pc", "pc", &[], "aceptar", &sombra);
        assert_eq!(reglas.len(), 1);
        assert_eq!(reglas[0].id, "shadow-ai");
        let (reglas, _) = resolver(&feed, &[], &[], "pc", "pc", &[], "bloquear", &Sombra::default());
        assert!(reglas.is_empty());
    }

    #[test]
    fn no_se_encadena_a_si_mismo() {
        assert_eq!(normalizar_proxy("http://127.0.0.1:8079"), None);
        assert_eq!(normalizar_proxy("10.0.0.8:8888").as_deref(), Some("10.0.0.8:8888"));
        assert_eq!(normalizar_proxy("socks5://10.0.0.8:1080"), None);
    }
}
