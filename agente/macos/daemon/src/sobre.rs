use ring::aead::{Aad, LessSafeKey, Nonce, UnboundKey, AES_256_GCM};
use ring::rand::{SecureRandom, SystemRandom};
use serde_json::Value;
use sha2::{Digest, Sha256};

const AAD: &[u8] = b"gobernanza";

pub fn cerrar(clave: &str, cuerpo: &Value) -> Result<Value, String> {
    let plano = serde_json::to_vec(cuerpo).map_err(|error| error.to_string())?;
    let mut nonce = [0u8; 12];
    SystemRandom::new().fill(&mut nonce).map_err(|_| "no hay aleatorio para cifrar".to_string())?;
    let sobre = sellar(clave, &plano, &nonce)?;
    Ok(serde_json::json!({ "sobre": sobre }))
}

fn sellar(clave: &str, plano: &[u8], nonce: &[u8; 12]) -> Result<String, String> {
    let suma = Sha256::digest(clave.as_bytes());
    let llave = LessSafeKey::new(UnboundKey::new(&AES_256_GCM, &suma).map_err(|_| "clave de cifrado inválida".to_string())?);
    let mut cuerpo = plano.to_vec();
    llave
        .seal_in_place_append_tag(Nonce::assume_unique_for_key(*nonce), Aad::from(AAD), &mut cuerpo)
        .map_err(|_| "no se pudo cifrar el cuerpo".to_string())?;
    let mut blob = Vec::with_capacity(1 + nonce.len() + cuerpo.len());
    blob.push(1);
    blob.extend_from_slice(nonce);
    blob.extend_from_slice(&cuerpo);
    Ok(base64url(&blob))
}

pub fn abrir(clave: &str, sobre: &str) -> Result<Value, String> {
    let blob = base64url_dec(sobre)?;
    if blob.len() < 1 + 12 + 16 || blob[0] != 1 {
        return Err("el sobre no es válido".into());
    }
    let mut nonce = [0u8; 12];
    nonce.copy_from_slice(&blob[1..13]);
    let mut cuerpo = blob[13..].to_vec();
    let suma = Sha256::digest(clave.as_bytes());
    let llave = LessSafeKey::new(UnboundKey::new(&AES_256_GCM, &suma).map_err(|_| "clave de cifrado inválida".to_string())?);
    let plano = llave
        .open_in_place(Nonce::assume_unique_for_key(nonce), Aad::from(AAD), &mut cuerpo)
        .map_err(|_| "no se pudo abrir el sobre".to_string())?;
    serde_json::from_slice(plano).map_err(|error| error.to_string())
}

fn base64url_dec(texto: &str) -> Result<Vec<u8>, String> {
    fn valor(byte: u8) -> Result<u8, String> {
        match byte {
            b'A'..=b'Z' => Ok(byte - b'A'),
            b'a'..=b'z' => Ok(byte - b'a' + 26),
            b'0'..=b'9' => Ok(byte - b'0' + 52),
            b'-' => Ok(62),
            b'_' => Ok(63),
            _ => Err("el sobre no es válido".into()),
        }
    }
    let datos = texto.as_bytes();
    let mut salida = Vec::with_capacity(datos.len() * 3 / 4);
    let mut i = 0;
    while i < datos.len() {
        let quedan = datos.len() - i;
        if quedan < 2 {
            return Err("el sobre no es válido".into());
        }
        let a = valor(datos[i])?;
        let b = valor(datos[i + 1])?;
        salida.push((a << 2) | (b >> 4));
        if quedan == 2 {
            break;
        }
        let c = valor(datos[i + 2])?;
        salida.push((b << 4) | (c >> 2));
        if quedan == 3 {
            break;
        }
        let d = valor(datos[i + 3])?;
        salida.push((c << 6) | d);
        i += 4;
    }
    Ok(salida)
}

fn base64url(datos: &[u8]) -> String {
    const ALFABETO: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut salida = String::new();
    let mut i = 0;
    while i + 3 <= datos.len() {
        let n = ((datos[i] as u32) << 16) | ((datos[i + 1] as u32) << 8) | datos[i + 2] as u32;
        salida.push(ALFABETO[((n >> 18) & 63) as usize] as char);
        salida.push(ALFABETO[((n >> 12) & 63) as usize] as char);
        salida.push(ALFABETO[((n >> 6) & 63) as usize] as char);
        salida.push(ALFABETO[(n & 63) as usize] as char);
        i += 3;
    }
    if i < datos.len() {
        let mut n = (datos[i] as u32) << 16;
        salida.push(ALFABETO[((n >> 18) & 63) as usize] as char);
        if i + 1 < datos.len() {
            n |= (datos[i + 1] as u32) << 8;
            salida.push(ALFABETO[((n >> 12) & 63) as usize] as char);
            salida.push(ALFABETO[((n >> 6) & 63) as usize] as char);
        } else {
            salida.push(ALFABETO[((n >> 12) & 63) as usize] as char);
        }
    }
    salida
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn coincide_con_el_servidor() {
        let sobre = sellar("gdv_vector", br#"{"contrato":1}"#, &[0x11; 12]).unwrap();
        assert_eq!(sobre, "AREREREREREREREREcnRQkCaKjPBiRW_LYX4Lz4SiSbSYj_bpvnsyZj7Zg");
    }

    #[test]
    fn abre_el_vector_del_servidor() {
        let plano = abrir("gdv_vector", "AREREREREREREREREcnRQkCaKjPBiRW_LYX4Lz4SiSbSYj_bpvnsyZj7Zg").unwrap();
        assert_eq!(plano.get("contrato").and_then(|valor| valor.as_i64()), Some(1));
    }

    #[test]
    fn el_sobre_no_lleva_el_json_en_claro() {
        let cuerpo = serde_json::json!({"contrato": 1, "hostname": "pc"});
        let cerrado = cerrar("gdv_vector", &cuerpo).unwrap();
        let sobre = cerrado.get("sobre").and_then(|valor| valor.as_str()).unwrap();
        assert!(!sobre.contains("hostname"));
        assert!(cerrado.get("contrato").is_none());
        assert!(sobre.len() > 20);
    }
}
