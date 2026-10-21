# Contrato del agente

Versión **1**. Los tres sistemas envían estos JSON. El API los valida con los mismos archivos de `esquemas/`.

| Archivo | Uso |
| --- | --- |
| `enrolamiento.schema.json` | Alta que pide el agente, indicando el tenant al que pertenece |
| `latido.schema.json` | Tarjeta: versión, CPU, RAM, usuario de sesión |
| `conexion.schema.json` | Metadato de conexión. Sin contenido |
| `evento-ia.schema.json` | Consulta, modelo, tokens, costo, archivo |
| `politica.schema.json` | Permitir, bloquear o redirigir |
| `aviso.schema.json` | Texto que publica el tenant |
| `proceso.schema.json` | Proceso y binario (el inventario de cada SO es posterior) |
| `resultado-bloqueo.schema.json` | Resultado cuando una política se aplique |
| `usuario.schema.json` y `tenant.schema.json` | Cuentas del panel |

`ejemplos/` trae un cuerpo válido de cada uno.

Autenticación del equipo, después del alta:

- `Authorization: Bearer gdv_…`, o
- huella SHA-256 del certificado de cliente, cuando un terminador mTLS ya la verificó.

Si el latido o la inspección fallan, la red del equipo sigue. Una app que rechace el certificado se registrará como `inspeccion_rechazada`. No se evade el pinning.

El bloqueo, la CA, el proxy y la captura de tráfico no forman parte de este contrato ejecutable todavía: los esquemas dejan el hueco para no divergir.
