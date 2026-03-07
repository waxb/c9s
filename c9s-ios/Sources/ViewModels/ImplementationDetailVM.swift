import Foundation
import Observation

/// ViewModel for the implementation detail screen.
/// Manages implementation data, tabs, timeline, prompt input, and actions.
@Observable
@MainActor
final class ImplementationDetailVM {

    // MARK: - State

    var implementation: ImplementationDetail?
    var timeline: [TervezoTimelineMessage] = []
    var plan: String?
    var analysis: String?
    var changes: [TervezoFileChange] = []
    var testReports: [TervezoTestReport] = []
    var isLoading = true
    var errorMessage: String?

    /// Active tab in the detail view.
    var selectedTab: DetailTab = .timeline

    /// Prompt input state.
    var promptText = ""
    var isSendingPrompt = false
    var promptError: String?

    /// Whether the implementation is waiting for user input.
    var isWaitingForInput: Bool {
        implementation?.status == "running" && implementation?.steps.contains(where: {
            $0.status == "waiting_for_input"
        }) ?? false
    }

    /// Whether actions are available (PR, restart, etc.).
    var canCreatePR: Bool {
        implementation?.branch != nil && implementation?.prUrl == nil
    }
    var canMergePR: Bool {
        implementation?.prUrl != nil && implementation?.prStatus == "open"
    }
    var canRestart: Bool {
        let terminalStatuses = Set(["failed", "stopped", "cancelled", "completed", "merged"])
        return terminalStatuses.contains(implementation?.status ?? "")
    }
    var canSSH: Bool {
        implementation?.sandboxId != nil && implementation?.isRunning == true
    }

    // MARK: - Types

    enum DetailTab: String, CaseIterable, Identifiable {
        case timeline = "Timeline"
        case plan = "Plan"
        case changes = "Changes"
        case tests = "Tests"

        var id: String { rawValue }

        var icon: String {
            switch self {
            case .timeline: "text.bubble"
            case .plan: "doc.text"
            case .changes: "doc.badge.plus"
            case .tests: "checkmark.circle"
            }
        }
    }

    // MARK: - Dependencies

    private let implementationId: String
    private let service: TervezoServiceProtocol
    private var pollingTask: Task<Void, Never>?

    init(implementationId: String, service: TervezoServiceProtocol = TervezoService()) {
        self.implementationId = implementationId
        self.service = service
    }

    deinit {
        pollingTask?.cancel()
    }

    // MARK: - Loading

    /// Load all implementation data in parallel.
    func loadAll() async {
        isLoading = true
        errorMessage = nil

        await withTaskGroup(of: Void.self) { group in
            group.addTask { await self.loadDetail() }
            group.addTask { await self.loadTimeline() }
            group.addTask { await self.loadPlan() }
            group.addTask { await self.loadChanges() }
            group.addTask { await self.loadTestOutput() }
        }

        isLoading = false
    }

    /// Refresh the implementation detail and active tab data.
    func refresh() async {
        await loadDetail()

        switch selectedTab {
        case .timeline: await loadTimeline()
        case .plan: await loadPlan()
        case .changes: await loadChanges()
        case .tests: await loadTestOutput()
        }
    }

    private func loadDetail() async {
        do {
            implementation = try await service.getImplementation(id: implementationId)
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    private func loadTimeline() async {
        do {
            timeline = try await service.getTimeline(id: implementationId, after: nil)
        } catch {
            // Timeline errors are non-fatal; may be empty for new implementations
        }
    }

    private func loadPlan() async {
        do {
            plan = try await service.getPlan(id: implementationId)
        } catch {
            // Plan may not exist yet
        }
    }

    private func loadChanges() async {
        do {
            changes = try await service.getChanges(id: implementationId)
        } catch {
            // Changes may not exist yet
        }
    }

    private func loadTestOutput() async {
        do {
            testReports = try await service.getTestOutput(id: implementationId)
        } catch {
            // Tests may not exist yet
        }
    }

    // MARK: - Polling

    func startPolling(interval: TimeInterval = 10) {
        pollingTask?.cancel()
        pollingTask = Task { [weak self] in
            while !Task.isCancelled {
                try? await Task.sleep(for: .seconds(interval))
                guard !Task.isCancelled else { break }
                await self?.refresh()
            }
        }
    }

    func stopPolling() {
        pollingTask?.cancel()
        pollingTask = nil
    }

    // MARK: - Prompt

    func sendPrompt() async {
        let message = promptText.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !message.isEmpty else { return }

        isSendingPrompt = true
        promptError = nil

        do {
            _ = try await service.sendPrompt(id: implementationId, message: message)
            promptText = ""
            // Refresh timeline to show the sent message
            await loadTimeline()
            await loadDetail()
        } catch let error as TervezoServiceError {
            switch error {
            case .conflict(let msg):
                promptError = "Cannot send: \(msg)"
            default:
                promptError = error.localizedDescription
            }
        } catch {
            promptError = error.localizedDescription
        }

        isSendingPrompt = false
    }

    // MARK: - Actions

    func createPR() async throws -> TervezoPRCreateResponse {
        let result = try await service.createPR(id: implementationId)
        await loadDetail()
        return result
    }

    func mergePR() async throws -> Bool {
        let result = try await service.mergePR(id: implementationId)
        await loadDetail()
        return result
    }

    func closePR() async throws -> Bool {
        let result = try await service.closePR(id: implementationId)
        await loadDetail()
        return result
    }

    func reopenPR() async throws -> Bool {
        let result = try await service.reopenPR(id: implementationId)
        await loadDetail()
        return result
    }

    func restart() async throws -> TervezoRestartResponse {
        let result = try await service.restart(id: implementationId)
        await loadDetail()
        return result
    }
}
