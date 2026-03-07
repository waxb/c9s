import Testing
import Foundation
@testable import C9sLib

@Suite("ImplementationDetailVM Tests")
@MainActor
struct ImplementationDetailVMTests {

    private func makeVM(
        id: String = "impl-42",
        detail: ImplementationDetail? = nil
    ) -> (ImplementationDetailVM, MockTervezoService) {
        let mock = MockTervezoService()
        if let detail {
            mock.getImplementationResult = .success(detail)
        } else {
            mock.getImplementationResult = .success(TestFixtures.makeImplementationDetail(id: id))
        }
        mock.getPlanResult = .success("## Plan\n1. Step one\n2. Step two")
        mock.getAnalysisResult = .success("Analysis text")
        mock.getChangesResult = .success([
            TervezoFileChange(filename: "src/main.rs", status: "modified", additions: 10, deletions: 2, changes: 12, patch: nil)
        ])
        mock.getTestOutputResult = .success([])

        let vm = ImplementationDetailVM(implementationId: id, service: mock)
        return (vm, mock)
    }

    // MARK: - Loading

    @Test("Load all populates implementation data")
    func loadAll() async {
        let (vm, mock) = makeVM()
        await vm.loadAll()

        #expect(vm.implementation != nil)
        #expect(vm.implementation?.id == "impl-42")
        #expect(vm.plan == "## Plan\n1. Step one\n2. Step two")
        #expect(vm.changes.count == 1)
        #expect(vm.isLoading == false)
        #expect(mock.getImplementationCallCount == 1)
    }

    @Test("Load failure sets error message")
    func loadFailure() async {
        let mock = MockTervezoService()
        mock.getImplementationResult = .failure(TervezoServiceError.networkError("Offline"))
        let vm = ImplementationDetailVM(implementationId: "bad-id", service: mock)

        await vm.loadAll()

        #expect(vm.implementation == nil)
        #expect(vm.errorMessage != nil)
    }

    // MARK: - Tab State

    @Test("Default tab is timeline")
    func defaultTab() {
        let (vm, _) = makeVM()
        #expect(vm.selectedTab == .timeline)
    }

    @Test("Changing tab works")
    func changeTab() {
        let (vm, _) = makeVM()
        vm.selectedTab = .plan
        #expect(vm.selectedTab == .plan)
    }

    // MARK: - Prompt

    @Test("Send prompt succeeds and clears text")
    func sendPromptSuccess() async {
        let (vm, mock) = makeVM()
        await vm.loadAll()

        vm.promptText = "Fix this bug"
        await vm.sendPrompt()

        #expect(vm.promptText.isEmpty)
        #expect(vm.isSendingPrompt == false)
        #expect(vm.promptError == nil)
        #expect(mock.sendPromptCallCount == 1)
        #expect(mock.lastSendPromptMessage == "Fix this bug")
    }

    @Test("Send prompt with empty text does nothing")
    func sendPromptEmpty() async {
        let (vm, mock) = makeVM()
        vm.promptText = "   "
        await vm.sendPrompt()

        #expect(mock.sendPromptCallCount == 0)
    }

    @Test("Send prompt conflict shows error")
    func sendPromptConflict() async {
        let (vm, mock) = makeVM()
        mock.sendPromptResult = .failure(TervezoServiceError.conflict("Not waiting for input"))

        vm.promptText = "Hello"
        await vm.sendPrompt()

        #expect(vm.promptError != nil)
        #expect(vm.promptError?.contains("Cannot send") == true)
    }

    // MARK: - Computed State

    @Test("canCreatePR is true when branch exists and no PR")
    func canCreatePR() async {
        let detail = TestFixtures.makeImplementationDetail(status: "completed")
        let (vm, _) = makeVM(detail: detail)
        await vm.loadAll()

        // Fixture has branch but no PR
        #expect(vm.canCreatePR == true)
    }

    @Test("canRestart is true for terminal statuses")
    func canRestart() async {
        let detail = TestFixtures.makeImplementationDetail(status: "failed")
        let (vm, _) = makeVM(detail: detail)
        await vm.loadAll()

        #expect(vm.canRestart == true)
    }

    @Test("canRestart is false for running")
    func cannotRestartRunning() async {
        let detail = TestFixtures.makeImplementationDetail(status: "running")
        let (vm, _) = makeVM(detail: detail)
        await vm.loadAll()

        #expect(vm.canRestart == false)
    }

    @Test("canSSH is true when sandbox exists and running")
    func canSSH() async {
        let detail = TestFixtures.makeImplementationDetail(status: "running")
        let (vm, _) = makeVM(detail: detail)
        await vm.loadAll()

        // Fixture has sandboxId and isRunning=true
        #expect(vm.canSSH == true)
    }

    // MARK: - Actions

    @Test("Create PR calls service")
    func createPR() async throws {
        let (vm, mock) = makeVM()
        mock.createPRResult = .success(TervezoPRCreateResponse(prUrl: "https://github.com/pr/1", prNumber: 1))
        await vm.loadAll()

        let result = try await vm.createPR()
        #expect(result.prUrl == "https://github.com/pr/1")
        #expect(mock.createPRCallCount == 1)
    }

    @Test("Restart calls service")
    func restart() async throws {
        let (vm, mock) = makeVM()
        mock.restartResult = .success(TervezoRestartResponse(implementationId: "impl-new", isNewImplementation: true))
        await vm.loadAll()

        let result = try await vm.restart()
        #expect(result.isNewImplementation == true)
        #expect(mock.restartCallCount == 1)
    }

    // MARK: - Refresh

    @Test("Refresh updates data based on active tab")
    func refreshUpdatesActiveTab() async {
        let (vm, mock) = makeVM()
        await vm.loadAll()
        let initialCallCount = mock.getImplementationCallCount

        vm.selectedTab = .plan
        await vm.refresh()

        #expect(mock.getImplementationCallCount == initialCallCount + 1)
    }
}
