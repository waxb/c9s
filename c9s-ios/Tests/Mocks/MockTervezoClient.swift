import Foundation
@testable import C9sLib

/// Mock implementation of TervezoServiceProtocol for unit tests.
/// Configure response/error for each method before calling.
final class MockTervezoService: TervezoServiceProtocol, @unchecked Sendable {

    // MARK: - Configurable Responses

    var listImplementationsResult: Result<ImplementationList, Error> = .success(
        ImplementationList(items: [], total: 0)
    )
    var getImplementationResult: Result<ImplementationDetail, Error>?
    var getTimelineResult: Result<[TervezoTimelineMessage], Error> = .success([])
    var getPlanResult: Result<String, Error> = .success("")
    var getAnalysisResult: Result<String, Error> = .success("")
    var getChangesResult: Result<[TervezoFileChange], Error> = .success([])
    var getTestOutputResult: Result<[TervezoTestReport], Error> = .success([])
    var getSSHResult: Result<TervezoSSHCredentials, Error>?
    var getStatusResult: Result<TervezoStatusResponse, Error>?
    var sendPromptResult: Result<TervezoPromptResponse, Error> = .success(
        TervezoPromptResponse(sent: true, followUpId: nil)
    )
    var createPRResult: Result<TervezoPRCreateResponse, Error>?
    var getPRResult: Result<TervezoPRDetails, Error>?
    var mergePRResult: Result<Bool, Error> = .success(true)
    var closePRResult: Result<Bool, Error> = .success(true)
    var reopenPRResult: Result<Bool, Error> = .success(true)
    var restartResult: Result<TervezoRestartResponse, Error>?
    var listWorkspacesResult: Result<[TervezoWorkspace], Error> = .success([])
    var createImplementationResult: Result<ImplementationDetail, Error>?

    // MARK: - Call Tracking

    var listImplementationsCallCount = 0
    var lastListStatusFilter: String?
    var getImplementationCallCount = 0
    var lastGetImplementationId: String?
    var sendPromptCallCount = 0
    var lastSendPromptId: String?
    var lastSendPromptMessage: String?
    var createPRCallCount = 0
    var restartCallCount = 0
    var listWorkspacesCallCount = 0
    var createImplementationCallCount = 0
    var lastCreatePrompt: String?
    var lastCreateMode: String?
    var lastCreateWorkspaceId: String?
    var lastCreateRepositoryName: String?
    var lastCreateBaseBranch: String?

    // MARK: - Protocol Implementation

    func listImplementations(status: String?) async throws -> ImplementationList {
        listImplementationsCallCount += 1
        lastListStatusFilter = status
        return try listImplementationsResult.get()
    }

    func getImplementation(id: String) async throws -> ImplementationDetail {
        getImplementationCallCount += 1
        lastGetImplementationId = id
        guard let result = getImplementationResult else {
            throw TervezoServiceError.httpError(statusCode: 404, message: "Not found")
        }
        return try result.get()
    }

    func getTimeline(id: String, after: String?) async throws -> [TervezoTimelineMessage] {
        return try getTimelineResult.get()
    }

    func getPlan(id: String) async throws -> String {
        return try getPlanResult.get()
    }

    func getAnalysis(id: String) async throws -> String {
        return try getAnalysisResult.get()
    }

    func getChanges(id: String) async throws -> [TervezoFileChange] {
        return try getChangesResult.get()
    }

    func getTestOutput(id: String) async throws -> [TervezoTestReport] {
        return try getTestOutputResult.get()
    }

    func getSSH(id: String) async throws -> TervezoSSHCredentials {
        guard let result = getSSHResult else {
            throw TervezoServiceError.httpError(statusCode: 404, message: "Sandbox not available")
        }
        return try result.get()
    }

    func getStatus(id: String) async throws -> TervezoStatusResponse {
        guard let result = getStatusResult else {
            throw TervezoServiceError.httpError(statusCode: 404, message: "Not found")
        }
        return try result.get()
    }

    func sendPrompt(id: String, message: String) async throws -> TervezoPromptResponse {
        sendPromptCallCount += 1
        lastSendPromptId = id
        lastSendPromptMessage = message
        return try sendPromptResult.get()
    }

    func createPR(id: String) async throws -> TervezoPRCreateResponse {
        createPRCallCount += 1
        guard let result = createPRResult else {
            throw TervezoServiceError.httpError(statusCode: 400, message: "Cannot create PR")
        }
        return try result.get()
    }

    func getPR(id: String) async throws -> TervezoPRDetails {
        guard let result = getPRResult else {
            throw TervezoServiceError.httpError(statusCode: 404, message: "No PR")
        }
        return try result.get()
    }

    func mergePR(id: String) async throws -> Bool {
        return try mergePRResult.get()
    }

    func closePR(id: String) async throws -> Bool {
        return try closePRResult.get()
    }

    func reopenPR(id: String) async throws -> Bool {
        return try reopenPRResult.get()
    }

    func restart(id: String) async throws -> TervezoRestartResponse {
        restartCallCount += 1
        guard let result = restartResult else {
            throw TervezoServiceError.httpError(statusCode: 400, message: "Cannot restart")
        }
        return try result.get()
    }

    func listWorkspaces() async throws -> [TervezoWorkspace] {
        listWorkspacesCallCount += 1
        return try listWorkspacesResult.get()
    }

    func createImplementation(
        prompt: String,
        mode: String,
        workspaceId: String,
        repositoryName: String?,
        baseBranch: String?
    ) async throws -> ImplementationDetail {
        createImplementationCallCount += 1
        lastCreatePrompt = prompt
        lastCreateMode = mode
        lastCreateWorkspaceId = workspaceId
        lastCreateRepositoryName = repositoryName
        lastCreateBaseBranch = baseBranch
        guard let result = createImplementationResult else {
            throw TervezoServiceError.httpError(statusCode: 400, message: "Cannot create")
        }
        return try result.get()
    }
}

// MARK: - Test Fixtures

enum TestFixtures {
    static func makeImplementationSummary(
        id: String = "impl-123",
        title: String? = "Fix login bug",
        status: String = "running",
        mode: String = "bugfix"
    ) -> ImplementationSummary {
        ImplementationSummary(
            id: id,
            title: title,
            status: status,
            mode: mode,
            repoUrl: "https://github.com/user/repo",
            branch: "fix/login-bug",
            prUrl: nil,
            prStatus: nil,
            createdAt: Date(timeIntervalSince1970: 1709836800), // 2024-03-07
            updatedAt: Date(timeIntervalSince1970: 1709840400)
        )
    }

    static func makeImplementationDetail(
        id: String = "impl-123",
        title: String? = "Fix login bug",
        status: String = "running",
        mode: String = "bugfix"
    ) -> ImplementationDetail {
        ImplementationDetail(
            id: id,
            title: title,
            status: status,
            mode: mode,
            prompt: "Fix the login bug where users can't sign in",
            plan: "## Plan\n1. Investigate auth module\n2. Fix token validation",
            analysis: "The codebase uses JWT tokens for auth...",
            error: nil,
            isRunning: status == "running",
            repoUrl: "https://github.com/user/repo",
            branch: "fix/login-bug",
            baseBranch: "main",
            branchPushed: true,
            prUrl: nil,
            prStatus: nil,
            sandboxId: "sandbox-456",
            iterations: 3,
            currentIteration: 2,
            createdAt: Date(timeIntervalSince1970: 1709836800),
            updatedAt: Date(timeIntervalSince1970: 1709840400),
            steps: [
                TervezoStep(id: "step-1", name: "Planning", order: 1, status: "completed",
                           startedAt: Date(timeIntervalSince1970: 1709836800),
                           completedAt: Date(timeIntervalSince1970: 1709837400), error: nil),
                TervezoStep(id: "step-2", name: "Implementation", order: 2, status: "running",
                           startedAt: Date(timeIntervalSince1970: 1709837400),
                           completedAt: nil, error: nil),
            ],
            timelineMessageCount: 42
        )
    }

    static func makeWorkspace(
        id: String = "ws-789",
        name: String = "My Workspace"
    ) -> TervezoWorkspace {
        TervezoWorkspace(id: id, name: name, slug: "my-workspace", logo: nil)
    }

    static func makeSSHCredentials(
        host: String = "sandbox.tervezo.ai",
        port: Int = 2222,
        username: String = "user",
        sshCommand: String = "ssh -p 2222 user@sandbox.tervezo.ai",
        sandboxId: String = "sandbox-456",
        sandboxUrl: String = "https://sandbox.tervezo.ai/terminal/sandbox-456"
    ) -> TervezoSSHCredentials {
        TervezoSSHCredentials(
            host: host,
            port: port,
            username: username,
            sshCommand: sshCommand,
            sandboxId: sandboxId,
            sandboxUrl: sandboxUrl
        )
    }
}
