use serde_json::Value;

const ESQUEMAS: &[(&str, &str)] = &[
    ("enrolamiento", include_str!("../../../contrato/esquemas/enrolamiento.schema.json")),
    ("latido", include_str!("../../../contrato/esquemas/latido.schema.json")),
    ("programa", include_str!("../../../contrato/esquemas/programa.schema.json")),
    ("evento-ia", include_str!("../../../contrato/esquemas/evento-ia.schema.json")),
    ("resultado-bloqueo", include_str!("../../../contrato/esquemas/resultado-bloqueo.schema.json")),
    ("conexion", include_str!("../../../contrato/esquemas/conexion.schema.json")),
    ("consumo", include_str!("../../../contrato/esquemas/consumo.schema.json")),
];

pub fn validar_contrato(nombre: &str, cuerpo: &Value) -> Result<(), String> {
    let texto = ESQUEMAS.iter().find(|(clave, _)| *clave == nombre).map(|(_, texto)| *texto).ok_or_else(|| format!("contrato desconocido: {nombre}"))?;
    let esquema: Value = serde_json::from_str(texto).map_err(|error| error.to_string())?;
    validar(&esquema, cuerpo)
}

pub fn validar(esquema: &Value, cuerpo: &Value) -> Result<(), String> {
    let objeto = cuerpo.as_object().ok_or("el cuerpo debe ser un objeto")?;
    let props = esquema.get("properties").and_then(Value::as_object).ok_or("esquema sin properties")?;
    if esquema.get("additionalProperties").and_then(Value::as_bool) == Some(false) {
        for clave in objeto.keys() {
            if !props.contains_key(clave) {
                return Err(format!("campo no permitido: {clave}"));
            }
        }
    }
    if let Some(requeridos) = esquema.get("required").and_then(Value::as_array) {
        for item in requeridos {
            let clave = item.as_str().ok_or("required inválido")?;
            if !objeto.contains_key(clave) {
                return Err(format!("falta el campo {clave}"));
            }
        }
    }
    for (clave, valor) in objeto {
        let Some(prop) = props.get(clave) else { continue };
        if let Some(esperado) = prop.get("type").and_then(Value::as_str) {
            let ok = match esperado {
                "string" => valor.is_string(),
                "integer" => valor.as_i64().is_some() || valor.as_u64().is_some(),
                "number" => valor.is_number(),
                "boolean" => valor.is_boolean(),
                "object" => valor.is_object(),
                "array" => valor.is_array(),
                _ => true,
            };
            if !ok {
                return Err(format!("{clave} tiene un tipo distinto"));
            }
            if esperado == "object" {
                validar(prop, valor)?;
            }
        }
        if let Some(opciones) = prop.get("enum").and_then(Value::as_array) {
            if !opciones.iter().any(|opcion| opcion == valor) {
                return Err(format!("{clave} no está en el contrato"));
            }
        }
        if let Some(constante) = prop.get("const") {
            if constante != valor {
                return Err(format!("{clave} no está en el contrato"));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use serde_json::json;

    #[test]
    fn el_latido_embebido_exige_contrato() {
        let cuerpo = json!({
            "contrato": 1,
            "versionAgente": "0.1.0",
            "cpu": "4",
            "ramMb": 1024,
            "usuarioSesion": "ana"
        });
        assert!(validar_contrato("latido", &cuerpo).is_ok());
        let mut incompleto = cuerpo;
        incompleto.as_object_mut().unwrap().remove("cpu");
        assert!(validar_contrato("latido", &incompleto).is_err());
    }
}
