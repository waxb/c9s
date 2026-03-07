import Foundation
import Observation

/// ViewModel for the SSH terminal screen.
/// Manages SSH credential fetching, connection state, and terminal lifecycle.
@Observable
@MainActor
final class TerminalVM {

    // MARK: - State

    var credentials: TervezoSSHCredentials?
    var isLoadingCredentials = false
    var isConnected = false
    var errorMessage: String?
    var connectionState: ConnectionState = .disconnected

    enum ConnectionState: String {
        case disconnected = "Disconnected"
        case fetchingCredentials = "Fetching credentials..."
        case connecting = "Connecting..."
        case connected = "Connected"
        case failed = "Connection failed"
    }

    // MARK: - Dependencies

    let implementationId: String
    private let sshService: SSHService

    init(implementationId: String, sshService: SSHService = SSHService()) {
        self.implementationId = implementationId
        self.sshService = sshService
    }

    // MARK: - Actions

    /// Fetch SSH credentials for the implementation's sandbox.
    func loadCredentials() async {
        isLoadingCredentials = true
        connectionState = .fetchingCredentials
        errorMessage = nil

        do {
            credentials = try await sshService.getCredentials(implementationId: implementationId)
            connectionState = .disconnected
        } catch {
            errorMessage = error.localizedDescription
            connectionState = .failed
        }

        isLoadingCredentials = false
    }

    /// Get the SSH command string for copying to clipboard.
    var sshCommand: String? {
        credentials.map { SSHService.sshCommandString(from: $0) }
    }

    /// Get the sandbox web URL for browser-based access.
    var sandboxWebURL: URL? {
        credentials.flatMap { SSHService.sandboxWebURL(from: $0) }
    }

    /// Mark terminal as connected.
    func markConnected() {
        isConnected = true
        connectionState = .connected
    }

    /// Mark terminal as disconnected.
    func markDisconnected() {
        isConnected = false
        connectionState = .disconnected
    }
}
