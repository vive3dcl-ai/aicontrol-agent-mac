const MARCA: &[u8] = b"GOBMAC01";

pub fn empaquetar(archivos: &[(String, Vec<u8>)]) -> Result<Vec<u8>, String> {
    if archivos.is_empty() {
        return Err("el paquete no trae AIcontrol.app".into());
    }
    if !archivos.iter().any(|(nombre, _)| nombre.starts_with("AIcontrol.app/")) {
        return Err("el paquete no trae AIcontrol.app".into());
    }
    let mut locales = Vec::new();
    let mut central = Vec::new();
    let mut entradas = 0u16;
    for (nombre, datos) in archivos {
        validar_nombre(nombre)?;
        if entradas == u16::MAX {
            return Err("el paquete trae demasiados archivos".into());
        }
        let crc = crc32(datos);
        let offset = locales.len() as u32;
        escribir_local(&mut locales, nombre, datos, crc);
        escribir_central(&mut central, nombre, datos, crc, offset);
        entradas += 1;
    }
    let inicio_central = locales.len() as u32;
    let mut zip = locales;
    zip.extend_from_slice(&central);
    escribir_eocd(&mut zip, entradas, central.len() as u32, inicio_central);
    let largo = u64::try_from(zip.len()).map_err(|_| "el paquete es demasiado grande".to_string())?;
    zip.extend_from_slice(&largo.to_le_bytes());
    zip.extend_from_slice(MARCA);
    Ok(zip)
}

pub fn leer(bytes: &[u8]) -> Result<Vec<(String, Vec<u8>)>, String> {
    if bytes.len() < 16 || &bytes[bytes.len() - MARCA.len()..] != MARCA {
        return Err("no está el paquete de instalación".into());
    }
    let mut largo_bytes = [0u8; 8];
    let corte = bytes.len() - 16;
    largo_bytes.copy_from_slice(&bytes[corte..bytes.len() - 8]);
    let largo = usize::try_from(u64::from_le_bytes(largo_bytes)).map_err(|_| "el paquete de instalación está incompleto".to_string())?;
    if largo == 0 || largo > corte {
        return Err("el paquete de instalación está incompleto".into());
    }
    let zip = &bytes[corte - largo..corte];
    leer_zip(zip)
}

fn validar_nombre(nombre: &str) -> Result<(), String> {
    if nombre.is_empty()
        || nombre.starts_with('/')
        || nombre.contains('\\')
        || nombre.split('/').any(|parte| parte.is_empty() || parte == "." || parte == "..")
    {
        return Err("el paquete de instalación trae un archivo no reconocido".into());
    }
    Ok(())
}

fn escribir_local(salida: &mut Vec<u8>, nombre: &str, datos: &[u8], crc: u32) {
    salida.extend_from_slice(&0x04034b50u32.to_le_bytes());
    salida.extend_from_slice(&20u16.to_le_bytes());
    salida.extend_from_slice(&0u16.to_le_bytes());
    salida.extend_from_slice(&0u16.to_le_bytes());
    salida.extend_from_slice(&0u16.to_le_bytes());
    salida.extend_from_slice(&0u16.to_le_bytes());
    salida.extend_from_slice(&crc.to_le_bytes());
    let tamano = datos.len() as u32;
    salida.extend_from_slice(&tamano.to_le_bytes());
    salida.extend_from_slice(&tamano.to_le_bytes());
    salida.extend_from_slice(&(nombre.len() as u16).to_le_bytes());
    salida.extend_from_slice(&0u16.to_le_bytes());
    salida.extend_from_slice(nombre.as_bytes());
    salida.extend_from_slice(datos);
}

fn escribir_central(salida: &mut Vec<u8>, nombre: &str, datos: &[u8], crc: u32, offset: u32) {
    salida.extend_from_slice(&0x02014b50u32.to_le_bytes());
    salida.extend_from_slice(&20u16.to_le_bytes());
    salida.extend_from_slice(&20u16.to_le_bytes());
    salida.extend_from_slice(&0u16.to_le_bytes());
    salida.extend_from_slice(&0u16.to_le_bytes());
    salida.extend_from_slice(&0u16.to_le_bytes());
    salida.extend_from_slice(&0u16.to_le_bytes());
    salida.extend_from_slice(&crc.to_le_bytes());
    let tamano = datos.len() as u32;
    salida.extend_from_slice(&tamano.to_le_bytes());
    salida.extend_from_slice(&tamano.to_le_bytes());
    salida.extend_from_slice(&(nombre.len() as u16).to_le_bytes());
    salida.extend_from_slice(&0u16.to_le_bytes());
    salida.extend_from_slice(&0u16.to_le_bytes());
    salida.extend_from_slice(&0u16.to_le_bytes());
    salida.extend_from_slice(&0u16.to_le_bytes());
    salida.extend_from_slice(&0u32.to_le_bytes());
    salida.extend_from_slice(&offset.to_le_bytes());
    salida.extend_from_slice(nombre.as_bytes());
}

fn escribir_eocd(salida: &mut Vec<u8>, entradas: u16, central: u32, offset: u32) {
    salida.extend_from_slice(&0x06054b50u32.to_le_bytes());
    salida.extend_from_slice(&0u16.to_le_bytes());
    salida.extend_from_slice(&0u16.to_le_bytes());
    salida.extend_from_slice(&entradas.to_le_bytes());
    salida.extend_from_slice(&entradas.to_le_bytes());
    salida.extend_from_slice(&central.to_le_bytes());
    salida.extend_from_slice(&offset.to_le_bytes());
    salida.extend_from_slice(&0u16.to_le_bytes());
}

fn leer_zip(zip: &[u8]) -> Result<Vec<(String, Vec<u8>)>, String> {
    if zip.len() < 22 {
        return Err("el paquete de instalación está incompleto".into());
    }
    let eocd = zip.windows(4).rposition(|marca| marca == 0x06054b50u32.to_le_bytes()).ok_or("el paquete de instalación está incompleto")?;
    if zip.len() < eocd + 22 {
        return Err("el paquete de instalación está incompleto".into());
    }
    let entradas = u16::from_le_bytes([zip[eocd + 10], zip[eocd + 11]]) as usize;
    let central_len = u32::from_le_bytes(zip[eocd + 12..eocd + 16].try_into().unwrap()) as usize;
    let central_off = u32::from_le_bytes(zip[eocd + 16..eocd + 20].try_into().unwrap()) as usize;
    if central_off.saturating_add(central_len) > zip.len() {
        return Err("el paquete de instalación está incompleto".into());
    }
    let mut cursor = central_off;
    let fin = central_off + central_len;
    let mut salida = Vec::with_capacity(entradas);
    for _ in 0..entradas {
        if cursor + 46 > fin {
            return Err("el paquete de instalación está incompleto".into());
        }
        if zip[cursor..cursor + 4] != 0x02014b50u32.to_le_bytes() {
            return Err("el paquete de instalación está dañado".into());
        }
        let metodo = u16::from_le_bytes([zip[cursor + 10], zip[cursor + 11]]);
        if metodo != 0 {
            return Err("el paquete de instalación está dañado".into());
        }
        let crc = u32::from_le_bytes(zip[cursor + 16..cursor + 20].try_into().unwrap());
        let tamano = u32::from_le_bytes(zip[cursor + 20..cursor + 24].try_into().unwrap()) as usize;
        let nombre_len = u16::from_le_bytes([zip[cursor + 28], zip[cursor + 29]]) as usize;
        let extra = u16::from_le_bytes([zip[cursor + 30], zip[cursor + 31]]) as usize;
        let comentario = u16::from_le_bytes([zip[cursor + 32], zip[cursor + 33]]) as usize;
        let offset = u32::from_le_bytes(zip[cursor + 42..cursor + 46].try_into().unwrap()) as usize;
        if cursor + 46 + nombre_len > fin {
            return Err("el paquete de instalación está incompleto".into());
        }
        let nombre = std::str::from_utf8(&zip[cursor + 46..cursor + 46 + nombre_len]).map_err(|_| "el paquete de instalación está dañado".to_string())?;
        validar_nombre(nombre)?;
        let datos = leer_local(zip, offset, nombre, tamano, crc)?;
        salida.push((nombre.to_string(), datos));
        cursor += 46 + nombre_len + extra + comentario;
    }
    if salida.is_empty() || !salida.iter().any(|(nombre, _)| nombre.starts_with("AIcontrol.app/")) {
        return Err("el paquete no trae AIcontrol.app".into());
    }
    Ok(salida)
}

fn leer_local(zip: &[u8], offset: usize, nombre: &str, tamano: usize, crc: u32) -> Result<Vec<u8>, String> {
    if offset + 30 > zip.len() || zip[offset..offset + 4] != 0x04034b50u32.to_le_bytes() {
        return Err("el paquete de instalación está incompleto".into());
    }
    let nombre_len = u16::from_le_bytes([zip[offset + 26], zip[offset + 27]]) as usize;
    let extra = u16::from_le_bytes([zip[offset + 28], zip[offset + 29]]) as usize;
    let inicio = offset + 30 + nombre_len + extra;
    let fin = inicio.checked_add(tamano).ok_or("el paquete de instalación está incompleto")?;
    if fin > zip.len() {
        return Err("el paquete de instalación está incompleto".into());
    }
    let local = std::str::from_utf8(&zip[offset + 30..offset + 30 + nombre_len]).unwrap_or("");
    if local != nombre {
        return Err("el paquete de instalación está dañado".into());
    }
    let datos = zip[inicio..fin].to_vec();
    if crc32(&datos) != crc {
        return Err("el paquete de instalación está dañado".into());
    }
    Ok(datos)
}

fn crc32(datos: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for byte in datos {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            let mascara = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xedb8_8320 & mascara);
        }
    }
    !crc
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_zip_vuelve_con_la_cola_gobmac01() {
        let paquete = empaquetar(&[
            ("AIcontrol.app/Contents/MacOS/AIcontrol-daemon".into(), b"daemon".to_vec()),
            ("AIcontrol.app/Contents/Info.plist".into(), b"<plist/>".to_vec()),
        ])
        .unwrap();
        assert!(paquete.ends_with(MARCA));
        let archivos = leer(&paquete).unwrap();
        assert_eq!(archivos.len(), 2);
        assert_eq!(archivos[0].1, b"daemon");
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        assert!(empaquetar(&[("otro.txt".into(), b"x".to_vec())]).is_err());
        assert!(empaquetar(&[("AIcontrol.app/../secreto".into(), b"x".to_vec())]).is_err());
    }
}
