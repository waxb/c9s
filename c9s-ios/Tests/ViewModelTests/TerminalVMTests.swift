import Testing
import Foundation
@testable import C9sLib

/// Tests for TerminalVM SSH credential loading and state management.
@Suite("TerminalVM Tests")
@MainActor
struct TerminalVMTests {

    // MARK: - Initial State

    @Test("Initial state is disconnected with no credentials")
    func initialState() {
        let mock = MockTervezoService()
        let sshService = SSHService(apiService: mock)
        let vm = TerminalVM(implementationId: "impl-123", sshService: sshService)

        #expect(vm.credentials == nil)
        #expect(vm.isLoadingCredentials == false)
        #expect(vm.isConnected == false)
        #expect(vm.connectionState == .disconnected)
        #expect(vm.sshCommand == nil)
        #expect(vm.sandboxWebURL == nil)
    }

    // MARK: - Load Credentials

    @Test("Successfully loads SSH credentials")
    func loadCredentialsSuccess() async {
        let mock = MockTervezoService()
        mock.getSSHResult = .success(TestFixtures.makeSSHCredentials())
        let sshService = SSHService(apiService: mock)
        let vm = TerminalVM(implementationId: "impl-123", sshService: sshService)

        await vm.loadCredentials()

        #expect(vm.credentials != nil)
        #expect(vm.credentials?.host == "sandbox.tervezo.ai")
        #expect(vm.isLoadingCredentials == false)
        #expect(vm.errorMessage == nil)
        #expect(vm.connectionState == .disconnected)
    }

    @Test("Shows error when credential loading fails")
    func loadCredentialsFailure() async {
        let mock = MockTervezoService()
        mock.getSSHResult = .failure(TervezoServiceError.httpError(statusCode: 404, message: "Sandbox not found"))
        let sshService = SSHService(apiService: mock)
        let vm = TerminalVM(implementationId: "impl-123", sshService: sshService)

        await vm.loadCredentials()

        #expect(vm.credentials == nil)
        #expect(vm.errorMessage != nil)
        #expect(vm.connectionState == .failed)
        #expect(vm.isLoadingCredentials == false)
    }

    // MARK: - SSH Command

    @Test("SSH command is available after loading credentials")
    func sshCommandAvailable() async {
        let mock = MockTervezoService()
        mock.getSSHResult = .success(TestFixtures.makeSSHCredentials(
            sshCommand: "ssh -p 2222 user@sandbox.tervezo.ai"
        ))
        let sshService = SSHService(apiService: mock)
        let vm = TerminalVM(implementationId: "impl-123", sshService: sshService)

        await vm.loadCredentials()

        #expect(vm.sshCommand == "ssh -p 2222 user@sandbox.tervezo.ai")
    }

    // MARK: - Sandbox Web URL

    @Test("Sandbox web URL is available after loading credentials")
    func sandboxWebURLAvailable() async {
        let mock = MockTervezoService()
        mock.getSSHResult = .success(TestFixtures.makeSSHCredentials(
            sandboxUrl: "https://sandbox.tervezo.ai/terminal/abc123"
        ))
        let sshService = SSHService(apiService: mock)
        let vm = TerminalVM(implementationId: "impl-123", sshService: sshService)

        await vm.loadCredentials()

        #expect(vm.sandboxWebURL != nil)
        #expect(vm.sandboxWebURL?.absoluteString == "https://sandbox.tervezo.ai/terminal/abc123")
    }

    // MARK: - Connection State

    @Test("Mark connected updates state")
    func markConnected() {
        let mock = MockTervezoService()
        let sshService = SSHService(apiService: mock)
        let vm = TerminalVM(implementationId: "impl-123", sshService: sshService)

        vm.markConnected()

        #expect(vm.isConnected == true)
        #expect(vm.connectionState == .connected)
    }

    @Test("Mark disconnected updates state")
    func markDisconnected() {
        let mock = MockTervezoService()
        let sshService = SSHService(apiService: mock)
        let vm = TerminalVM(implementationId: "impl-123", sshService: sshService)

        vm.markConnected()
        vm.markDisconnected()

        #expect(vm.isConnected == false)
        #expect(vm.connectionState == .disconnected)
    }

    @Test("Implementation ID is stored")
    func implementationIdStored() {
        let mock = MockTervezoService()
        let sshService = SSHService(apiService: mock)
        let vm = TerminalVM(implementationId: "my-impl-456", sshService: sshService)

        #expect(vm.implementationId == "my-impl-456")
    }
}
