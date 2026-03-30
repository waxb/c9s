import Foundation

/// Service for fetching SSH credentials and managing sandbox terminal connections.
/// Fetches credentials from the Tervezo API and provides connection parameters
/// for the terminal view.
final class SSHService: Sendable {

    private let apiService: TervezoServiceProtocol

    init(apiService: TervezoServiceProtocol = TervezoService()) {
        self.apiService = apiService
    }

    /// Fetch SSH credentials for an implementation's sandbox.
    func getCredentials(implementationId: String) async throws -> TervezoSSHCredentials {
        try await apiService.getSSH(id: implementationId)
    }

    /// Build a copyable SSH command string from credentials.
    static func sshCommandString(from credentials: TervezoSSHCredentials) -> String {
        credentials.sshCommand
    }

    /// Build the sandbox web URL for browser-based terminal access.
    static func sandboxWebURL(from credentials: TervezoSSHCredentials) -> URL? {
        URL(string: credentials.sandboxUrl)
    }
}
