# AIcontrol agent macOS

Agente nativo de macOS: daemon Rust y barra de menú Swift. El código está en `agente/macos` y los esquemas del contrato en `agente/contrato`.

```bash
cargo test --manifest-path agente/macos/daemon/Cargo.toml
```

El `.app`, el zip `GOBMAC01` y el `.pkg` se arman en un Mac:

```bash
bash agente/macos/instalador/construir.sh
```

El mapa contra Windows está en `agente/macos/DESDE-WINDOWS.md`.
