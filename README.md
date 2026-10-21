# Agente macOS

AIcontrol para Mac sigue el modelo del agente de Windows: un daemon root, una barra de menú y un proxy local. El mapa de cada función está en [DESDE-WINDOWS.md](DESDE-WINDOWS.md). El crate de Windows no se toca.

Hay tres piezas.

- [daemon](daemon) es el LaunchDaemon. Ahí viven el proxy `127.0.0.1:8079`, la tarjeta `127.0.0.1:4765`, el latido, la CA y el PAC. Los datos quedan en `/Library/Application Support/Gobernanza`.
- [app](app) es la barra de menú. Solo habla con la tarjeta. No abre `config.dat` ni el llavero.
- [instalador](instalador) arma el `.app`, el zip `GOBMAC01` y el `.pkg`.

`GobernanzaRed/` es el esqueleto viejo de `NETransparentProxyProvider`. No se empaqueta. El tráfico entra por el PAC, no por una extensión de red.

## Compilar

El daemon se prueba en Linux. El `.app` y el pkg se arman en un Mac: hacen falta Xcode, el SDK de macOS y las raíces del sistema para el llavero y `networksetup`.

```bash
cargo test --manifest-path daemon/Cargo.toml
```

En un Mac:

```bash
bash instalador/construir.sh
```

Eso deja `AIcontrol.app`, `AIcontrol-actualizacion.zip` (cola `GOBMAC01`) y, si `pkgbuild` está, `AIcontrol.pkg`.

El alta informa `sistema: macos`. Las actualizaciones del panel usan `darwin`; el servidor trata las dos cadenas como el mismo sistema.

## Firma

El pkg se firma con Developer ID y se notariza con `notarytool`. El certificado no se guarda en este repositorio. Sin esa firma, Gatekeeper avisa al abrir el pkg. El daemon no pide el entitlement de Network Extension.
