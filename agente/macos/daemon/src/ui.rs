pub const SERVIDOR_PRINCIPAL: &str = "http://192.168.20.253:7590";

pub fn es_loopback(url: &str) -> bool {
    let texto = url.trim().to_ascii_lowercase();
    texto.contains("://127.0.0.1") || texto.contains("://localhost") || texto.contains("://[::1]") || texto.contains("://0.0.0.0")
}

pub fn url_visible(direccion: &str, flags: &str) -> String {
    for candidato in [direccion, flags] {
        let limpio = candidato.trim().trim_end_matches('/');
        if !limpio.is_empty() && !es_loopback(limpio) {
            return limpio.to_string();
        }
    }
    SERVIDOR_PRINCIPAL.to_string()
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_loopback_no_se_ofrece_como_servidor() {
        assert!(es_loopback("http://127.0.0.1:7590"));
        assert_eq!(url_visible("http://127.0.0.1:7590", ""), SERVIDOR_PRINCIPAL);
        assert_eq!(url_visible("http://10.0.0.8:7590", ""), "http://10.0.0.8:7590");
    }
}
