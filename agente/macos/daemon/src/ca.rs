use rcgen::{
    BasicConstraints, CertificateParams, CertificateRevocationListParams, CrlDistributionPoint,
    DnType, IsCa, KeyIdMethod, KeyPair, KeyUsagePurpose, SerialNumber,
};
use std::fs;
use std::path::Path;

const FORMATO: &str = "crl-v1";
const ARCHIVO_FORMATO: &str = "formato.txt";
const ARCHIVO_REINSTALAR: &str = "reinstalar-ca";
const URL_CRL: &str = "http://127.0.0.1:4765/gobernanza.crl";

pub struct Autoridad {
    cert: rcgen::Certificate,
    key: KeyPair,
    pem: String,
    der: Vec<u8>,
}

impl Autoridad {
    pub fn generar() -> Result<Self, String> {
        let params = parametros_ca();
        let key = KeyPair::generate().map_err(|error| error.to_string())?;
        let cert = params.self_signed(&key).map_err(|error| error.to_string())?;
        Ok(Self { pem: cert.pem(), der: cert.der().to_vec(), cert, key })
    }

    #[allow(dead_code)]
    pub fn pem(&self) -> String {
        self.pem.clone()
    }

    pub fn guardar(&self, dir: &Path) -> Result<(), String> {
        fs::create_dir_all(dir).map_err(|error| error.to_string())?;
        fs::write(dir.join("gobernanza-ca.crt"), &self.pem).map_err(|error| error.to_string())?;
        fs::write(dir.join("gobernanza-ca.der"), &self.der).map_err(|error| error.to_string())?;
        fs::write(dir.join("gobernanza-ca.key"), self.key.serialize_pem()).map_err(|error| error.to_string())?;
        fs::write(dir.join("gobernanza.crl"), self.crl()?).map_err(|error| error.to_string())?;
        fs::write(dir.join(ARCHIVO_FORMATO), FORMATO).map_err(|error| error.to_string())?;
        fs::write(dir.join(ARCHIVO_REINSTALAR), b"").map_err(|error| error.to_string())?;
        Ok(())
    }

    pub fn hoja(&self, host: &str) -> Result<Hoja, String> {
        let mut params = CertificateParams::new(vec![host.to_string()]).map_err(|error| error.to_string())?;
        params.distinguished_name.push(DnType::CommonName, host);
        params.crl_distribution_points = vec![CrlDistributionPoint { uris: vec![URL_CRL.to_string()] }];
        let key = KeyPair::generate().map_err(|error| error.to_string())?;
        let cert = params.signed_by(&key, &self.cert, &self.key).map_err(|error| error.to_string())?;
        Ok(Hoja { cadena: vec![cert.der().to_vec(), self.der.clone()], clave: key.serialize_der() })
    }

    fn crl(&self) -> Result<Vec<u8>, String> {
        let params = CertificateRevocationListParams {
            this_update: rcgen::date_time_ymd(2024, 1, 1),
            next_update: rcgen::date_time_ymd(2035, 1, 1),
            crl_number: SerialNumber::from(1u64),
            issuing_distribution_point: None,
            revoked_certs: Vec::new(),
            key_identifier_method: KeyIdMethod::Sha256,
        };
        let crl = params.signed_by(&self.cert, &self.key).map_err(|error| error.to_string())?;
        Ok(crl.der().to_vec())
    }
}

pub struct Hoja {
    pub cadena: Vec<Vec<u8>>,
    pub clave: Vec<u8>,
}

fn parametros_ca() -> CertificateParams {
    let mut params = CertificateParams::default();
    params.not_before = rcgen::date_time_ymd(2024, 1, 1);
    params.not_after = rcgen::date_time_ymd(2035, 1, 1);
    params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    params.distinguished_name.push(DnType::CommonName, "Gobernanza 2");
    params.key_usages = vec![KeyUsagePurpose::KeyCertSign, KeyUsagePurpose::CrlSign, KeyUsagePurpose::DigitalSignature];
    params
}

pub fn cargar_o_crear(dir: &Path) -> Result<Autoridad, String> {
    let crt = dir.join("gobernanza-ca.crt");
    let clave = dir.join("gobernanza-ca.key");
    let formato = fs::read_to_string(dir.join(ARCHIVO_FORMATO)).unwrap_or_default();
    if formato.trim() == FORMATO && crt.is_file() && clave.is_file() {
        let pem = fs::read_to_string(&crt).map_err(|error| error.to_string())?;
        let der = der_de_pem(&pem)?;
        if aceptable_en_windows(&der) {
            let key_pem = fs::read_to_string(&clave).map_err(|error| error.to_string())?;
            let key = KeyPair::from_pem(&key_pem).map_err(|error| error.to_string())?;
            let cert = parametros_ca().self_signed(&key).map_err(|error| error.to_string())?;
            if !dir.join("gobernanza-ca.der").is_file() {
                let _ = fs::write(dir.join("gobernanza-ca.der"), &der);
            }
            let autoridad = Autoridad { cert, key, pem, der };
            if !dir.join("gobernanza.crl").is_file() {
                fs::write(dir.join("gobernanza.crl"), autoridad.crl()?).map_err(|error| error.to_string())?;
            }
            return Ok(autoridad);
        }
        let _ = fs::remove_file(&crt);
        let _ = fs::remove_file(&clave);
        let _ = fs::remove_file(dir.join("gobernanza-ca.der"));
    }
    let autoridad = Autoridad::generar()?;
    autoridad.guardar(dir)?;
    Ok(autoridad)
}

pub fn requiere_instalacion(dir: &Path) -> bool {
    dir.join(ARCHIVO_REINSTALAR).is_file()
}

pub fn marcar_instalada(dir: &Path) {
    let _ = fs::remove_file(dir.join(ARCHIVO_REINSTALAR));
}

fn aceptable_en_windows(der: &[u8]) -> bool {
    let texto = String::from_utf8_lossy(der);
    !texto.contains("40960101000000Z") && !texto.contains("750101000000Z")
}

fn der_de_pem(pem: &str) -> Result<Vec<u8>, String> {
    let b64: String = pem.lines().filter(|linea| !linea.starts_with("-----")).collect();
    base64_decode(&b64)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_recarga_conserva_el_certificado_guardado() {
        let dir = std::env::temp_dir().join(format!("gob-ca-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let primera = cargar_o_crear(&dir).unwrap();
        let der = primera.der.clone();
        drop(primera);
        let segunda = cargar_o_crear(&dir).unwrap();
        assert_eq!(segunda.der, der);
        assert!(aceptable_en_windows(&segunda.der));
        assert!(segunda.hoja("chatgpt.com").is_ok());
        assert!(dir.join("gobernanza.crl").is_file());
        assert!(requiere_instalacion(&dir));
        marcar_instalada(&dir);
        assert!(!requiere_instalacion(&dir));
        let _ = std::fs::remove_dir_all(&dir);
    }
}

fn base64_decode(texto: &str) -> Result<Vec<u8>, String> {
    const TABLA: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut salida = Vec::new();
    let mut buf = 0u32;
    let mut bits = 0;
    for byte in texto.bytes() {
        if byte.is_ascii_whitespace() { continue; }
        if byte == b'=' { break; }
        let valor = TABLA.iter().position(|item| *item == byte).ok_or("certificado PEM inválido")? as u32;
        buf = (buf << 6) | valor;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            salida.push((buf >> bits) as u8);
            buf &= (1 << bits) - 1;
        }
    }
    Ok(salida)
}
