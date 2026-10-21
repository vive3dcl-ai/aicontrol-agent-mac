import Foundation
import Network
import NetworkExtension

/// No forma parte del agente. El tráfico entra por el PAC y el daemon en 127.0.0.1:8079.
/// Esqueleto de proxy transparente. No inspecciona y no reclama flujos:
/// si la inspección no está, la red sigue.
final class GobernanzaProxyProvider: NETransparentProxyProvider {
    override func startProxy(options: [String: Any]?, completionHandler: @escaping (Error?) -> Void) {
        completionHandler(nil)
    }

    override func stopProxy(with reason: NEProviderStopReason, completionHandler: @escaping () -> Void) {
        completionHandler()
    }

    override func handleNewFlow(_ flow: NEAppProxyFlow) -> Bool {
        _ = flow
        return false
    }

    override func handleNewUDPFlow(_ flow: NEAppProxyUDPFlow, initialRemoteEndpoint remoteEndpoint: NWEndpoint) -> Bool {
        _ = flow
        _ = remoteEndpoint
        return false
    }
}
