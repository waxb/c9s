import Foundation
import Observation

/// ViewModel for the Create Implementation form.
/// Fetches workspaces, validates form fields, and submits a new implementation.
@Observable
@MainActor
final class CreateImplementationVM {

    // MARK: - Form Fields

    var prompt = ""
    var selectedWorkspaceId: String?
    var repositoryName = ""
    var baseBranch = ""
    var selectedMode: ImplementationMode = .implement

    // MARK: - State

    var workspaces: [TervezoWorkspace] = []
    var isLoadingWorkspaces = false
    var isSubmitting = false
    var errorMessage: String?
    var createdImplementation: ImplementationDetail?

    // MARK: - Validation

    var isValid: Bool {
        !prompt.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
            && selectedWorkspaceId != nil
    }

    var promptValidationError: String? {
        guard !prompt.isEmpty else { return nil }
        let trimmed = prompt.trimmingCharacters(in: .whitespacesAndNewlines)
        if trimmed.count < 10 {
            return "Prompt should be at least 10 characters"
        }
        return nil
    }

    // MARK: - Types

    enum ImplementationMode: String, CaseIterable, Identifiable {
        case implement
        case bugfix
        case test

        var id: String { rawValue }

        var displayName: String {
            switch self {
            case .implement: "Implement"
            case .bugfix: "Bug Fix"
            case .test: "Test"
            }
        }

        var description: String {
            switch self {
            case .implement: "Build a new feature or make changes"
            case .bugfix: "Fix an existing bug or issue"
            case .test: "Write or improve tests"
            }
        }

        var icon: String {
            switch self {
            case .implement: "hammer"
            case .bugfix: "ladybug"
            case .test: "checkmark.circle"
            }
        }
    }

    // MARK: - Dependencies

    private let service: TervezoServiceProtocol

    init(service: TervezoServiceProtocol = TervezoService()) {
        self.service = service
    }

    // MARK: - Actions

    /// Fetch available workspaces from the API.
    func loadWorkspaces() async {
        isLoadingWorkspaces = true
        errorMessage = nil

        do {
            workspaces = try await service.listWorkspaces()
            // Auto-select if only one workspace
            if workspaces.count == 1 {
                selectedWorkspaceId = workspaces[0].id
            }
        } catch {
            errorMessage = "Failed to load workspaces: \(error.localizedDescription)"
        }

        isLoadingWorkspaces = false
    }

    /// Submit the form and create a new implementation.
    func submit() async {
        guard isValid else { return }
        guard let workspaceId = selectedWorkspaceId else { return }

        isSubmitting = true
        errorMessage = nil

        let trimmedPrompt = prompt.trimmingCharacters(in: .whitespacesAndNewlines)
        let repo = repositoryName.trimmingCharacters(in: .whitespacesAndNewlines)
        let branch = baseBranch.trimmingCharacters(in: .whitespacesAndNewlines)

        do {
            createdImplementation = try await service.createImplementation(
                prompt: trimmedPrompt,
                mode: selectedMode.rawValue,
                workspaceId: workspaceId,
                repositoryName: repo.isEmpty ? nil : repo,
                baseBranch: branch.isEmpty ? nil : branch
            )
        } catch {
            errorMessage = error.localizedDescription
        }

        isSubmitting = false
    }

    /// Reset the form to its initial state.
    func reset() {
        prompt = ""
        selectedWorkspaceId = workspaces.count == 1 ? workspaces[0].id : nil
        repositoryName = ""
        baseBranch = ""
        selectedMode = .implement
        errorMessage = nil
        createdImplementation = nil
    }
}
