# De Windows a macOS

El agente de Windows 0.1.10 es la especificación. Este texto dice cómo se comporta y qué hace el agente de Mac en cada punto. El crate de Windows no se modifica.

El agente de Mac no usa la extensión de red. El navegador manda el tráfico con un PAC, el daemon hace de proxy en `127.0.0.1:8079` y la CA queda en el llavero del sistema. Es el mismo modelo que WinINET, el PAC y el almacén ROOT de Windows.

## Procesos

En Windows hay dos programas. `AIcontrol-servicio.exe` es el servicio `GobernanzaAgente`, cuenta LocalSystem, arranque automático. No acepta detenerse desde services.msc. `AIcontrol-bandeja.exe` es el icono de la sesión. Si falta, el servicio lo vuelve a crear cada 3 segundos. Un administrador puede matar el proceso desde el Administrador de tareas.

En Mac el daemon es `AIcontrol.app/Contents/MacOS/AIcontrol-daemon`. Lo levanta el LaunchDaemon `local.itsec.aicontrol`, como root, con `KeepAlive`. No hay un botón de parar para el usuario. Root puede bajarlo con `launchctl`, del mismo modo que el administrador de Windows puede finalizar el proceso. La barra de menú es `AIcontrol.app/Contents/MacOS/AIcontrol`, un `LSUIElement`. La carga el LaunchAgent `local.itsec.aicontrol.menu` y, si falta, el daemon la abre otra vez cada 3 segundos.

La carpeta de datos sigue llamándose Gobernanza para no perder la CA ni la configuración en una actualización en el sitio.

| Windows | macOS |
| --- | --- |
| `C:\Program Files\ITsec\Gobernanza` | `/Applications/AIcontrol.app` |
| `%ProgramData%\Gobernanza` | `/Library/Application Support/Gobernanza` |
| Servicio `GobernanzaAgente` | LaunchDaemon `local.itsec.aicontrol` |
| Bandeja por sesión | LaunchAgent `local.itsec.aicontrol.menu` |

## Conexión

No hay WebSocket de control. Cada mensaje es un POST HTTP/1.1 con `Connection: close`. Si la URL es `https`, el cliente usa TLS 1.2 y las raíces del sistema. No presenta certificado de cliente.

El alta es `POST /v1/dispositivos/enrolar`, sin Bearer y sin sobre. El cuerpo lleva `contrato: 1`, `tenantSlug`, `hostname`, `sistema`, `versionSo`, `versionAgente`, `cpu`, `ramMb`, `usuarioSesion` y, si hace falta, `clave` y `machineId`. `sistema` es `macos`. El esquema no acepta `darwin`. `versionSo` es la versión de `sw_vers`, por ejemplo `15.6`. `machineId` es el `IOPlatformUUID`, de 8 a 128 caracteres `[A-Za-z0-9_-]`. `cpu` es `N núcleos`. `ramMb` sale de `hw.memsize`. El usuario es el de la consola (`/dev/console`). Se descartan `root`, `SYSTEM` y las cuentas que terminan en `$`. Si no hay persona, el valor es `usuario`.

La respuesta trae `token` (empieza por `gdv_`) y puede traer `claveDefinitiva`. El token queda en `dispositivo.token`.

El enlace de la cuenta es otro paso: `POST /v1/dispositivos/activar`, con el inventario, `correo` en minúsculas y `clave`. Si ya hay token, ese POST va cifrado.

El latido es `POST /v1/dispositivos/latido` cada 60 segundos (mínimo 5). Se quita `sistema` del inventario. Se agregan `estadoExtension: no_instalada`, `estadoCa` (`vigente` o `no_instalada`), `renuevaTokens: true` y, si hay un paquete en curso, `estadoActualizacion` y `versionPendiente`.

Si el POST autenticado responde 401 y hay clave definitiva, `POST /v1/dispositivos/renovar` va en claro, sin Bearer. El cuerpo es `{ machineId, sobre }`. El sobre cifra `{ contrato: 1, machineId }`. La respuesta trae `renovacion`, otro sobre, con `token` y `claveDefinitiva`, ambos con prefijo `gdv_`.

Si el latido falla, el proxy no se apaga: sigue con `estado.json`. Un 5xx, un fallo de conexión o una respuesta HTTP incompleta esperan 10 segundos. El resto espera `intervalo * min(fallos, 4)`, con tope de 120 segundos. El éxito deja `conexion` en `estable`.

Otros POST, con sobre en cuanto existe la clave definitiva:

- cada 5 segundos, si hay enlace y token: `POST /v1/ingesta/consumo`
- al ver un programa: `POST /v1/ingesta/programas`
- al bloquear: `POST /v1/ingesta/bloqueos`
- al capturar: `POST /v1/ingesta/eventos-ia` y `POST /v1/ingesta/conexiones`
- al desinstalar: `POST /v1/dispositivos/desinstalar`, hasta 3 veces, cada 500 ms
- la clave de ajustes: `POST /v1/dispositivos/ajustes/autorizar-local`

Cada 10 segundos el daemon vuelve a instalar la CA si falta y vuelve a escribir el PAC y las políticas.

## Sobre

AES-256-GCM. La clave son los 32 bytes del SHA-256 de la clave definitiva en UTF-8. El nonce es de 12 bytes, al azar. El AAD es `gobernanza`. El blob es el byte `0x01`, el nonce, el texto cifrado y el tag de 16 bytes. Se manda en Base64URL sin relleno, dentro de `{ "sobre": "..." }`. El alta no lleva sobre, porque todavía no hay clave.

El vector de prueba, con clave `gdv_vector`, plano `{"contrato":1}` y nonce de doce bytes `0x11`, es `AREREREREREREREREcnRQkCaKjPBiRW_LYX4Lz4SiSbSYj_bpvnsyZj7Zg`. Es el mismo que el API y que el agente de Windows.

## Configuración en disco

`config.dat` empieza con la magia `GOBCFG01`. El resto es AES-256-GCM. En Windows la clave la guarda DPAPI de equipo. En Mac la clave de 32 bytes vive en el llavero del sistema, ítem `GobernanzaAgenteConfig`, cuenta `aicontrol`. Solo el daemon root la usa. La barra de menú no abre `config.dat`: habla con `http://127.0.0.1:4765`.

Si el llavero no está (una prueba en Linux, o un daemon sin root), la misma clave cae en `config.key`, modo `0600`. El archivo `config.dat` no lleva el token ni la URL en claro.

`estado.json` guarda la conexión, el servidor elegido, la cuenta, las reglas, los programas y el modo web. Va en claro, igual que en Windows.

## Proxy y PAC

El proxy escucha en `127.0.0.1:8079`. Solo acepta `CONNECT`. Otro método responde 405. El nombre es lo que está antes del puerto.

La decisión sale de las reglas ya resueltas:

- `Tunel`: no hay regla y el host no es de IA. El CONNECT se copia sin mirar el TLS.
- `Aviso`: hay regla y la receta es «cualquiera». Se abre el TLS con una hoja de la CA y se responde el HTML de bloqueo o de redirección.
- `Formula`: hay regla con ruta o query. Solo se bloquea si la URL cumple la receta. Si no, la petición se reenvía.
- `Mitm`: no hay regla y el host es `chatgpt.com`, `*.chatgpt.com`, `openai.com` o `*.openai.com`. Se lee el primer POST y se extrae el último mensaje del usuario.

La hoja dura lo que dura la conexión. El CN y el SAN son el host. El punto de distribución de la CRL es `http://127.0.0.1:4765/gobernanza.crl`. El ALPN del proxy es solo `http/1.1`. Si no se puede firmar la hoja, esa conexión sigue en túnel. Si el navegador no confía en la CA, se suelta la conexión. El resultado que se informa es `aplicado`. No se manda `inspeccion_rechazada` ni `omitido_fail_open`: el agente de Windows tampoco lo hace.

La captura lee el cuerpo solo si hay `Content-Length`, con tope de 1 048 576 bytes. El texto que se guarda tiene tope de 32 000. No se interpretan WebSocket, HTTP/2 ni cuerpos fragmentados. El resto del túnel se copia.

La tarjeta `127.0.0.1:4765` sirve el PAC, la CRL, `estado.json` y las acciones de la ventana. El PAC siempre incluye ChatGPT y OpenAI, más los sitios y los hosts de las reglas. `localhost`, `127.*`, `::1` y los nombres sin punto salen `DIRECT`. El resto listado va a `PROXY 127.0.0.1:8079`. Si `modoWeb` es `bloquear`, todo `https:` entra al proxy. Lo que no está en la lista sale `DIRECT`, o por el `HTTPS_PROXY` / `HTTP_PROXY` que ya hubiera y que no sea el propio `127.0.0.1:8079`.

La CA se llama `Gobernanza 2`. Es CA, firma certificados y CRL, y vale del 2024-01-01 al 2035-01-01. Los archivos están en `ca/`: `gobernanza-ca.crt`, `.der`, `.key`, `gobernanza.crl` y `formato.txt` con `crl-v1`. En Mac se instala en el llavero del sistema como raíz de confianza (`security add-trusted-cert`).

## Navegadores

En Windows el servicio escribe políticas en `HKLM\Software\Policies` y la bandeja escribe el PAC de WinINET. En Mac las dos cosas las hace el daemon, porque es el único que corre como root.

Chromium (Chrome, Edge, Brave, Opera, Opera GX, Vivaldi, Chromium y Yandex) recibe un plist en `/Library/Managed Preferences/<bundle>.plist` con `ProxyMode=pac_script`, `ProxyPacUrl` apuntando al PAC, `ProxyPacMandatory` y `QuicAllowed` en falso. Además, si el agente tiene que reabrir el navegador, lo hace con `--disable-quic --proxy-pac-url=... --restore-last-session`.

Firefox, LibreWolf, Waterfox y Floorp reciben `policies.json` dentro de `distribution/` de la aplicación, y el bloque marcado en el `user.js` de cada perfil:

```
// gobernanza-proxy inicio
user_pref("network.proxy.type", 2);
user_pref("network.proxy.autoconfig_url", "http://127.0.0.1:4765/proxy.pac");
user_pref("security.enterprise_roots.enabled", true);
user_pref("network.http.http3.enable", false);
// gobernanza-proxy fin
```

Safari y el resto de las apps que respetan el proxy del sistema usan `networksetup -setautoproxyurl` en cada servicio de red. Eso es el equivalente del AutoConfigURL de WinINET. Lo que el PAC manda `DIRECT` no se queda colgado en el proxy.

## Reglas, programas y consumo

Las reglas llegan en el `paquete` del latido y solo se aplican si la cuenta está enlazada. La prioridad es equipo 40, usuario 30, grupo 20, tenant 10, y gana el destino más específico. `permitir`, o `redirigir` sin URL, no genera regla. `modoWeb=bloquear` sobre un sitio web sin política mejor crea la regla `modo-web`.

Una receta de versión mayor que 1 no coincide. La ruta puede ser `cualquiera`, `igual` o `prefijo`. La query puede ser `existe` o `igual`, con el valor ya decodificado.

Los programas se miran cada segundo. El nombre es el del ejecutable, o el del script si el proceso es python, node, java, ruby, perl, deno o bun. `bloquear` espera 3 segundos, cierra el proceso y hasta 64 descendientes, publica una vez y muestra un aviso 4 segundos. El mismo programa no vuelve a avisar hasta pasados 120 segundos. El propio daemon y la barra de menú no se vigilan. Sin cuenta enlazada no hay vigilancia.

El consumo es el CPU y la RAM del proceso del daemon, no los de la barra de menú.

## Barra de menú

La ventana sigue los mismos pasos que la bandeja de Windows. Habla solo con la tarjeta.

1. Servidor. La opción global se muestra como «Conectar a AIcontrol», sin dirección. Prueba el servicio local y, si no responde, usa el servicio público. Self-host borra el token y guarda solo su URL.
2. Certificado. El botón pasa a la confirmación.
3. Navegadores. «Instalar y aplicar» llama a `POST /certificado/instalar` con `cerrarNavegadores: true`.
4. Cuenta. Correo y clave de enrolamiento, `POST /activar`.
5. Listo.

Ajustes pide la clave de Config (`POST /ajustes/entrar`). La sesión dura 12 horas. Dentro se puede cambiar la dirección, cambiar la cuenta, cerrar hasta el próximo arranque (`POST /cerrar`, el PAC se quita y el certificado se queda, pausa de 180 segundos de diferencia de arranque) o desinstalar.

La desinstalación muestra dos avisos: se elimina el control de IA por completo, y el cambio no se puede deshacer. Después pide la clave. Si el servidor responde, manda esa respuesta. Sin red, se compara con la copia local en tiempo constante. Una autorización vale 900 segundos.

## Actualización

El servidor ya reparte por sistema. `macos` y `darwin` son el mismo cupo. Publicar no instala nada. El responsable pide el cambio en Actualizaciones y el latido ofrece el paquete de a 5 equipos.

El agente de Windows entiende un `GOBBIN01` con tres `.exe`. El de Mac no. El actualizador Darwin es un zip almacenado sin comprimir, y al final lleva un entero de 8 bytes con el largo del zip y la cola `GOBMAC01`. Adentro va `AIcontrol.app/`. El nombre del archivo no puede ser el del `.pkg`.

El daemon baja el archivo con el token, comprueba el SHA-256, lo deja en `pendiente/gobernanza-<versión>.paquete` y escribe `pendiente/aplicar.json`. Una copia desprendida del propio binario (`setsid`, para que launchd no la mate junto con el servicio) reemplaza `/Applications/AIcontrol.app`, copia el plist del LaunchDaemon si el bundle lo trae, y hace `launchctl kickstart -k system/local.itsec.aicontrol`. No usa el pkg y no pide reinicio. Un paquete inválido no se aplica, y ese intento no se repite hasta el próximo arranque.

Cuando el proceso nuevo informa la versión instalada, el latido manda `estadoActualizacion: aplicada` y borra el marcador. Mientras la versión del marcador no es la que corre, manda `pendiente_reinicio`.

El instalador de un equipo nuevo es `AIcontrol.pkg`. Al bajarlo, el nombre lleva la versión: `AIcontrol-0.1.0.pkg`. La firma Developer ID y la notarización se hacen en un Mac, fuera de este repositorio, igual que la firma OV de Windows.

## Desinstalación

El daemon desactiva el LaunchDaemon para que `KeepAlive` no lo vuelva a levantar, arranca una copia desprendida y sale. Esa copia:

1. Cierra los navegadores, quita el PAC de `networksetup`, borra los plist y el `user.js`, y comprueba que no quede el proxy.
2. Quita `Gobernanza 2` del llavero del sistema. Lo reintenta hasta 8 veces.
3. Avisa al panel, borra el LaunchDaemon, el LaunchAgent, `estado.json`, el token, `config.dat`, la clave del llavero, la CA y `/Applications/AIcontrol.app`.

Si el panel no responde, la desinstalación local sigue y el último paso lo dice.
