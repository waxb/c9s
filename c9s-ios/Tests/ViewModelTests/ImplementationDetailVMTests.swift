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

    // MARK: - PR Actions (mergePR, closePR, reopenPR)

    @Test("Merge PR calls service and refreshes detail")
    func mergePR() async throws {
        let (vm, mock) = makeVM()
        mock.mergePRResult = .success(true)
        await vm.loadAll()
        let initialCallCount = mock.getImplementationCallCount

        let result = try await vm.mergePR()
        #expect(result == true)
        // mergePR calls loadDetail after success
        #expect(mock.getImplementationCallCount == initialCallCount + 1)
    }

    @Test("Close PR calls service and refreshes detail")
    func closePR() async throws {
        let (vm, mock) = makeVM()
        mock.closePRResult = .success(true)
        await vm.loadAll()
        let initialCallCount = mock.getImplementationCallCount

        let result = try await vm.closePR()
        #expect(result == true)
        #expect(mock.getImplementationCallCount == initialCallCount + 1)
    }

    @Test("Reopen PR calls service and refreshes detail")
    func reopenPR() async throws {
        let (vm, mock) = makeVM()
        mock.reopenPRResult = .success(true)
        await vm.loadAll()
        let initialCallCount = mock.getImplementationCallCount

        let result = try await vm.reopenPR()
        #expect(result == true)
        #expect(mock.getImplementationCallCount == initialCallCount + 1)
    }

    @Test("Merge PR propagates service error")
    func mergePRError() async {
        let (vm, mock) = makeVM()
        mock.mergePRResult = .failure(TervezoServiceError.httpError(statusCode: 409, message: "PR has conflicts"))
        await vm.loadAll()

        do {
            _ = try await vm.mergePR()
            Issue.record("Should have thrown")
        } catch {
            // Expected — error propagated to caller
        }
    }

    // MARK: - canMergePR

    @Test("canMergePR true when PR exists and is open")
    func canMergePR() async {
        let detail = ImplementationDetail(
            id: "impl-merge", title: "Test", status: "completed", mode: "feature",
            prompt: nil, plan: nil, analysis: nil, error: nil, isRunning: false,
            repoUrl: nil, branch: "feat/test", baseBranch: "main", branchPushed: true,
            prUrl: "https://github.com/test/repo/pull/1", prStatus: "open",
            sandboxId: nil, iterations: 1, currentIteration: 1,
            createdAt: Date(), updatedAt: nil, steps: [], timelineMessageCount: 0
        )
        let (vm, _) = makeVM(detail: detail)
        await vm.loadAll()

        #expect(vm.canMergePR == true)
    }

    @Test("canMergePR false when no PR URL")
    func cannotMergePRWithoutURL() async {
        let (vm, _) = makeVM() // Default fixture has no prUrl
        await vm.loadAll()
        #expect(vm.canMergePR == false)
    }

    // MARK: - isWaitingForInput

    @Test("isWaitingForInput true when running with waiting step")
    func isWaitingForInput() async {
        let detail = ImplementationDetail(
            id: "impl-wait", title: "Waiting", status: "running", mode: "feature",
            prompt: nil, plan: nil, analysis: nil, error: nil, isRunning: true,
            repoUrl: nil, branch: nil, baseBranch: nil, branchPushed: nil,
            prUrl: nil, prStatus: nil, sandboxId: nil,
            iterations: 1, currentIteration: 1,
            createdAt: Date(), updatedAt: nil,
            steps: [
                TervezoStep(id: "s1", name: "Planning", order: 1, status: "completed",
                           startedAt: nil, completedAt: nil, error: nil),
                TervezoStep(id: "s2", name: "Implementation", order: 2, status: "waiting_for_input",
                           startedAt: nil, completedAt: nil, error: nil),
            ],
            timelineMessageCount: 5
        )
        let (vm, _) = makeVM(detail: detail)
        await vm.loadAll()

        #expect(vm.isWaitingForInput == true)
    }

    @Test("isWaitingForInput false when not running")
    func isNotWaitingWhenCompleted() async {
        let detail = TestFixtures.makeImplementationDetail(status: "completed")
        let (vm, _) = makeVM(detail: detail)
        await vm.loadAll()

        #expect(vm.isWaitingForInput == false)
    }

    // MARK: - canRestart for all terminal statuses

    @Test("canRestart true for all terminal statuses")
    func canRestartTerminalStatuses() async {
        for status in ["failed", "stopped", "cancelled", "completed", "merged"] {
            let detail = TestFixtures.makeImplementationDetail(status: status)
            let mock = MockTervezoService()
            mock.getImplementationResult = .success(detail)
            mock.getPlanResult = .success("")
            mock.getAnalysisResult = .success("")
            mock.getChangesResult = .success([])
            mock.getTestOutputResult = .success([])
            let vm = ImplementationDetailVM(implementationId: "test", service: mock)
            await vm.loadAll()
            #expect(vm.canRestart == true, "canRestart should be true for status '\(status)'")
        }
    }

    // MARK: - Send Prompt Non-Conflict Error

    @Test("Send prompt network error sets promptError and preserves text")
    func sendPromptNetworkError() async {
        let (vm, mock) = makeVM()
        mock.sendPromptResult = .failure(TervezoServiceError.networkError("Connection timeout"))

        vm.promptText = "Keep this text"
        await vm.sendPrompt()

        #expect(vm.promptError != nil)
        // Text should be preserved on error (not cleared)
        #expect(vm.promptText == "Keep this text")
        #expect(vm.isSendingPrompt == false)
    }

    // MARK: - SSE Event Handling

    @Test("handleSSEEvent connected sets isStreaming")
    func handleSSEConnected() async {
        let (vm, _) = makeVM()
        #expect(vm.isStreaming == false)

        await vm.handleSSEEventForTest(.connected)

        #expect(vm.isStreaming == true)
        #expect(vm.streamError == nil)
    }

    @Test("handleSSEEvent disconnected clears isStreaming")
    func handleSSEDisconnected() async {
        let (vm, _) = makeVM()
        await vm.handleSSEEventForTest(.connected)
        #expect(vm.isStreaming == true)

        await vm.handleSSEEventForTest(.disconnected)
        #expect(vm.isStreaming == false)
    }

    @Test("handleSSEEvent timelineMessages appends non-duplicate messages")
    func handleSSETimelineMessages() async {
        let (vm, _) = makeVM()
        await vm.loadAll()

        let messages = [
            TervezoTimelineMessage(id: "sse-1", type: "user_prompt",
                                   timestamp: "2026-03-07T10:00:00Z",
                                   rawJSON: ["text": .string("Hello")]),
            TervezoTimelineMessage(id: "sse-2", type: "assistant_text",
                                   timestamp: "2026-03-07T10:00:05Z",
                                   rawJSON: ["text": .string("Hi")]),
        ]

        await vm.handleSSEEventForTest(.timelineMessages(messages))
        #expect(vm.timeline.count == 2)

        // Sending same messages again should not duplicate
        await vm.handleSSEEventForTest(.timelineMessages(messages))
        #expect(vm.timeline.count == 2)
    }

    @Test("handleSSEEvent planUpdate sets plan")
    func handleSSEPlanUpdate() async {
        let (vm, _) = makeVM()
        await vm.handleSSEEventForTest(.planUpdate("## New Plan"))
        #expect(vm.plan == "## New Plan")
    }

    @Test("handleSSEEvent analysisUpdate sets analysis")
    func handleSSEAnalysisUpdate() async {
        let (vm, _) = makeVM()
        await vm.handleSSEEventForTest(.analysisUpdate("New analysis"))
        #expect(vm.analysis == "New analysis")
    }

    @Test("handleSSEEvent error sets streamError")
    func handleSSEError() async {
        let (vm, _) = makeVM()
        await vm.handleSSEEventForTest(.error("Connection lost"))
        #expect(vm.streamError == "Connection lost")
    }
}
