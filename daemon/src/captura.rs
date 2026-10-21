use serde_json::Value;

const MAX_TEXTO: usize = 32_000;

pub fn extraer_entrada(cuerpo: &[u8]) -> (String, String) {
    let cuerpo = if cuerpo.len() > 1_048_576 { &cuerpo[..1_048_576] } else { cuerpo };
    let Ok(carga) = serde_json::from_slice::<Value>(cuerpo) else { return (String::new(), String::new()) };
    let modelo = carga.get("model").and_then(Value::as_str).unwrap_or("").to_string();
    if let Some(mensajes) = carga.get("messages").and_then(Value::as_array) {
        for mensaje in mensajes.iter().rev() {
            if !es_usuario(mensaje) {
                continue;
            }
            let texto = texto_contenido(mensaje.get("content"));
            if !texto.is_empty() {
                return (limitar(&texto), modelo);
            }
        }
    }
    if let Some(texto) = carga.get("input").and_then(Value::as_str) {
        let texto = texto.trim();
        if !texto.is_empty() {
            return (limitar(texto), modelo);
        }
    }
    if let Some(lista) = carga.get("input").and_then(Value::as_array) {
        for mensaje in lista.iter().rev() {
            if !es_usuario(mensaje) {
                continue;
            }
            let texto = texto_contenido(mensaje.get("content"));
            if !texto.is_empty() {
                return (limitar(&texto), modelo);
            }
        }
    }
    (String::new(), modelo)
}

fn es_usuario(mensaje: &Value) -> bool {
    let rol = rol_de(mensaje);
    rol.is_empty() || rol == "user"
}

fn rol_de(mensaje: &Value) -> String {
    if let Some(rol) = mensaje.get("role").and_then(Value::as_str) {
        return rol.to_string();
    }
    mensaje.pointer("/author/role").and_then(Value::as_str).unwrap_or("").to_string()
}

fn texto_contenido(valor: Option<&Value>) -> String {
    let Some(valor) = valor else { return String::new() };
    match valor {
        Value::String(texto) => texto.trim().to_string(),
        Value::Object(mapa) => {
            if let Some(partes) = mapa.get("parts").and_then(Value::as_array) {
                return unir_textos(partes);
            }
            mapa.get("text").and_then(Value::as_str).unwrap_or("").trim().to_string()
        }
        Value::Array(partes) => unir_textos(partes),
        _ => String::new(),
    }
}

fn unir_textos(partes: &[Value]) -> String {
    let mut textos = Vec::new();
    for parte in partes {
        match parte {
            Value::String(texto) if !texto.trim().is_empty() => textos.push(texto.clone()),
            Value::Object(mapa) => {
                if let Some(texto) = mapa.get("text").and_then(Value::as_str) {
                    if !texto.trim().is_empty() {
                        textos.push(texto.to_string());
                    }
                }
            }
            _ => {}
        }
    }
    textos.join("\n")
}

fn limitar(texto: &str) -> String {
    if texto.len() > MAX_TEXTO { texto[..MAX_TEXTO].to_string() } else { texto.to_string() }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn toma_el_ultimo_mensaje_del_usuario() {
        let cuerpo = br#"{"model":"auto","messages":[{"role":"user","content":"hola"},{"role":"assistant","content":"respuesta"},{"role":"user","content":[{"text":"segunda"}]}]}"#;
        let (texto, modelo) = extraer_entrada(cuerpo);
        assert_eq!(texto, "segunda");
        assert_eq!(modelo, "auto");
    }
}
