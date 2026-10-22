import AppKit

let base = "http://127.0.0.1:4765"
let sesionRed = URLSession(configuration: .ephemeral, delegate: nil, delegateQueue: OperationQueue())

let tintaFondo = NSColor(srgbRed: 28 / 255, green: 25 / 255, blue: 23 / 255, alpha: 1)
let tintaBorde = NSColor(srgbRed: 68 / 255, green: 64 / 255, blue: 60 / 255, alpha: 1)
let tintaClara = NSColor(srgbRed: 250 / 255, green: 250 / 255, blue: 249 / 255, alpha: 1)
let tintaPiedra = NSColor(srgbRed: 214 / 255, green: 211 / 255, blue: 209 / 255, alpha: 1)
let tintaCampo = NSColor(srgbRed: 41 / 255, green: 37 / 255, blue: 36 / 255, alpha: 1)
let tintaPeligro = NSColor(srgbRed: 252 / 255, green: 165 / 255, blue: 165 / 255, alpha: 1)
let tintaBordePeligro = NSColor(srgbRed: 127 / 255, green: 29 / 255, blue: 29 / 255, alpha: 1)
let tintaOk = NSColor(srgbRed: 134 / 255, green: 239 / 255, blue: 172 / 255, alpha: 1)
let verde = NSColor(srgbRed: 22 / 255, green: 163 / 255, blue: 74 / 255, alpha: 1)
let azul = NSColor(srgbRed: 56 / 255, green: 189 / 255, blue: 248 / 255, alpha: 1)
let rojo = NSColor(srgbRed: 220 / 255, green: 38 / 255, blue: 38 / 255, alpha: 1)
let tituloVerde = NSColor(srgbRed: 74 / 255, green: 222 / 255, blue: 128 / 255, alpha: 1)
let tituloAzul = NSColor(srgbRed: 125 / 255, green: 211 / 255, blue: 252 / 255, alpha: 1)
let tituloRojo = NSColor(srgbRed: 248 / 255, green: 113 / 255, blue: 113 / 255, alpha: 1)

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

func colorOrbe(_ conexion: String) -> NSColor {
    switch conexion {
    case "estable": return verde
    case "conectando", "eligiendo": return azul
    default: return rojo
    }
}

func colorTitulo(_ conexion: String) -> NSColor {
    switch conexion {
    case "estable": return tituloVerde
    case "conectando", "eligiendo": return tituloAzul
    default: return tituloRojo
    }
}

func tituloConexion(_ conexion: String) -> String {
    switch conexion {
    case "estable": return "Conectado"
    case "conectando": return "Conectando"
    case "eligiendo": return "Elige el servidor"
    default: return "Sin conexión"
    }
}

func iconoEstado(_ conexion: String, tenue: Bool) -> NSImage {
    let lado = 22
    let espacio = CGColorSpaceCreateDeviceRGB()
    let info = CGImageAlphaInfo.premultipliedLast.rawValue
    guard let ctx = CGContext(data: nil, width: lado, height: lado, bitsPerComponent: 8, bytesPerRow: lado * 4, space: espacio, bitmapInfo: info) else {
        return NSImage()
    }
    ctx.clear(CGRect(x: 0, y: 0, width: lado, height: lado))
    let color = colorOrbe(conexion)
    ctx.setFillColor(color.cgColor)
    let radio: CGFloat = tenue ? 5 : 8
    let origen = (CGFloat(lado) - radio * 2) / 2
    ctx.fillEllipse(in: CGRect(x: origen, y: origen, width: radio * 2, height: radio * 2))
    guard let cg = ctx.makeImage() else { return NSImage() }
    let imagen = NSImage(cgImage: cg, size: NSSize(width: lado, height: lado))
    imagen.isTemplate = false
    return imagen
}

func logoMarca() -> NSImage? {
    let candidatos = [
        Bundle.main.url(forResource: "logo", withExtension: "bgra"),
        Bundle.main.resourceURL?.appendingPathComponent("logo.bgra"),
        URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent().appendingPathComponent("Resources/logo.bgra"),
    ]
    guard let url = candidatos.compactMap({ $0 }).first(where: { FileManager.default.fileExists(atPath: $0.path) }),
          let datos = try? Data(contentsOf: url) else { return nil }
    let ancho = 600
    let alto = 155
    guard datos.count >= ancho * alto * 4 else { return nil }
    var rgba = Data(count: ancho * alto * 4)
    datos.withUnsafeBytes { crudo in
        let src = crudo.bindMemory(to: UInt8.self)
        rgba.withUnsafeMutableBytes { destino in
            let pix = destino.bindMemory(to: UInt8.self)
            for i in 0..<(ancho * alto) {
                let b = Int(src[i * 4])
                let g = Int(src[i * 4 + 1])
                let r = Int(src[i * 4 + 2])
                let a = Int(src[i * 4 + 3])
                if a == 0 { continue }
                pix[i * 4] = UInt8(r * 255 / a)
                pix[i * 4 + 1] = UInt8(g * 255 / a)
                pix[i * 4 + 2] = UInt8(b * 255 / a)
                pix[i * 4 + 3] = UInt8(a)
            }
        }
    }
    let espacio = CGColorSpaceCreateDeviceRGB()
    guard let proveedor = CGDataProvider(data: rgba as CFData),
          let cg = CGImage(width: ancho, height: alto, bitsPerComponent: 8, bitsPerPixel: 32, bytesPerRow: ancho * 4, space: espacio, bitmapInfo: CGBitmapInfo(rawValue: CGImageAlphaInfo.premultipliedLast.rawValue), provider: proveedor, decode: nil, shouldInterpolate: true, intent: .defaultIntent) else { return nil }
    return NSImage(cgImage: cg, size: NSSize(width: ancho, height: alto))
}

final class FondoTarjeta: NSView {
    override var isFlipped: Bool { true }
    override func draw(_ dirtyRect: NSRect) {
        NSColor.clear.setFill()
        dirtyRect.fill()
        let camino = NSBezierPath(roundedRect: bounds.insetBy(dx: 0.5, dy: 0.5), xRadius: 18, yRadius: 18)
        tintaFondo.setFill()
        camino.fill()
        tintaBorde.setStroke()
        camino.lineWidth = 1
        camino.stroke()
    }
}

final class Orbe: NSView {
    var color = rojo
    var angulo: CGFloat = 0
    var animar = false
    override var isFlipped: Bool { true }
    override func draw(_ dirtyRect: NSRect) {
        guard let ctx = NSGraphicsContext.current?.cgContext else { return }
        ctx.saveGState()
        ctx.translateBy(x: 0, y: bounds.height)
        ctx.scaleBy(x: 1, y: -1)
        let halo = bounds.insetBy(dx: 10, dy: 10)
        ctx.setFillColor(color.withAlphaComponent(0.2).cgColor)
        ctx.fillEllipse(in: halo)
        ctx.setFillColor(color.cgColor)
        ctx.fillEllipse(in: halo.insetBy(dx: 28, dy: 28))
        if animar {
            ctx.setStrokeColor(color.cgColor)
            ctx.setLineWidth(4)
            ctx.setLineCap(.round)
            ctx.addArc(center: CGPoint(x: bounds.midX, y: bounds.midY), radius: halo.width / 2 - 2, startAngle: angulo * .pi / 180, endAngle: (angulo + 110) * .pi / 180, clockwise: false)
            ctx.strokePath()
        }
        ctx.restoreGState()
    }
}

final class Agente: NSObject, NSApplicationDelegate, NSWindowDelegate {
    let barra = NSStatusBar.system.statusItem(withLength: NSStatusItem.squareLength)
    let ventana = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 340, height: 420), styleMask: [.borderless], backing: .buffered, defer: false)
    let fondo = FondoTarjeta(frame: NSRect(x: 0, y: 0, width: 340, height: 420))
    let logo = NSImageView()
    let orbe = Orbe(frame: NSRect(x: 0, y: 0, width: 132, height: 132))
    let titulo = NSTextField(labelWithString: "Sin conexión")
    let certificado = NSTextField(labelWithString: "Certificado pendiente")
    let aviso = NSTextField(wrappingLabelWithString: "")
    let modoGlobal = NSButton(title: "AIcontrol", target: nil, action: nil)
    let modoPropio = NSButton(title: "Self-host", target: nil, action: nil)
    let direccion = NSTextField(wrappingLabelWithString: "")
    let host = NSTextField()
    let guardarDireccion = NSButton(title: "Guardar dirección", target: nil, action: nil)
    let correo = NSTextField()
    let clave = NSSecureTextField()
    let principal = NSButton(title: "Activar", target: nil, action: nil)
    let secundario = NSButton(title: "Cerrar", target: nil, action: nil)
    let baja = NSButton(title: "Desinstalar", target: nil, action: nil)
    let claveBaja = NSSecureTextField()

    var estado: [String: Any] = [:]
    var modoAjustes = false
    var preguntar = false
    var enLinea = false
    var modoConexion = "global"
    var direccionLista = false
    var faseBaja = 0
    var faseCierre = false
    var pulso = false
    var informe = ""
    var colorAviso = tintaPiedra
    var anclada = false

    func applicationDidFinishLaunching(_ notification: Notification) {
        barra.button?.imagePosition = .imageOnly
        barra.button?.imageScaling = .scaleProportionallyDown
        barra.button?.target = self
        barra.button?.action = #selector(clicBarra)
        barra.button?.sendAction(on: [.leftMouseUp, .rightMouseUp])
        ponerIcono("desconectado", tenue: false)
        ventana.isOpaque = false
        ventana.backgroundColor = .clear
        ventana.hasShadow = true
        ventana.level = .floating
        ventana.isReleasedWhenClosed = false
        ventana.appearance = NSAppearance(named: .darkAqua)
        ventana.contentView = fondo
        armar()
        Timer.scheduledTimer(withTimeInterval: 1, repeats: true) { [weak self] _ in self?.refrescar() }
        Timer.scheduledTimer(withTimeInterval: 0.04, repeats: true) { [weak self] _ in self?.girarOrbe() }
        refrescar()
        let (codigo, _) = pedir("GET", "/estado.json", nil)
        if codigo != 200 || pasoActual() != "listo" { mostrar() }
    }

    func armar() {
        logo.image = logoMarca()
        logo.imageScaling = .scaleProportionallyDown
        logo.imageAlignment = .alignCenter
        if logo.image == nil {
            logo.isHidden = true
        }
        titulo.alignment = .center
        titulo.font = NSFont.systemFont(ofSize: 16, weight: .semibold)
        titulo.textColor = tituloRojo
        titulo.drawsBackground = false
        titulo.isBezeled = false
        certificado.alignment = .center
        certificado.font = NSFont.systemFont(ofSize: 13)
        certificado.textColor = tintaPiedra
        certificado.drawsBackground = false
        certificado.isBezeled = false
        aviso.alignment = .center
        aviso.font = NSFont.systemFont(ofSize: 13)
        aviso.textColor = tintaPiedra
        aviso.maximumNumberOfLines = 6
        aviso.drawsBackground = false
        aviso.isBezeled = false
        direccion.alignment = .center
        direccion.font = NSFont.systemFont(ofSize: 13)
        direccion.textColor = tintaPiedra
        direccion.drawsBackground = false
        direccion.isBezeled = false
        estiloCampo(host, "https://tu-servidor")
        estiloCampo(correo, "Correo del usuario")
        estiloCampo(clave, "Clave de enrolamiento")
        estiloCampo(claveBaja, "Clave de Config")
        modoGlobal.target = self
        modoGlobal.action = #selector(marcarGlobal)
        modoPropio.target = self
        modoPropio.action = #selector(marcarPropio)
        guardarDireccion.target = self
        guardarDireccion.action = #selector(guardarPropio)
        principal.target = self
        principal.action = #selector(principalPulsado)
        secundario.target = self
        secundario.action = #selector(cerrarHastaInicio)
        baja.target = self
        baja.action = #selector(desinstalar)
        for vista in [logo, orbe, titulo, certificado, aviso, modoGlobal, modoPropio, direccion, host, guardarDireccion, correo, clave, principal, secundario, claveBaja, baja] as [NSView] {
            fondo.addSubview(vista)
        }
    }

    func estiloCampo(_ campo: NSTextField, _ marca: String) {
        campo.placeholderString = marca
        campo.placeholderAttributedString = NSAttributedString(string: marca, attributes: [.foregroundColor: tintaPiedra, .font: NSFont.systemFont(ofSize: 13)])
        campo.textColor = tintaClara
        campo.backgroundColor = tintaCampo
        campo.isBezeled = false
        campo.isBordered = false
        campo.drawsBackground = true
        campo.focusRingType = .none
        campo.font = NSFont.systemFont(ofSize: 13)
        campo.wantsLayer = true
        campo.layer?.cornerRadius = 8
        campo.layer?.borderWidth = 1
        campo.layer?.borderColor = tintaBorde.cgColor
        campo.layer?.backgroundColor = tintaCampo.cgColor
    }

    func estiloBoton(_ boton: NSButton, relleno: NSColor?, tinta: NSColor, borde: NSColor) {
        boton.isBordered = false
        boton.wantsLayer = true
        boton.layer?.cornerRadius = 8
        boton.layer?.backgroundColor = (relleno ?? .clear).cgColor
        boton.layer?.borderWidth = 1
        boton.layer?.borderColor = borde.cgColor
        let parrafo = NSMutableParagraphStyle()
        parrafo.alignment = .center
        boton.attributedTitle = NSAttributedString(string: boton.title, attributes: [
            .foregroundColor: tinta,
            .font: NSFont.systemFont(ofSize: 13, weight: .semibold),
            .paragraphStyle: parrafo,
        ])
    }

    func ponerIcono(_ conexion: String, tenue: Bool) {
        let imagen = iconoEstado(conexion, tenue: tenue)
        barra.button?.image = imagen
        barra.button?.image?.isTemplate = false
    }

    @objc func clicBarra(_ sender: NSStatusBarButton) {
        if NSApp.currentEvent?.type == .rightMouseUp {
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
        faseBaja = 0
        faseCierre = false
        mostrar()
    }

    @objc func abrirAjustes() {
        _ = pedir("POST", "/ajustes/bloquear", nil)
        modoAjustes = true
        faseBaja = 0
        faseCierre = false
        direccionLista = false
        mostrar()
    }

    func mostrar() {
        refrescar()
        let primera = !anclada
        aplicar()
        if primera {
            anclar()
            anclada = true
        }
        ventana.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)
    }

    func anclar() {
        guard let boton = barra.button, let origen = boton.window else {
            ventana.center()
            return
        }
        let rect = origen.convertToScreen(boton.convert(boton.bounds, to: nil))
        let x = max(8, rect.midX - 170)
        let y = max(8, rect.minY - ventana.frame.height - 8)
        ventana.setFrameOrigin(NSPoint(x: x, y: y))
    }

    func refrescar() {
        let (codigo, texto) = pedir("GET", "/estado.json", nil)
        if codigo == 200 {
            enLinea = true
            estado = jsonDe(texto)
        } else {
            enLinea = false
            if estado.isEmpty {
                estado = ["conexion": "desconectado", "certificado": "pendiente", "enlazado": false]
            }
        }
        let conexion = estado["conexion"] as? String ?? "desconectado"
        pulso.toggle()
        let animando = conexion == "conectando" || conexion == "eligiendo"
        ponerIcono(conexion, tenue: animando && pulso)
        if ventana.isVisible && !(ventana.firstResponder is NSTextView) {
            aplicar()
        } else if ventana.isVisible {
            pintarEstado()
        }
    }

    func girarOrbe() {
        guard ventana.isVisible, orbe.animar else { return }
        orbe.angulo = (orbe.angulo + 8).truncatingRemainder(dividingBy: 360)
        orbe.needsDisplay = true
    }

    func pasoActual() -> String {
        let endpoint = estado["endpointElegido"] as? Bool ?? false
        let certificadoListo = (estado["certificado"] as? String) == "instalado"
        let enlazado = estado["enlazado"] as? Bool ?? false
        let token = estado["tieneToken"] as? Bool ?? false
        let ajustes = estado["ajustes"] as? Bool ?? false
        if modoAjustes { return ajustes ? "ajustes" : "clave" }
        if !endpoint { return "servidor" }
        if !certificadoListo { return preguntar ? "navegadores" : "certificado" }
        if !enlazado || !token { return "cuenta" }
        return "listo"
    }

    func pintarEstado() {
        let conexion = estado["conexion"] as? String ?? "desconectado"
        let paso = pasoActual()
        orbe.animar = conexion == "conectando" || conexion == "eligiendo"
        orbe.color = colorOrbe(conexion)
        orbe.needsDisplay = true
        if paso == "ajustes" || paso == "clave" {
            titulo.stringValue = "Ajustes"
            titulo.textColor = tintaClara
        } else {
            titulo.stringValue = tituloConexion(conexion)
            titulo.textColor = colorTitulo(conexion)
        }
        let instalado = (estado["certificado"] as? String) == "instalado"
        certificado.stringValue = instalado ? "Certificado instalado" : "Certificado pendiente"
        aviso.stringValue = informe
        aviso.textColor = colorAviso
    }

    func aplicar() {
        let paso = pasoActual()
        let editando = ventana.firstResponder is NSTextView
        pintarEstado()
        let muestraCert = paso != "servidor" && paso != "ajustes" && paso != "clave"
        var piezas: [(NSView, CGFloat)] = []
        func fila(_ vista: NSView, _ alto: CGFloat) {
            vista.isHidden = false
            piezas.append((vista, alto))
        }
        for vista in [logo, orbe, titulo, certificado, aviso, modoGlobal, modoPropio, direccion, host, guardarDireccion, correo, clave, principal, secundario, claveBaja, baja] {
            vista.isHidden = true
        }
        if logo.image != nil && paso != "ajustes" && paso != "clave" { fila(logo, 52) }
        if paso != "ajustes" && paso != "clave" { fila(orbe, 132) }
        fila(titulo, 24)
        if muestraCert { fila(certificado, 18) }
        switch paso {
        case "servidor":
            aviso.stringValue = "Self-host usa un servidor propio. AIcontrol prueba este equipo y, si no responde, sigue por el servicio público."
            aviso.textColor = tintaPiedra
            fila(aviso, 64)
            fila(direccion, 18)
            direccion.stringValue = "Conectar a AIcontrol"
            fila(modoGlobal, 34)
            fila(modoPropio, 34)
            if modoConexion == "personalizado" { fila(host, 32) }
            principal.title = "Continuar"
            fila(principal, 36)
        case "certificado":
            aviso.stringValue = "Hace falta el certificado del equipo para ver el tráfico de IA."
            aviso.textColor = tintaPiedra
            fila(aviso, 40)
            principal.title = "Instalar certificado"
            fila(principal, 36)
        case "navegadores":
            aviso.stringValue = "Al instalar el certificado se activa el filtro en Chrome, Edge, Brave, Opera, Firefox y el resto de navegadores Chromium. Los que estén abiertos se cierran para que lo tomen."
            aviso.textColor = tintaPiedra
            fila(aviso, 72)
            principal.title = "Instalar y aplicar"
            fila(principal, 36)
        case "cuenta":
            if !informe.isEmpty { fila(aviso, 36) }
            if !editando {
                clave.placeholderString = "Clave de enrolamiento"
            }
            fila(correo, 32)
            fila(clave, 32)
            principal.title = "Activar"
            fila(principal, 36)
        case "listo":
            if !informe.isEmpty { fila(aviso, 36) }
        case "clave":
            if !informe.isEmpty { fila(aviso, 36) }
            if !editando { clave.placeholderString = "Clave de Config" }
            fila(clave, 32)
            principal.title = "Entrar"
            fila(principal, 36)
        case "ajustes":
            if faseCierre {
                aviso.stringValue = "El agente se detiene hasta el próximo reinicio. El certificado se queda y el filtro vuelve al arrancar."
                aviso.textColor = tintaPeligro
            } else if faseBaja == 1 {
                aviso.stringValue = "Si desinstala AIcontrol, se eliminará el control de IA por completo. Los navegadores y el sistema dejarán de usar el agente."
                aviso.textColor = tintaPeligro
            } else if faseBaja == 2 {
                aviso.stringValue = "Se quitarán el certificado, el proxy y las políticas de los navegadores. El equipo volverá a la navegación anterior. Esta acción no se puede deshacer."
                aviso.textColor = tintaPeligro
            }
            if !aviso.stringValue.isEmpty { fila(aviso, 64) }
            fila(direccion, 18)
            direccion.stringValue = "Conectar a AIcontrol"
            fila(modoGlobal, 34)
            fila(modoPropio, 34)
            if !direccionLista && !editando {
                modoConexion = (estado["hostModo"] as? String) == "personalizado" ? "personalizado" : "global"
                host.stringValue = estado["hostPersonalizado"] as? String ?? ""
                direccionLista = true
            }
            if modoConexion == "personalizado" { fila(host, 32) }
            guardarDireccion.title = "Guardar dirección"
            fila(guardarDireccion, 34)
            if !editando { clave.placeholderString = "Clave de enrolamiento" }
            fila(correo, 32)
            fila(clave, 32)
            principal.title = "Cambiar cuenta"
            fila(principal, 36)
            secundario.title = faseCierre ? "Sí, cerrar" : "Cerrar"
            fila(secundario, 34)
            if faseBaja == 2 { fila(claveBaja, 32) }
            baja.title = faseBaja == 1 ? "Continuar" : faseBaja == 2 ? "Sí, desinstalar" : "Desinstalar"
            fila(baja, 34)
        default:
            aviso.stringValue = "El agente no responde en 127.0.0.1:4765."
            aviso.textColor = tintaPeligro
            fila(aviso, 40)
        }
        if aviso.stringValue.isEmpty { aviso.isHidden = true }
        let elegido = tintaClara
        estiloBoton(modoGlobal, relleno: modoConexion == "global" ? elegido : nil, tinta: modoConexion == "global" ? tintaFondo : tintaPiedra, borde: modoConexion == "global" ? elegido : tintaBorde)
        estiloBoton(modoPropio, relleno: modoConexion == "personalizado" ? elegido : nil, tinta: modoConexion == "personalizado" ? tintaFondo : tintaPiedra, borde: modoConexion == "personalizado" ? elegido : tintaBorde)
        estiloBoton(principal, relleno: tintaClara, tinta: tintaFondo, borde: tintaClara)
        estiloBoton(guardarDireccion, relleno: tintaCampo, tinta: tintaClara, borde: tintaBorde)
        estiloBoton(secundario, relleno: nil, tinta: tintaPeligro, borde: tintaBordePeligro)
        estiloBoton(baja, relleno: nil, tinta: tintaPeligro, borde: tintaBordePeligro)
        var y: CGFloat = 18
        let ancho: CGFloat = 304
        for (vista, alto) in piezas where !vista.isHidden && alto > 0 {
            let x: CGFloat = vista === orbe || vista === logo ? (340 - (vista === orbe ? 132 : 200)) / 2 : 18
            let w: CGFloat = vista === orbe ? 132 : vista === logo ? 200 : ancho
            vista.frame = NSRect(x: x, y: y, width: w, height: alto)
            y += alto + 10
        }
        let altoVentana = max(180, y + 8)
        let arriba = ventana.frame.maxY
        let origenX = ventana.frame.minX
        ventana.setFrame(NSRect(x: anclada ? origenX : ventana.frame.minX, y: anclada ? arriba - altoVentana : ventana.frame.minY, width: 340, height: altoVentana), display: true)
        fondo.frame = ventana.contentView?.bounds ?? fondo.frame
        fondo.needsDisplay = true
    }

    @objc func marcarGlobal() {
        modoConexion = "global"
        aplicar()
    }

    @objc func marcarPropio() {
        modoConexion = "personalizado"
        aplicar()
    }

    @objc func principalPulsado() {
        switch pasoActual() {
        case "servidor":
            elegir(modoConexion, host.stringValue, "/endpoint/elegir")
        case "certificado":
            preguntar = true
            aplicar()
        case "navegadores":
            instalarCertificado()
        case "cuenta", "ajustes":
            activar()
        case "clave":
            entrarAjustes()
        default:
            break
        }
    }

    func elegir(_ modo: String, _ host: String, _ ruta: String) {
        let cuerpo = "{\"modo\":\"\(escapar(modo))\",\"host\":\"\(escapar(host))\"}"
        let (codigo, texto) = pedir("POST", ruta, cuerpo)
        if codigo != 204 {
            informe = texto.isEmpty ? "No se pudo guardar la dirección" : texto
            colorAviso = tintaPeligro
        } else if ruta == "/ajustes/direccion" {
            informe = "Dirección guardada"
            colorAviso = tintaOk
        } else {
            informe = ""
        }
        preguntar = false
        refrescar()
        aplicar()
    }

    @objc func guardarPropio() { elegir(modoConexion, host.stringValue, "/ajustes/direccion") }

    func instalarCertificado() {
        let (codigo, texto) = pedir("POST", "/certificado/instalar", "{\"cerrarNavegadores\":true}")
        if codigo != 200 {
            informe = jsonDe(texto)["error"] as? String ?? (texto.isEmpty ? "No se pudo instalar el certificado" : texto)
            colorAviso = tintaPeligro
        } else {
            informe = ""
        }
        preguntar = false
        refrescar()
        aplicar()
    }

    func activar() {
        let cuerpo = "{\"correo\":\"\(escapar(correo.stringValue))\",\"clave\":\"\(escapar(clave.stringValue))\"}"
        let (codigo, texto) = pedir("POST", "/activar", cuerpo)
        if codigo != 204 {
            informe = texto.isEmpty ? "No se pudo enlazar la cuenta" : texto
            colorAviso = tintaPeligro
        } else {
            clave.stringValue = ""
            informe = "Cuenta enlazada"
            colorAviso = tintaOk
        }
        refrescar()
        aplicar()
    }

    func entrarAjustes() {
        let cuerpo = "{\"clave\":\"\(escapar(clave.stringValue))\"}"
        let (codigo, texto) = pedir("POST", "/ajustes/entrar", cuerpo)
        if codigo != 204 {
            informe = texto.isEmpty ? "Clave de Config incorrecta" : texto
            colorAviso = tintaPeligro
        } else {
            clave.stringValue = ""
            informe = ""
            direccionLista = false
        }
        refrescar()
        aplicar()
    }

    @objc func cerrarHastaInicio() {
        if !faseCierre {
            faseCierre = true
            faseBaja = 0
            aplicar()
            return
        }
        _ = pedir("POST", "/cerrar", nil)
        ventana.orderOut(nil)
    }

    @objc func desinstalar() {
        if faseBaja == 0 {
            faseCierre = false
            faseBaja = 1
            aplicar()
            return
        }
        if faseBaja == 1 {
            faseBaja = 2
            aplicar()
            return
        }
        let cuerpo = "{\"clave\":\"\(escapar(claveBaja.stringValue))\"}"
        let (codigo, texto) = pedir("POST", "/desinstalar", cuerpo)
        if codigo != 204 {
            informe = texto.isEmpty ? "No se pudo desinstalar" : texto
            colorAviso = tintaPeligro
            aviso.stringValue = informe
            aviso.textColor = tintaPeligro
            return
        }
        NSApp.terminate(nil)
    }
}

let app = NSApplication.shared
let agente = Agente()
app.setActivationPolicy(.accessory)
app.delegate = agente
app.run()
