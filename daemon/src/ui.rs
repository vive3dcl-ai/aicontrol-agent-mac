pub const ETIQUETA_GLOBAL: &str = "AIcontrol";

pub fn es_loopback(url: &str) -> bool {
    let texto = url.trim().to_ascii_lowercase();
    texto.contains("://127.0.0.1") || texto.contains("://localhost") || texto.contains("://[::1]") || texto.contains("://0.0.0.0")
}

pub fn url_visible(_direccion: &str, _flags: &str) -> String {
    ETIQUETA_GLOBAL.to_string()
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_conexion_global_no_muestra_la_direccion() {
        assert!(es_loopback("http://127.0.0.1:7590"));
        assert_eq!(url_visible("http://127.0.0.1:7590", ""), ETIQUETA_GLOBAL);
        assert_eq!(url_visible("http://10.0.0.8:7590", ""), ETIQUETA_GLOBAL);
        assert!(!url_visible("https://aai.latinrouter.ai", "").contains("latinrouter"));
    }
}
