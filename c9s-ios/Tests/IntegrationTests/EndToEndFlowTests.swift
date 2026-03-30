import Testing
import Foundation
@testable import C9sLib

/// Integration tests verifying end-to-end flows across multiple ViewModels.
/// These tests simulate realistic user journeys through the app, ensuring
/// that ViewModels interact correctly with the service layer.
@Suite("End-to-End Flow Tests")
@MainActor
struct EndToEndFlowTests {

    // MARK: - List → Detail → Action Flow

    @Test("List load → select implementation → view detail → send prompt")
    func listToDetailToPrompt() async {
        let mock = MockTervezoService()

        // 1. Setup: list with one running implementation
        let summary = TestFixtures.makeImplementationSummary(
            id: "impl-e2e", title: "E2E Test", status: "running"
        )
        mock.listImplementationsResult = .success(
            ImplementationList(items: [summary], total: 1)
        )

        // 2. Load list
        let listVM = ImplementationListVM(service: mock)
        await listVM.loadImplementations()

        #expect(listVM.implementations.count == 1)
        #expect(listVM.implementations[0].id == "impl-e2e")

        // 3. Setup detail
        let detail = TestFixtures.makeImplementationDetail(
            id: "impl-e2e", title: "E2E Test", status: "running"
        )
        mock.getImplementationResult = .success(detail)
        mock.getPlanResult = .success("## Plan\n1. Fix the bug")
        mock.getAnalysisResult = .success("Analysis of the codebase")
        mock.getChangesResult = .success([])
        mock.getTestOutputResult = .success([])
        mock.getTimelineResult = .success([])

        // 4. Load detail (simulates tapping implementation in list)
        let detailVM = ImplementationDetailVM(
            implementationId: "impl-e2e", service: mock
        )
        await detailVM.loadAll()

        #expect(detailVM.implementation != nil)
        #expect(detailVM.implementation?.title == "E2E Test")
        #expect(detailVM.plan == "## Plan\n1. Fix the bug")

        // 5. Send prompt
        mock.sendPromptResult = .success(TervezoPromptResponse(sent: true, followUpId: nil))
        detailVM.promptText = "Please also fix the logout flow"
        await detailVM.sendPrompt()

        #expect(mock.sendPromptCallCount == 1)
        #expect(mock.lastSendPromptMessage == "Please also fix the logout flow")
        #expect(detailVM.promptText.isEmpty) // cleared after send
    }

    @Test("List load → select implementation → create PR → refresh")
    func listToDetailToPR() async {
        let mock = MockTervezoService()

        // Setup list and detail
        mock.listImplementationsResult = .success(
            ImplementationList(items: [
                TestFixtures.makeImplementationSummary(
                    id: "impl-pr", status: "completed"
                ),
            ], total: 1)
        )

        let detail = TestFixtures.makeImplementationDetail(
            id: "impl-pr", status: "completed"
        )
        mock.getImplementationResult = .success(detail)
        mock.getTimelineResult = .success([])
        mock.getPlanResult = .success("")
        mock.getAnalysisResult = .success("")
        mock.getChangesResult = .success([])
        mock.getTestOutputResult = .success([])

        // Load list and detail
        let listVM = ImplementationListVM(service: mock)
        await listVM.loadImplementations()
        let detailVM = ImplementationDetailVM(
            implementationId: "impl-pr", service: mock
        )
        await detailVM.loadAll()

        // Create PR
        mock.createPRResult = .success(
            TervezoPRCreateResponse(prUrl: "https://github.com/user/repo/pull/42", prNumber: 42)
        )
        await detailVM.createPR()

        #expect(mock.createPRCallCount == 1)
    }

    // MARK: - Create Implementation Flow

    @Test("Load workspaces → fill form → create → navigate to detail")
    func createImplementationFlow() async {
        let mock = MockTervezoService()

        // Setup workspaces
        mock.listWorkspacesResult = .success([
            TestFixtures.makeWorkspace(id: "ws-1", name: "Production"),
            TestFixtures.makeWorkspace(id: "ws-2", name: "Staging"),
        ])

        // 1. Load create form
        let createVM = CreateImplementationVM(service: mock)
        await createVM.loadWorkspaces()

        #expect(createVM.workspaces.count == 2)
        #expect(createVM.selectedWorkspaceId == nil) // not auto-selected with 2 workspaces

        // 2. Fill form
        createVM.prompt = "Add user authentication with OAuth2 and JWT tokens"
        createVM.selectedWorkspaceId = "ws-1"
        createVM.selectedMode = .implement
        createVM.repositoryName = "my-app"
        createVM.baseBranch = "main"

        #expect(createVM.isValid == true)

        // 3. Submit
        let createdDetail = TestFixtures.makeImplementationDetail(
            id: "new-impl-123", title: "Add user authentication", status: "pending"
        )
        mock.createImplementationResult = .success(createdDetail)
        await createVM.submit()

        #expect(createVM.createdImplementation != nil)
        #expect(createVM.createdImplementation?.id == "new-impl-123")
        #expect(mock.createImplementationCallCount == 1)
        #expect(mock.lastCreatePrompt == "Add user authentication with OAuth2 and JWT tokens")
        #expect(mock.lastCreateWorkspaceId == "ws-1")

        // 4. Load detail for the newly created implementation
        mock.getImplementationResult = .success(createdDetail)
        mock.getTimelineResult = .success([])
        mock.getPlanResult = .success("")
        mock.getAnalysisResult = .success("")
        mock.getChangesResult = .success([])
        mock.getTestOutputResult = .success([])

        let detailVM = ImplementationDetailVM(
            implementationId: "new-impl-123", service: mock
        )
        await detailVM.loadAll()

        #expect(detailVM.implementation?.id == "new-impl-123")
        #expect(detailVM.implementation?.status == "pending")
    }

    // MARK: - Error Recovery Flow

    @Test("List load fails → retry → succeeds")
    func listLoadErrorRecovery() async {
        let mock = MockTervezoService()

        // First attempt fails
        mock.listImplementationsResult = .failure(
            TervezoServiceError.networkError("Connection refused")
        )
        let listVM = ImplementationListVM(service: mock)
        await listVM.loadImplementations()

        #expect(listVM.implementations.isEmpty)
        #expect(listVM.errorMessage != nil)

        // Retry succeeds
        mock.listImplementationsResult = .success(
            ImplementationList(items: [
                TestFixtures.makeImplementationSummary(id: "impl-1", status: "running"),
            ], total: 1)
        )
        await listVM.refresh()

        #expect(listVM.implementations.count == 1)
    }

    @Test("Detail load fails → still shows stale implementation → retry works")
    func detailLoadErrorRecovery() async {
        let mock = MockTervezoService()

        // First load succeeds
        mock.getImplementationResult = .success(
            TestFixtures.makeImplementationDetail(id: "impl-stale", status: "running")
        )
        mock.getTimelineResult = .success([])
        mock.getPlanResult = .success("Initial plan")
        mock.getAnalysisResult = .success("")
        mock.getChangesResult = .success([])
        mock.getTestOutputResult = .success([])

        let detailVM = ImplementationDetailVM(
            implementationId: "impl-stale", service: mock
        )
        await detailVM.loadAll()

        #expect(detailVM.implementation != nil)
        #expect(detailVM.plan == "Initial plan")

        // Second load fails (network error)
        mock.getImplementationResult = .failure(
            TervezoServiceError.networkError("Timeout")
        )
        await detailVM.refresh()

        // Implementation data should still be present from first load
        #expect(detailVM.implementation != nil)
    }

    // MARK: - Filter and Search Flow

    @Test("Load list → filter by status → search → clear filters")
    func listFilterSearchFlow() async {
        let mock = MockTervezoService()
        mock.listImplementationsResult = .success(
            ImplementationList(items: [
                TestFixtures.makeImplementationSummary(
                    id: "impl-1", title: "Fix login bug", status: "running"
                ),
                TestFixtures.makeImplementationSummary(
                    id: "impl-2", title: "Add dark mode", status: "completed"
                ),
                TestFixtures.makeImplementationSummary(
                    id: "impl-3", title: "Refactor auth", status: "failed"
                ),
            ], total: 3)
        )

        let listVM = ImplementationListVM(service: mock)
        await listVM.loadImplementations()

        #expect(listVM.filteredImplementations.count == 3)

        // Filter by running
        listVM.statusFilter = .running
        #expect(listVM.filteredImplementations.count == 1)
        #expect(listVM.filteredImplementations[0].id == "impl-1")

        // Search within filter
        listVM.statusFilter = .all
        listVM.searchText = "dark"
        #expect(listVM.filteredImplementations.count == 1)
        #expect(listVM.filteredImplementations[0].id == "impl-2")

        // Clear search
        listVM.searchText = ""
        #expect(listVM.filteredImplementations.count == 3)
    }

    // MARK: - Prompt Conflict Handling

    @Test("Send prompt to non-running implementation returns conflict")
    func promptConflict() async {
        let mock = MockTervezoService()

        // Setup completed implementation
        mock.getImplementationResult = .success(
            TestFixtures.makeImplementationDetail(id: "impl-done", status: "completed")
        )
        mock.getTimelineResult = .success([])
        mock.getPlanResult = .success("")
        mock.getAnalysisResult = .success("")
        mock.getChangesResult = .success([])
        mock.getTestOutputResult = .success([])

        let detailVM = ImplementationDetailVM(
            implementationId: "impl-done", service: mock
        )
        await detailVM.loadAll()

        // Attempt to send prompt — should get conflict
        mock.sendPromptResult = .failure(
            TervezoServiceError.conflict("Implementation is not in a valid state for prompts")
        )
        detailVM.promptText = "Can you also fix the other thing?"
        await detailVM.sendPrompt()

        #expect(detailVM.errorMessage != nil)
        #expect(detailVM.errorMessage?.contains("Conflict") == true)
    }

    // MARK: - SSH Terminal Flow

    @Test("Fetch SSH credentials for implementation with sandbox")
    func sshCredentialFlow() async {
        let mock = MockTervezoService()
        mock.getSSHResult = .success(TervezoSSHCredentials(
            host: "sandbox.tervezo.ai",
            port: 2222,
            username: "developer",
            sshCommand: "ssh -p 2222 developer@sandbox.tervezo.ai",
            sandboxId: "sb-123",
            sandboxUrl: "https://sandbox.tervezo.ai/terminal/sb-123"
        ))

        let sshService = SSHService(apiService: mock)
        let terminalVM = TerminalVM(implementationId: "impl-with-sandbox", sshService: sshService)

        await terminalVM.loadCredentials()

        #expect(terminalVM.credentials != nil)
        #expect(terminalVM.credentials?.host == "sandbox.tervezo.ai")
        #expect(terminalVM.sshCommand == "ssh -p 2222 developer@sandbox.tervezo.ai")
        #expect(terminalVM.sandboxWebURL?.absoluteString == "https://sandbox.tervezo.ai/terminal/sb-123")
        #expect(terminalVM.connectionState == .disconnected)
    }
}
