import Testing
import Foundation
@testable import C9sLib

/// Tests for CreateImplementationVM form validation, workspace loading, and submission.
@Suite("CreateImplementationVM Tests")
@MainActor
struct CreateImplementationVMTests {

    // MARK: - Validation

    @Test("Form is invalid when prompt is empty")
    func invalidWhenPromptEmpty() {
        let mock = MockTervezoService()
        let vm = CreateImplementationVM(service: mock)
        vm.selectedWorkspaceId = "ws-1"
        vm.prompt = ""
        #expect(vm.isValid == false)
    }

    @Test("Form is invalid when workspace is not selected")
    func invalidWithoutWorkspace() {
        let mock = MockTervezoService()
        let vm = CreateImplementationVM(service: mock)
        vm.prompt = "Fix the login bug in authentication module"
        vm.selectedWorkspaceId = nil
        #expect(vm.isValid == false)
    }

    @Test("Form is valid with prompt and workspace")
    func validWithPromptAndWorkspace() {
        let mock = MockTervezoService()
        let vm = CreateImplementationVM(service: mock)
        vm.prompt = "Fix the login bug in authentication module"
        vm.selectedWorkspaceId = "ws-1"
        #expect(vm.isValid == true)
    }

    @Test("Whitespace-only prompt is invalid")
    func whitespacePromptInvalid() {
        let mock = MockTervezoService()
        let vm = CreateImplementationVM(service: mock)
        vm.prompt = "   \n  "
        vm.selectedWorkspaceId = "ws-1"
        #expect(vm.isValid == false)
    }

    @Test("Short prompt shows validation error")
    func shortPromptValidation() {
        let mock = MockTervezoService()
        let vm = CreateImplementationVM(service: mock)
        vm.prompt = "Fix bug"
        #expect(vm.promptValidationError != nil)
        #expect(vm.promptValidationError?.contains("10 characters") == true)
    }

    @Test("Long prompt has no validation error")
    func longPromptNoError() {
        let mock = MockTervezoService()
        let vm = CreateImplementationVM(service: mock)
        vm.prompt = "Fix the login authentication bug that prevents users from signing in"
        #expect(vm.promptValidationError == nil)
    }

    @Test("Empty prompt has no validation error (not yet touched)")
    func emptyPromptNoError() {
        let mock = MockTervezoService()
        let vm = CreateImplementationVM(service: mock)
        vm.prompt = ""
        #expect(vm.promptValidationError == nil)
    }

    // MARK: - Load Workspaces

    @Test("Loads workspaces from service")
    func loadWorkspaces() async {
        let mock = MockTervezoService()
        mock.listWorkspacesResult = .success([
            TestFixtures.makeWorkspace(id: "ws-1", name: "Workspace A"),
            TestFixtures.makeWorkspace(id: "ws-2", name: "Workspace B"),
        ])
        let vm = CreateImplementationVM(service: mock)

        await vm.loadWorkspaces()

        #expect(vm.workspaces.count == 2)
        #expect(vm.isLoadingWorkspaces == false)
        #expect(vm.errorMessage == nil)
    }

    @Test("Auto-selects single workspace")
    func autoSelectSingleWorkspace() async {
        let mock = MockTervezoService()
        mock.listWorkspacesResult = .success([
            TestFixtures.makeWorkspace(id: "ws-only", name: "Only Workspace"),
        ])
        let vm = CreateImplementationVM(service: mock)

        await vm.loadWorkspaces()

        #expect(vm.selectedWorkspaceId == "ws-only")
    }

    @Test("Does not auto-select with multiple workspaces")
    func noAutoSelectMultiple() async {
        let mock = MockTervezoService()
        mock.listWorkspacesResult = .success([
            TestFixtures.makeWorkspace(id: "ws-1", name: "A"),
            TestFixtures.makeWorkspace(id: "ws-2", name: "B"),
        ])
        let vm = CreateImplementationVM(service: mock)

        await vm.loadWorkspaces()

        #expect(vm.selectedWorkspaceId == nil)
    }

    @Test("Shows error when workspace loading fails")
    func workspaceLoadError() async {
        let mock = MockTervezoService()
        mock.listWorkspacesResult = .failure(TervezoServiceError.networkError("Connection failed"))
        let vm = CreateImplementationVM(service: mock)

        await vm.loadWorkspaces()

        #expect(vm.errorMessage != nil)
        #expect(vm.errorMessage?.contains("workspaces") == true)
        #expect(vm.workspaces.isEmpty)
    }

    // MARK: - Submit

    @Test("Successful submission sets createdImplementation")
    func submitSuccess() async {
        let mock = MockTervezoService()
        mock.listWorkspacesResult = .success([TestFixtures.makeWorkspace(id: "ws-1")])
        mock.createImplementationResult = .success(TestFixtures.makeImplementationDetail(id: "new-impl"))

        let vm = CreateImplementationVM(service: mock)
        vm.prompt = "Implement user profile page with avatar upload"
        vm.selectedWorkspaceId = "ws-1"
        vm.selectedMode = .implement

        await vm.submit()

        #expect(vm.createdImplementation != nil)
        #expect(vm.createdImplementation?.id == "new-impl")
        #expect(vm.isSubmitting == false)
        #expect(vm.errorMessage == nil)
    }

    @Test("Failed submission shows error")
    func submitFailure() async {
        let mock = MockTervezoService()
        mock.createImplementationResult = .failure(
            TervezoServiceError.httpError(statusCode: 422, message: "Invalid prompt")
        )

        let vm = CreateImplementationVM(service: mock)
        vm.prompt = "Implement user profile page with avatar upload"
        vm.selectedWorkspaceId = "ws-1"

        await vm.submit()

        #expect(vm.createdImplementation == nil)
        #expect(vm.errorMessage != nil)
        #expect(vm.isSubmitting == false)
    }

    @Test("Submit does nothing when form is invalid")
    func submitInvalid() async {
        let mock = MockTervezoService()
        mock.createImplementationResult = .success(TestFixtures.makeImplementationDetail())

        let vm = CreateImplementationVM(service: mock)
        vm.prompt = ""
        vm.selectedWorkspaceId = nil

        await vm.submit()

        #expect(vm.createdImplementation == nil)
    }

    // MARK: - Mode

    @Test("Default mode is implement")
    func defaultMode() {
        let mock = MockTervezoService()
        let vm = CreateImplementationVM(service: mock)
        #expect(vm.selectedMode == .implement)
        #expect(vm.selectedMode.rawValue == "implement")
    }

    @Test("All modes have display names and icons")
    func modeMetadata() {
        for mode in CreateImplementationVM.ImplementationMode.allCases {
            #expect(!mode.displayName.isEmpty)
            #expect(!mode.description.isEmpty)
            #expect(!mode.icon.isEmpty)
        }
    }

    // MARK: - Reset

    @Test("Reset clears all fields")
    func resetClearsFields() async {
        let mock = MockTervezoService()
        mock.listWorkspacesResult = .success([
            TestFixtures.makeWorkspace(id: "ws-1"),
            TestFixtures.makeWorkspace(id: "ws-2"),
        ])
        let vm = CreateImplementationVM(service: mock)
        await vm.loadWorkspaces()

        vm.prompt = "Some prompt text"
        vm.selectedWorkspaceId = "ws-1"
        vm.repositoryName = "my-repo"
        vm.baseBranch = "develop"
        vm.selectedMode = .bugfix
        vm.errorMessage = "Some error"

        vm.reset()

        #expect(vm.prompt.isEmpty)
        #expect(vm.selectedWorkspaceId == nil)
        #expect(vm.repositoryName.isEmpty)
        #expect(vm.baseBranch.isEmpty)
        #expect(vm.selectedMode == .implement)
        #expect(vm.errorMessage == nil)
    }

    @Test("Reset auto-selects when single workspace exists")
    func resetAutoSelectsSingle() async {
        let mock = MockTervezoService()
        mock.listWorkspacesResult = .success([
            TestFixtures.makeWorkspace(id: "ws-only"),
        ])
        let vm = CreateImplementationVM(service: mock)
        await vm.loadWorkspaces()

        vm.selectedWorkspaceId = nil
        vm.reset()

        #expect(vm.selectedWorkspaceId == "ws-only")
    }
}
