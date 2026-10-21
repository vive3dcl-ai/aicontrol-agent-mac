import AppKit

let base = "http://127.0.0.1:4765"
let sesionRed = URLSession(configuration: .ephemeral, delegate: nil, delegateQueue: OperationQueue())

func pedir(_ metodo: String, _ ruta: String, _ cuerpo: String?) -> (Int, String) {
    guard let url = URL(string: base + ruta) else { return (0, "") }
    var pedido = URLRequest(url: url)
    pedido.httpMethod = metodo
    pedido.timeoutInterval = 30
    if let cuerpo {
        pedido.httpBody = Data(cuerpo.utf8)
        pedido.setValue("application/json", forHTTPHeaderField: "Content-Type")
    }
    let espera = DispatchSemaphore(value: 0)
    var codigo = 0
    var texto = ""
    sesionRed.dataTask(with: pedido) { datos, respuesta, _ in
        codigo = (respuesta as? HTTPURLResponse)?.statusCode ?? 0
        texto = String(data: datos ?? Data(), encoding: .utf8) ?? ""
        espera.signal()
    }.resume()
    _ = espera.wait(timeout: .now() + 31)
    return (codigo, texto)
}

func jsonDe(_ texto: String) -> [String: Any] {
    guard let datos = texto.data(using: .utf8),
          let objeto = try? JSONSerialization.jsonObject(with: datos) as? [String: Any] else { return [:] }
    return objeto
}

func escapar(_ texto: String) -> String {
    texto.replacingOccurrences(of: "\\", with: "\\\\").replacingOccurrences(of: "\"", with: "\\\"")
}

final class Agente: NSObject, NSApplicationDelegate, NSWindowDelegate {
    let barra = NSStatusBar.system.statusItem(withLength: NSStatusItem.squareLength)
    let ventana = NSWindow(
        contentRect: NSRect(x: 0, y: 0, width: 420, height: 520),
        styleMask: [.titled, .closable],
        backing: .buffered,
        defer: false
    )
    var estado: [String: Any] = [:]
    var modoAjustes = false
    var preguntar = false
    var confirmarCierre = false
    var enLinea = false
    var timer: Timer?

    func applicationDidFinishLaunching(_ notification: Notification) {
        barra.button?.title = "AI"
        barra.button?.target = self
        barra.button?.action = #selector(clicBarra)
        barra.button?.sendAction(on: [.leftMouseUp, .rightMouseUp])
        ventana.title = "AIcontrol"
        ventana.delegate = self
        ventana.isReleasedWhenClosed = false
        ventana.center()
        refrescar()
        timer = Timer.scheduledTimer(withTimeInterval: 1, repeats: true) { [weak self] _ in self?.refrescar() }
        let (codigo, _) = pedir("GET", "/estado.json", nil)
        if codigo != 200 || pasoActual() != "listo" {
            mostrar()
        }
    }

    @objc func clicBarra(_ sender: NSStatusBarButton) {
        let evento = NSApp.currentEvent
        if evento?.type == .rightMouseUp {
            let menu = NSMenu()
            menu.addItem(withTitle: "Abrir", action: #selector(abrirNormal), keyEquivalent: "")
            menu.addItem(withTitle: "Ajustes", action: #selector(abrirAjustes), keyEquivalent: "")
            barra.menu = menu
            barra.button?.performClick(nil)
            barra.menu = nil
            return
        }
        if ventana.isVisible { ventana.orderOut(nil) } else { abrirNormal() }
    }

    @objc func abrirNormal() {
        modoAjustes = false
        mostrar()
    }

    @objc func abrirAjustes() {
        _ = pedir("POST", "/ajustes/bloquear", nil)
        modoAjustes = true
        mostrar()
    }

    func mostrar() {
        refrescar()
        ventana.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)
        dibujar()
    }

    func refrescar() {
        let (codigo, texto) = pedir("GET", "/estado.json", nil)
        if codigo == 200 {
            enLinea = true
            estado = jsonDe(texto)
            let conexion = estado["conexion"] as? String ?? "desconectado"
            barra.button?.title = conexion == "estable" ? "●" : "○"
        } else {
            enLinea = false
            if estado.isEmpty {
                barra.button?.title = "○"
            }
        }
        if ventana.isVisible {
            let editor = ventana.firstResponder is NSTextView
            if !editor {
                dibujar()
            }
        }
    }

    func pasoActual() -> String {
        let endpoint = estado["endpointElegido"] as? Bool ?? false
        let certificado = (estado["certificado"] as? String) == "instalado"
        let enlazado = estado["enlazado"] as? Bool ?? false
        let token = estado["tieneToken"] as? Bool ?? false
        let ajustes = estado["ajustes"] as? Bool ?? false
        if modoAjustes { return ajustes ? "ajustes" : "clave" }
        if !endpoint { return "servidor" }
        if !certificado { return preguntar ? "navegadores" : "certificado" }
        if !enlazado || !token { return "cuenta" }
        return "listo"
    }

    func dibujar() {
        guard let vista = ventana.contentView else { return }
        vista.subviews.forEach { $0.removeFromSuperview() }
        let titulo = NSTextField(labelWithString: "AIcontrol")
        titulo.font = NSFont.boldSystemFont(ofSize: 20)
        titulo.frame = NSRect(x: 24, y: 460, width: 360, height: 28)
        vista.addSubview(titulo)
        if !enLinea {
            texto("El agente no responde en 127.0.0.1:4765.", y: 420)
            return
        }
        switch pasoActual() {
        case "servidor":
            texto("Elige el servidor de la organización.", y: 420)
            boton("Global", accion: #selector(elegirGlobal), y: 360)
            boton("Self-host", accion: #selector(elegirPropio), y: 310)
            campo("host", "https://servidor:7590", y: 260, seguro: false)
        case "certificado":
            texto("Instala el certificado del equipo para poder ver el tráfico cifrado.", y: 420)
            boton("Instalar certificado", accion: #selector(pedirNavegadores), y: 340)
        case "navegadores":
            texto("Se cierran los navegadores abiertos y se aplica el filtro.", y: 420)
            boton("Instalar y aplicar", accion: #selector(instalarCertificado), y: 340)
        case "cuenta":
            texto("Enlaza la cuenta con el correo y la clave de enrolamiento.", y: 420)
            campo("correo", "correo", y: 360, seguro: false)
            campo("clave", "clave", y: 310, seguro: true)
            boton("Activar", accion: #selector(activar), y: 250)
        case "listo":
            let conexion = estado["conexion"] as? String ?? ""
            texto(tituloConexion(conexion), y: 420)
            texto("Certificado: \(estado["certificado"] as? String ?? "pendiente")", y: 380)
            texto("Cuenta: \(estado["correo"] as? String ?? "")", y: 350)
        case "clave":
            texto("Escribe la clave de Config.", y: 420)
            campo("claveAjustes", "clave", y: 360, seguro: true)
            boton("Entrar", accion: #selector(entrarAjustes), y: 300)
        case "ajustes":
            texto("Dirección del servidor.", y: 430)
            boton("Global", accion: #selector(guardarGlobal), y: 380)
            campo("hostAjustes", estado["hostPersonalizado"] as? String ?? "https://servidor:7590", y: 330, seguro: false)
            boton("Guardar dirección", accion: #selector(guardarPropio), y: 280)
            texto("Cambiar cuenta.", y: 240)
            campo("correoNuevo", "correo", y: 200, seguro: false)
            campo("claveNueva", "clave", y: 160, seguro: true)
            boton("Cambiar cuenta", accion: #selector(cambiarCuenta), y: 110)
            boton(confirmarCierre ? "Sí, cerrar" : "Cerrar", accion: #selector(cerrarHastaInicio), y: 60)
            boton("Desinstalar", accion: #selector(desinstalar), y: 16)
        default:
            break
        }
    }

    func tituloConexion(_ conexion: String) -> String {
        switch conexion {
        case "estable": return "Conectado"
        case "conectando": return "Conectando"
        case "eligiendo": return "Elige el servidor"
        default: return "Desconectado"
        }
    }

    func texto(_ valor: String, y: CGFloat) {
        let campo = NSTextField(wrappingLabelWithString: valor)
        campo.frame = NSRect(x: 24, y: y, width: 372, height: 44)
        ventana.contentView?.addSubview(campo)
    }

    func boton(_ titulo: String, accion: Selector, y: CGFloat) {
        let boton = NSButton(title: titulo, target: self, action: accion)
        boton.frame = NSRect(x: 24, y: y, width: 180, height: 32)
        ventana.contentView?.addSubview(boton)
    }

    func campo(_ id: String, _ marcador: String, y: CGFloat, seguro: Bool) {
        let campo: NSTextField = seguro ? NSSecureTextField(frame: .zero) : NSTextField(frame: .zero)
        campo.identifier = NSUserInterfaceItemIdentifier(id)
        campo.placeholderString = marcador
        campo.frame = NSRect(x: 220, y: y, width: 176, height: 28)
        ventana.contentView?.addSubview(campo)
    }

    func valor(_ id: String) -> String {
        ventana.contentView?.subviews.compactMap { $0 as? NSTextField }.first { $0.identifier?.rawValue == id }?.stringValue ?? ""
    }

    func elegir(_ modo: String, _ host: String, _ ruta: String) {
        let cuerpo = "{\"modo\":\"\(escapar(modo))\",\"host\":\"\(escapar(host))\"}"
        let (codigo, texto) = pedir("POST", ruta, cuerpo)
        if codigo != 204 { avisar(texto.isEmpty ? "No se pudo guardar la dirección" : texto) }
        preguntar = false
        refrescar()
        dibujar()
    }

    @objc func elegirGlobal() { elegir("global", "", "/endpoint/elegir") }
    @objc func elegirPropio() { elegir("personalizado", valor("host"), "/endpoint/elegir") }
    @objc func guardarGlobal() { elegir("global", "", "/ajustes/direccion") }
    @objc func guardarPropio() { elegir("personalizado", valor("hostAjustes"), "/ajustes/direccion") }

    @objc func pedirNavegadores() {
        preguntar = true
        dibujar()
    }

    @objc func instalarCertificado() {
        let (codigo, texto) = pedir("POST", "/certificado/instalar", "{\"cerrarNavegadores\":true}")
        if codigo != 200 { avisar(jsonDe(texto)["error"] as? String ?? texto) }
        preguntar = false
        refrescar()
        dibujar()
    }

    @objc func activar() {
        let cuerpo = "{\"correo\":\"\(escapar(valor("correo")))\",\"clave\":\"\(escapar(valor("clave")))\"}"
        let (codigo, texto) = pedir("POST", "/activar", cuerpo)
        if codigo != 204 { avisar(texto.isEmpty ? "No se pudo enlazar la cuenta" : texto) }
        refrescar()
        dibujar()
    }

    @objc func cambiarCuenta() {
        let cuerpo = "{\"correo\":\"\(escapar(valor("correoNuevo")))\",\"clave\":\"\(escapar(valor("claveNueva")))\"}"
        let (codigo, texto) = pedir("POST", "/activar", cuerpo)
        if codigo != 204 { avisar(texto.isEmpty ? "No se pudo cambiar la cuenta" : texto) }
        refrescar()
    }

    @objc func entrarAjustes() {
        let cuerpo = "{\"clave\":\"\(escapar(valor("claveAjustes")))\"}"
        let (codigo, texto) = pedir("POST", "/ajustes/entrar", cuerpo)
        if codigo != 204 { avisar(texto.isEmpty ? "Clave de Config incorrecta" : texto) }
        refrescar()
        dibujar()
    }

    @objc func cerrarHastaInicio() {
        if !confirmarCierre {
            confirmarCierre = true
            dibujar()
            return
        }
        _ = pedir("POST", "/cerrar", nil)
        ventana.orderOut(nil)
    }

    @objc func desinstalar() {
        guard alerta("AIcontrol", "Si desinstala AIcontrol, se eliminará el control de IA por completo. Los navegadores y el sistema dejarán de usar el agente.") else { return }
        guard alerta("AIcontrol", "Se quitarán el certificado, el proxy y las políticas de los navegadores. El equipo volverá a la navegación anterior. Esta acción no se puede deshacer.") else { return }
        let clave = pedirClave()
        if clave.isEmpty { return }
        let (codigo, texto) = pedir("POST", "/desinstalar", "{\"clave\":\"\(escapar(clave))\"}")
        if codigo != 204 {
            avisar(texto.isEmpty ? "No se pudo desinstalar" : texto)
            return
        }
        NSApp.terminate(nil)
    }

    func alerta(_ titulo: String, _ cuerpo: String) -> Bool {
        let aviso = NSAlert()
        aviso.messageText = titulo
        aviso.informativeText = cuerpo
        aviso.addButton(withTitle: "Continuar")
        aviso.addButton(withTitle: "Cancelar")
        return aviso.runModal() == .alertFirstButtonReturn
    }

    func pedirClave() -> String {
        let aviso = NSAlert()
        aviso.messageText = "Clave de Config"
        let campo = NSSecureTextField(frame: NSRect(x: 0, y: 0, width: 240, height: 24))
        aviso.accessoryView = campo
        aviso.addButton(withTitle: "Continuar")
        aviso.addButton(withTitle: "Cancelar")
        if aviso.runModal() != .alertFirstButtonReturn { return "" }
        return campo.stringValue
    }

    func avisar(_ texto: String) {
        let aviso = NSAlert()
        aviso.messageText = "AIcontrol"
        aviso.informativeText = texto
        aviso.runModal()
    }
}

let app = NSApplication.shared
let agente = Agente()
app.setActivationPolicy(.accessory)
app.delegate = agente
app.run()
