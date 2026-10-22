# Servicio y actualizaciones

El agente de Windows ya funciona así. Este documento deja el mismo modelo para aplicarlo después en Linux y macOS. No cambia el agente que ya está instalado en Linux.

## Windows, como quedó

Hay dos programas, con nombre visible AIcontrol.

- `AIcontrol-servicio.exe` es el servicio. El nombre interno en Windows sigue siendo `GobernanzaAgente`, cuenta `LocalSystem`, arranque automático. No acepta `SERVICE_CONTROL_STOP`. Sí acepta el apagado del equipo. La carpeta sigue siendo `C:\Program Files\ITsec\Gobernanza` y los datos `%ProgramData%\Gobernanza`, para que una actualización en el sitio no pierda el certificado ni la configuración.
- `AIcontrol-bandeja.exe` es el icono de la sesión del usuario. El servicio mira las sesiones activas. Si el proceso de la bandeja no está, lo vuelve a crear con el token de esa sesión. La bandeja escribe el PAC de WinINET del usuario. El servicio escribe las políticas de navegador en `HKLM` y la CA en el almacén del equipo.
- `uninstall.exe` es el desinstalador. El instalador de escritorio se llama `AIcontrol-setup.exe` y lleva el icono del producto.

Un equipo que todavía tiene `gobernanza-servicio.exe` recibe el paquete con esos nombres viejos, porque es el único formato que su actualizador acepta. Al arrancar, el binario nuevo se copia a `AIcontrol-servicio.exe` y `AIcontrol-bandeja.exe`, apunta el servicio a la ruta nueva y termina. Windows lo vuelve a iniciar a los 5 segundos. Ese segundo arranque borra los ejecutables viejos. El Administrador de tareas muestra la descripción `AIcontrol` porque la ruta del archivo cambió. No se lanza un segundo proceso a mano.

El usuario que se informa a la flota es el de la sesión activa. Se descarta la cuenta del equipo (`PC$`) y las cuentas `SYSTEM`, `Local Service` y `Network Service`.

Un administrador local todavía puede finalizar el proceso desde el Administrador de tareas. No se esconde el proceso ni se usa un driver.

## Lo que hay que repetir en Linux

La unidad de sistema reemplaza al servicio. El agente de Linux 0.1.12 ya lo aplica.

- `gobernanza-agente.service` sigue en el sistema, con `Restart=always` y `RestartSec` corto. No se entrega una unidad de usuario que el operador pueda parar. Quien no es root no detiene la unidad. Root sí puede, del mismo modo que el administrador de Windows puede matar el proceso.
- La bandeja sigue en la sesión gráfica. Si el proceso muere, la unidad de sistema la vuelve a levantar. No hace falta que la bandeja sea la dueña del proxy.
- El proxy, el latido y la CA quedan en el proceso de sistema. El estado vive en `/var/lib/gobernanza`, no en el home.
- El nombre visible del producto es AIcontrol. El nombre interno del servicio y la carpeta de datos pueden quedarse, igual que en Windows, para no romper el certificado ni la configuración en una actualización en el sitio.
- El instalador y la firma de Linux quedan para cuando se arme el paquete. El paralelo de la firma OV es firmar el paquete con la clave de ITsec. S.A. y publicar el hash en el mismo registro de versiones.

## macOS, como quedó

El daemon y la barra de menú viven en `/Applications/AIcontrol.app`. El LaunchDaemon `local.itsec.aicontrol` corre el daemon como root, con `KeepAlive`. El LaunchAgent `local.itsec.aicontrol.menu` abre la barra en la sesión gráfica. Si esa barra falta, el daemon la vuelve a abrir. Root puede bajar el daemon; el usuario no tiene un botón de parar.

Los datos siguen en `/Library/Application Support/Gobernanza`, para que una actualización en el sitio no pierda la CA ni `config.dat`. La clave de ese archivo está en el llavero del sistema. El detalle de cada función está en `gobernanza/agente/macos/DESDE-WINDOWS.md`.

El actualizador no es el `GOBBIN01` de Windows. Es un zip con la cola `GOBMAC01`: al final la marca `GOBMAC01`, antes un entero de 8 bytes con el largo del zip, y el zip trae `AIcontrol.app/`. Una copia desprendida reemplaza la app y hace `launchctl kickstart -k system/local.itsec.aicontrol`. No usa el pkg y no pide reinicio. El instalador de un equipo nuevo es `AIcontrol.pkg`. El alta manda `sistema: macos`. Publicar y repartir usan `darwin`; el servidor los trata como el mismo sistema.

## Actualización, igual en los dos

No se desinstala la versión anterior y no hace falta reiniciar el equipo. El paquete reemplaza los ejecutables en silencio y el servicio vuelve a arrancar.

1. La plataforma publica la versión con dos archivos: el instalador, que el cliente baja desde Actualizaciones, y el actualizador, que el agente se aplica solo. Publicar no envía nada a los equipos.
2. El responsable del tenant, en Actualizaciones, pide el cambio para Windows, Linux o macOS.
3. El servidor reparte la descarga en bloques de 5 equipos conectados. El tamaño se puede cambiar con `ACTUALIZACION_BLOQUE`. Quien no entra en el bloque queda en espera y no ocupa cupo. Un equipo desconectado no ocupa cupo: cuando vuelve a conectar, el latido lo promueve si hay lugar.
4. El latido responde con la versión, las notas, el hash y la URL solo si el equipo está atrás y su cupo está en turno o descargando. Quien ya está aplicando no vuelve a recibir el archivo.
5. El agente descarga en silencio, comprueba el SHA-256 y no toca el ejecutable que está corriendo. Una pausa de hasta 10 minutos no corta la descarga. El latido espera 60 segundos. El servidor acepta la transferencia hasta 30 minutos. Informa el avance de 0 a 90 y luego marca aplicando en 95.
6. Una copia desprendida del propio ejecutable reemplaza los tres programas, vuelve a registrar el servicio y lo arranca. Si el paquete no es válido, no se llama a `msiexec`.
7. Cuando el proceso nuevo informa la versión instalada, la fase y el porcentaje de ese equipo se borran. La tarjeta de la flota deja de mostrar progreso.

La flota tiene, además, una sección `actualizacionGlobal` que solo viene en la respuesta mientras quede algún equipo del sistema pedido por detrás de la versión. Trae `porcentaje`, `listos`, `total`, `enCurso` y `enEspera`. El porcentaje es el promedio: un equipo al día vale 100, uno en descarga o aplicación vale su avance, uno en espera o desconectado vale 0. Cuando todos llegaron, el campo es `null` y la sección no se dibuja. El progreso de cada tarjeta tampoco se dibuja si ese equipo no tiene fase.

En Windows el archivo es un `GOBBIN01`: al final la marca `GOBBIN01`, antes un entero de 8 bytes con el largo de la carga, y la carga empieza con la cantidad de archivos (siempre 3) y, por cada uno, el largo del nombre, el nombre, el largo y los bytes. El agente que ya está instalado acepta los nombres `gobernanza-servicio.exe`, `gobernanza-bandeja.exe` y `uninstall.exe`. El agente nuevo también acepta `AIcontrol-servicio.exe` y `AIcontrol-bandeja.exe`, y al instalarlos los deja con el nombre nuevo. Hasta que todos los equipos hayan pasado por esa versión, el paquete publicado tiene que seguir trayendo los nombres viejos.

En Linux el paquete es un `GOBLIN01`: al final la marca `GOBLIN01`, antes un entero de 8 bytes con el largo, y la carga es el binario nuevo. El agente lo guarda en `/var/lib/gobernanza/pendiente`, comprueba el hash, reemplaza el binario sin desinstalar antes y reinicia la unidad. No se aplica en el arranque siguiente ni con un reinicio del equipo.

En macOS el archivo es el zip `GOBMAC01` descrito arriba. El agente que ya está instalado lo aplica solo. El `.pkg` es únicamente el instalador que se entrega a un cliente nuevo.

## Cómo se sube una versión

Cada versión de un sistema lleva dos archivos distintos. Los dos se publican juntos en `POST /v1/actualizaciones`. El cuerpo es JSON:

- `sistema`: `windows`, `linux` o `darwin`.
- `version`: `mayor.menor.parche`.
- `cambios`: notas que ve el tenant.
- `archivoNombre`, `sha256`, `archivoBase64`: el actualizador automático. En Windows es el paquete `GOBBIN01` que el agente ya instalado sabe aplicar. En macOS es el zip `GOBMAC01`. En Linux es el paquete `GOBLIN01`. El nombre no puede ser el del instalador.
- `instaladorNombre`, `instaladorSha256`, `instaladorBase64`: el instalador que se entrega a un cliente nuevo. En Windows es `AIcontrol-setup.exe`. En macOS es `AIcontrol.pkg`. En Linux es `AIcontrol-setup.deb`.

El SHA-256 es el del archivo en binario, en hexadecimal de 64 caracteres, y tiene que coincidir con el base64. El servidor guarda los dos bajo `data/actualizaciones/{sistema}/{version}/`, cada uno con su nombre. Si el cuerpo no trae instalador, conserva el que ya estaba.

La llamada lleva `Authorization: Bearer`. El token `act_` se pega en el archivo `token_update`, en la raíz del repo, en una sola línea; la subida lo manda con ese mismo encabezado. Una sesión de plataforma deja la versión aprobada. Un token de actualizador, creado en el panel en Actualizadores, empieza por `act_` y usa el mismo encabezado. Ese token solo puede subir los sistemas que tiene permitidos; si el sistema no está en el permiso, la respuesta es 403. La subida con token queda pendiente hasta que un admin la aprueba. Rechazarla no la borra del historial. Una versión ya aprobada no se pisa: hay que subir otra versión y la respuesta es 409. Una pendiente del mismo sistema y versión se puede reemplazar. El cuerpo grande se envía al API (`https://aai.latinrouter.ai/v1/actualizaciones`), no al panel.

El tenant, en Actualizaciones, solo ve la última versión aprobada. Aprobar no manda la actualización a la flota.

El responsable, en Actualizaciones, usa Actualizar para el paquete automático y Descargar, a la derecha, para bajar el instalador de la última versión publicada de ese sistema. Descargar llama a `GET /v1/actualizaciones/instalador?sistema=windows` y solo responde si esa versión tiene instalador. El archivo que llega lleva la versión en el nombre: `AIcontrol-setup.exe` de la 0.1.9 se baja como `AIcontrol-setup-0.1.9.exe`.

## Desinstalación

Antes de borrar el servicio y los datos, el desinstalador avisa con `POST /v1/dispositivos/desinstalar`, autenticado con el token del equipo. Lo reintenta tres veces. Si el panel responde, la flota marca el equipo como desinstalado y crea el aviso «Equipo desinstalado». Si no hay red, la desinstalación local sigue y el último paso lo dice.

En la tarjeta desinstalada aparece «Este equipo desinstaló el agente» y el botón Borrar. Borrar pide la suma de dos números y recién entonces llama a `DELETE /v1/dispositivos/{id}`. Solo se puede borrar un equipo que ya está desinstalado.

## Shadow AI

El paquete del latido trae `sombra`: `accion` (`permitir`, `bloquear` o `redirigir`) y, si redirige, `redireccion` con `nombre` y `url`. Esa regla cubre cada host de `feedDominios` que no tenga una política más específica. Un dominio adoptado sale del feed, entra en `sitios` y pasa a una política normal. El modo web no pisa `sombra`. Si `accion` es `permitir`, el feed no se bloquea aunque el modo web sea bloquear.
