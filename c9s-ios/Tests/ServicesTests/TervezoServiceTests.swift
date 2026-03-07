import Testing
import Foundation
@testable import C9sLib

/// Tests for TervezoService URL construction, JSON decoding, and error handling.
/// Uses URLProtocol-based mocking to intercept HTTP requests.
@Suite("TervezoService Tests")
struct TervezoServiceTests {

    // MARK: - Mock URL Protocol

    /// Intercepts URLSession requests and returns configured responses.
    final class MockURLProtocol: URLProtocol, @unchecked Sendable {
        nonisolated(unsafe) static var requestHandler: ((URLRequest) throws -> (HTTPURLResponse, Data))?

        override class func canInit(with request: URLRequest) -> Bool { true }
        override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }

        override func startLoading() {
            guard let handler = Self.requestHandler else {
                client?.urlProtocolDidFinishLoading(self)
                return
            }
            do {
                let (response, data) = try handler(request)
                client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
                client?.urlProtocol(self, didLoad: data)
                client?.urlProtocolDidFinishLoading(self)
            } catch {
                client?.urlProtocol(self, didFailWithError: error)
            }
        }

        override func stopLoading() {}
    }

    /// Creates a TervezoService with a mock URLSession.
    private func makeService(apiKey: String = "tzv_test_key_123") -> TervezoService {
        let keychain = KeychainService.shared
        try? keychain.saveAPIKey(apiKey)
        try? keychain.deleteBaseURL() // Use default

        let config = URLSessionConfiguration.ephemeral
        config.protocolClasses = [MockURLProtocol.self]
        let session = URLSession(configuration: config)

        return TervezoService(keychain: keychain, session: session)
    }

    private func mockResponse(statusCode: Int = 200, json: [String: Any]) -> (HTTPURLResponse, Data) {
        let data = try! JSONSerialization.data(withJSONObject: json)
        let response = HTTPURLResponse(
            url: URL(string: "https://app.tervezo.ai/api/v1")!,
            statusCode: statusCode,
            httpVersion: nil,
            headerFields: ["Content-Type": "application/json"]
        )!
        return (response, data)
    }

    // MARK: - List Implementations

    @Test("List implementations returns parsed items")
    func listImplementationsSuccess() async throws {
        let service = makeService()

        MockURLProtocol.requestHandler = { request in
            #expect(request.url?.path.contains("implementations") == true)
            #expect(request.value(forHTTPHeaderField: "Authorization") == "Bearer tzv_test_key_123")

            return self.mockResponse(json: [
                "items": [
                    [
                        "id": "impl-1",
                        "title": "Fix bug",
                        "status": "running",
                        "mode": "bugfix",
                        "repoUrl": "https://github.com/user/repo",
                        "branch": "fix/bug",
                        "prUrl": NSNull(),
                        "prStatus": NSNull(),
                        "createdAt": "2024-03-07T12:00:00Z",
                        "updatedAt": "2024-03-07T13:00:00Z",
                    ]
                ],
                "total": 1,
            ])
        }

        let result = try await service.listImplementations(status: nil)
        #expect(result.items.count == 1)
        #expect(result.items[0].id == "impl-1")
        #expect(result.items[0].title == "Fix bug")
        #expect(result.items[0].status == "running")
        #expect(result.total == 1)
    }

    @Test("List implementations with status filter includes query parameter")
    func listImplementationsWithFilter() async throws {
        let service = makeService()

        MockURLProtocol.requestHandler = { request in
            #expect(request.url?.query?.contains("status=running") == true)
            return self.mockResponse(json: ["items": [], "total": 0])
        }

        let result = try await service.listImplementations(status: "running")
        #expect(result.items.isEmpty)
    }

    // MARK: - Get Implementation Detail

    @Test("Get implementation returns full detail")
    func getImplementationSuccess() async throws {
        let service = makeService()

        MockURLProtocol.requestHandler = { request in
            #expect(request.url?.path.contains("implementations/impl-42") == true)

            return self.mockResponse(json: [
                "id": "impl-42",
                "title": "Add auth",
                "status": "completed",
                "mode": "feature",
                "prompt": "Add OAuth2",
                "plan": "## Steps\n1. Add dependency",
                "analysis": "Current auth uses sessions",
                "error": NSNull(),
                "isRunning": false,
                "repoUrl": "https://github.com/user/repo",
                "branch": "feat/auth",
                "baseBranch": "main",
                "branchPushed": true,
                "prUrl": "https://github.com/user/repo/pull/5",
                "prStatus": "open",
                "sandboxId": "sb-1",
                "iterations": 2,
                "currentIteration": 2,
                "createdAt": "2024-03-07T12:00:00Z",
                "updatedAt": "2024-03-07T14:00:00Z",
                "steps": [
                    [
                        "id": "s1", "name": "Planning", "order": 1, "status": "completed",
                        "startedAt": "2024-03-07T12:00:00Z", "completedAt": "2024-03-07T12:05:00Z",
                        "error": NSNull(),
                    ]
                ],
                "timelineMessageCount": 15,
            ])
        }

        let detail = try await service.getImplementation(id: "impl-42")
        #expect(detail.id == "impl-42")
        #expect(detail.title == "Add auth")
        #expect(detail.status == "completed")
        #expect(detail.isRunning == false)
        #expect(detail.plan == "## Steps\n1. Add dependency")
        #expect(detail.steps.count == 1)
        #expect(detail.steps[0].name == "Planning")
        #expect(detail.timelineMessageCount == 15)
    }

    // MARK: - Timeline

    @Test("Get timeline parses multiple message types")
    func getTimelineSuccess() async throws {
        let service = makeService()

        MockURLProtocol.requestHandler = { _ in
            self.mockResponse(json: [
                "messages": [
                    [
                        "id": "msg-1", "type": "user_prompt",
                        "timestamp": "2024-03-07T12:00:00Z",
                        "text": "Fix the login bug",
                    ],
                    [
                        "id": "msg-2", "type": "assistant_text",
                        "timestamp": "2024-03-07T12:00:05Z",
                        "text": "I'll investigate the auth module.",
                    ],
                    [
                        "id": "msg-3", "type": "tool_call",
                        "timestamp": "2024-03-07T12:00:10Z",
                        "toolName": "Read",
                        "toolCallId": "tc-1",
                    ],
                    NSNull(), // null messages should be skipped
                ]
            ])
        }

        let messages = try await service.getTimeline(id: "impl-1", after: nil)
        #expect(messages.count == 3)
        #expect(messages[0].type == "user_prompt")
        #expect(messages[1].type == "assistant_text")
        #expect(messages[2].type == "tool_call")
    }

    // MARK: - Error Handling

    @Test("No API key throws noAPIKey error")
    func noAPIKeyError() async throws {
        let keychain = KeychainService.shared
        try keychain.deleteAPIKey()

        let config = URLSessionConfiguration.ephemeral
        config.protocolClasses = [MockURLProtocol.self]
        let session = URLSession(configuration: config)
        let service = TervezoService(keychain: keychain, session: session)

        do {
            _ = try await service.listImplementations(status: nil)
            #expect(Bool(false), "Should have thrown")
        } catch let error as TervezoServiceError {
            switch error {
            case .noAPIKey:
                break // Expected
            default:
                #expect(Bool(false), "Expected noAPIKey, got \(error)")
            }
        }
    }

    @Test("HTTP 409 throws conflict error")
    func conflictError() async throws {
        let service = makeService()

        MockURLProtocol.requestHandler = { _ in
            let data = Data("{\"error\": \"Implementation not waiting\"}".utf8)
            let response = HTTPURLResponse(
                url: URL(string: "https://app.tervezo.ai/api/v1/implementations/x/prompt")!,
                statusCode: 409,
                httpVersion: nil,
                headerFields: nil
            )!
            return (response, data)
        }

        do {
            _ = try await service.sendPrompt(id: "x", message: "hello")
            #expect(Bool(false), "Should have thrown")
        } catch let error as TervezoServiceError {
            switch error {
            case .conflict:
                break // Expected
            default:
                #expect(Bool(false), "Expected conflict, got \(error)")
            }
        }
    }

    @Test("HTTP 500 throws httpError")
    func serverError() async throws {
        let service = makeService()

        MockURLProtocol.requestHandler = { _ in
            let data = Data("{\"error\": \"Internal server error\"}".utf8)
            let response = HTTPURLResponse(
                url: URL(string: "https://app.tervezo.ai/api/v1/implementations")!,
                statusCode: 500,
                httpVersion: nil,
                headerFields: nil
            )!
            return (response, data)
        }

        do {
            _ = try await service.listImplementations(status: nil)
            #expect(Bool(false), "Should have thrown")
        } catch let error as TervezoServiceError {
            switch error {
            case .httpError(let code, _):
                #expect(code == 500)
            default:
                #expect(Bool(false), "Expected httpError, got \(error)")
            }
        }
    }

    // MARK: - Actions

    @Test("Send prompt returns success")
    func sendPromptSuccess() async throws {
        let service = makeService()

        MockURLProtocol.requestHandler = { request in
            #expect(request.httpMethod == "POST")
            #expect(request.url?.path.contains("prompt") == true)

            // Verify the request body contains the message
            if let body = request.httpBody,
               let json = try? JSONSerialization.jsonObject(with: body) as? [String: Any] {
                #expect(json["message"] as? String == "Fix this please")
            }

            return self.mockResponse(json: ["sent": true])
        }

        let result = try await service.sendPrompt(id: "impl-1", message: "Fix this please")
        #expect(result.sent == true)
    }

    @Test("Create PR returns URL and number")
    func createPRSuccess() async throws {
        let service = makeService()

        MockURLProtocol.requestHandler = { _ in
            self.mockResponse(json: [
                "prUrl": "https://github.com/user/repo/pull/42",
                "prNumber": 42,
            ])
        }

        let result = try await service.createPR(id: "impl-1")
        #expect(result.prUrl == "https://github.com/user/repo/pull/42")
        #expect(result.prNumber == 42)
    }

    // MARK: - Workspaces

    @Test("List workspaces returns parsed items")
    func listWorkspacesSuccess() async throws {
        let service = makeService()

        MockURLProtocol.requestHandler = { _ in
            self.mockResponse(json: [
                "items": [
                    ["id": "ws-1", "name": "Team Alpha", "slug": "team-alpha", "logo": NSNull()],
                    ["id": "ws-2", "name": "Team Beta", "slug": "team-beta", "logo": "https://example.com/logo.png"],
                ]
            ])
        }

        let workspaces = try await service.listWorkspaces()
        #expect(workspaces.count == 2)
        #expect(workspaces[0].name == "Team Alpha")
        #expect(workspaces[0].logo == nil)
        #expect(workspaces[1].logo == "https://example.com/logo.png")
    }

    // MARK: - Changes

    @Test("Get changes parses file diffs")
    func getChangesSuccess() async throws {
        let service = makeService()

        MockURLProtocol.requestHandler = { _ in
            self.mockResponse(json: [
                "files": [
                    [
                        "filename": "src/auth.rs",
                        "status": "modified",
                        "additions": 15,
                        "deletions": 3,
                        "changes": 18,
                        "patch": "@@ -10,3 +10,15 @@\n+new code here",
                    ]
                ]
            ])
        }

        let changes = try await service.getChanges(id: "impl-1")
        #expect(changes.count == 1)
        #expect(changes[0].filename == "src/auth.rs")
        #expect(changes[0].additions == 15)
        #expect(changes[0].deletions == 3)
        #expect(changes[0].patch != nil)
    }

    // MARK: - SSH Credentials

    @Test("Get SSH credentials parses all fields")
    func getSSHSuccess() async throws {
        let service = makeService()

        MockURLProtocol.requestHandler = { _ in
            self.mockResponse(json: [
                "host": "sandbox.tervezo.ai",
                "port": 22,
                "username": "user",
                "sshCommand": "ssh user@sandbox.tervezo.ai -p 22",
                "sandboxId": "sb-123",
                "sandboxUrl": "https://sandbox.tervezo.ai/sb-123",
            ])
        }

        let creds = try await service.getSSH(id: "impl-1")
        #expect(creds.host == "sandbox.tervezo.ai")
        #expect(creds.port == 22)
        #expect(creds.username == "user")
    }
}
