import Testing
import Foundation
import SwiftData
@testable import C9sLib

/// Integration tests that simulate realistic multi-step user journeys through the app.
/// Uses MockURLProtocol to intercept HTTP requests and return configured responses,
/// testing how multiple service calls chain together in end-to-end flows.
///
/// Why these tests matter: Unit tests verify individual functions in isolation.
/// These integration tests verify that the service layer, ViewModels, and cache
/// work together correctly across multi-step operations — catching issues like
/// incorrect state transitions, data inconsistencies between API calls, and
/// error handling gaps in real user scenarios.
@Suite("Integration Tests")
struct IntegrationTests {

    // MARK: - Mock URL Protocol

    /// Intercepts all URLSession requests in the test session.
    /// Supports request-by-request response sequencing for multi-step flows.
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

    // MARK: - Helpers

    private func makeService(apiKey: String = "tzv_test_integration_key") -> TervezoService {
        let keychain = KeychainService.shared
        try? keychain.saveAPIKey(apiKey)
        try? keychain.deleteBaseURL()

        let config = URLSessionConfiguration.ephemeral
        config.protocolClasses = [MockURLProtocol.self]
        let session = URLSession(configuration: config)

        return TervezoService(keychain: keychain, session: session)
    }

    private func makeJSON(_ dict: [String: Any]) -> Data {
        try! JSONSerialization.data(withJSONObject: dict)
    }

    private func httpResponse(statusCode: Int = 200) -> HTTPURLResponse {
        HTTPURLResponse(
            url: URL(string: "https://app.tervezo.ai/api/v1")!,
            statusCode: statusCode,
            httpVersion: nil,
            headerFields: ["Content-Type": "application/json"]
        )!
    }

    // MARK: - Test Data Factories

    private func listResponseJSON(implementations: [[String: Any]]) -> [String: Any] {
        ["items": implementations, "total": implementations.count]
    }

    private func implementationJSON(
        id: String,
        title: String,
        status: String,
        mode: String = "feature",
        branch: String? = nil,
        prUrl: String? = nil,
        prStatus: String? = nil
    ) -> [String: Any] {
        var json: [String: Any] = [
            "id": id,
            "title": title,
            "status": status,
            "mode": mode,
            "repoUrl": "https://github.com/test/repo",
            "branch": branch ?? NSNull(),
            "prUrl": prUrl ?? NSNull(),
            "prStatus": prStatus ?? NSNull(),
            "createdAt": "2026-03-07T10:00:00Z",
            "updatedAt": "2026-03-07T12:00:00Z",
        ]
        // Ensure NSNull for nil values
        if branch == nil { json["branch"] = NSNull() }
        if prUrl == nil { json["prUrl"] = NSNull() }
        if prStatus == nil { json["prStatus"] = NSNull() }
        return json
    }

    private func detailJSON(
        id: String,
        title: String,
        status: String,
        mode: String = "feature",
        plan: String? = nil,
        analysis: String? = nil,
        error: String? = nil,
        isRunning: Bool = false,
        branch: String? = "feat/test",
        prUrl: String? = nil,
        sandboxId: String? = nil,
        steps: [[String: Any]] = []
    ) -> [String: Any] {
        [
            "id": id,
            "title": title,
            "status": status,
            "mode": mode,
            "prompt": "Test prompt for \(title)",
            "plan": plan ?? NSNull(),
            "analysis": analysis ?? NSNull(),
            "error": error ?? NSNull(),
            "isRunning": isRunning,
            "repoUrl": "https://github.com/test/repo",
            "branch": branch ?? NSNull(),
            "baseBranch": "main",
            "branchPushed": true,
            "prUrl": prUrl ?? NSNull(),
            "prStatus": prUrl != nil ? "open" : NSNull(),
            "sandboxId": sandboxId ?? NSNull(),
            "iterations": 2,
            "currentIteration": 1,
            "createdAt": "2026-03-07T10:00:00Z",
            "updatedAt": "2026-03-07T12:00:00Z",
            "steps": steps,
            "timelineMessageCount": 10,
        ]
    }

    // MARK: - Flow 1: List → Detail → Timeline

    @Test("Full browse flow: list implementations, select one, fetch detail and timeline")
    func listDetailTimelineFlow() async throws {
        let service = makeService()
        var requestCount = 0

        MockURLProtocol.requestHandler = { request in
            requestCount += 1
            let path = request.url?.path ?? ""

            // Step 1: List implementations
            if path.hasSuffix("/implementations") && request.httpMethod == "GET" && !path.contains("/impl-") {
                return (self.httpResponse(), self.makeJSON(self.listResponseJSON(implementations: [
                    self.implementationJSON(id: "impl-browse-1", title: "Add dark mode", status: "completed", branch: "feat/dark-mode"),
                    self.implementationJSON(id: "impl-browse-2", title: "Fix auth bug", status: "running", branch: "fix/auth"),
                    self.implementationJSON(id: "impl-browse-3", title: "Add search", status: "failed"),
                ])))
            }

            // Step 2: Get detail for selected implementation
            if path.contains("/implementations/impl-browse-2") && !path.contains("timeline") {
                return (self.httpResponse(), self.makeJSON(self.detailJSON(
                    id: "impl-browse-2",
                    title: "Fix auth bug",
                    status: "running",
                    mode: "bugfix",
                    plan: "## Plan\n1. Debug token validation\n2. Fix refresh flow",
                    analysis: "Uses JWT with 1h expiry",
                    isRunning: true,
                    branch: "fix/auth",
                    sandboxId: "sb-browse-1",
                    steps: [
                        ["id": "s1", "name": "Planning", "order": 1, "status": "completed",
                         "startedAt": "2026-03-07T10:00:00Z", "completedAt": "2026-03-07T10:05:00Z", "error": NSNull()],
                        ["id": "s2", "name": "Implementation", "order": 2, "status": "running",
                         "startedAt": "2026-03-07T10:05:00Z", "completedAt": NSNull(), "error": NSNull()],
                    ]
                )))
            }

            // Step 3: Get timeline
            if path.contains("/implementations/impl-browse-2/timeline") {
                return (self.httpResponse(), self.makeJSON([
                    "messages": [
                        ["id": "msg-1", "type": "user_prompt", "timestamp": "2026-03-07T10:00:00Z",
                         "text": "Fix the auth token refresh bug"],
                        ["id": "msg-2", "type": "assistant_text", "timestamp": "2026-03-07T10:00:05Z",
                         "text": "I'll investigate the token validation logic."],
                        ["id": "msg-3", "type": "tool_call", "timestamp": "2026-03-07T10:00:10Z",
                         "toolName": "Read", "toolCallId": "tc-1"],
                        ["id": "msg-4", "type": "file_change", "timestamp": "2026-03-07T10:01:00Z",
                         "filename": "src/auth/token.rs", "additions": 15, "deletions": 3],
                    ]
                ]))
            }

            throw TervezoServiceError.httpError(statusCode: 404, message: "Not found: \(path)")
        }

        // Execute flow
        let list = try await service.listImplementations(status: nil)
        #expect(list.items.count == 3)
        #expect(list.items[1].id == "impl-browse-2")
        #expect(list.items[1].status == "running")

        let detail = try await service.getImplementation(id: "impl-browse-2")
        #expect(detail.title == "Fix auth bug")
        #expect(detail.isRunning == true)
        #expect(detail.steps.count == 2)
        #expect(detail.steps[0].status == "completed")
        #expect(detail.steps[1].status == "running")
        #expect(detail.plan?.contains("Debug token") == true)
        #expect(detail.sandboxId == "sb-browse-1")

        let timeline = try await service.getTimeline(id: "impl-browse-2", after: nil)
        #expect(timeline.count == 4)
        #expect(timeline[0].type == "user_prompt")
        #expect(timeline[3].type == "file_change")

        #expect(requestCount == 3)
    }

    // MARK: - Flow 2: Create Implementation → Navigate to Detail

    @Test("Create implementation flow: fetch workspaces, create, navigate to detail")
    func createImplementationFlow() async throws {
        let service = makeService()
        var requestPaths: [String] = []

        MockURLProtocol.requestHandler = { request in
            let path = request.url?.path ?? ""
            requestPaths.append("\(request.httpMethod ?? "?") \(path)")

            // Step 1: List workspaces
            if path.hasSuffix("/workspaces") {
                return (self.httpResponse(), self.makeJSON([
                    "items": [
                        ["id": "ws-1", "name": "Production", "slug": "production", "logo": NSNull()],
                        ["id": "ws-2", "name": "Staging", "slug": "staging", "logo": NSNull()],
                    ]
                ]))
            }

            // Step 2: Create implementation
            if path.hasSuffix("/implementations") && request.httpMethod == "POST" {
                // Verify request body
                if let body = request.httpBody,
                   let json = try? JSONSerialization.jsonObject(with: body) as? [String: Any] {
                    // Validate required fields were sent
                    guard json["prompt"] != nil, json["workspaceId"] != nil else {
                        return (self.httpResponse(statusCode: 400), self.makeJSON(["error": "Missing required fields"]))
                    }
                }

                return (self.httpResponse(statusCode: 201), self.makeJSON(self.detailJSON(
                    id: "impl-new-1",
                    title: "Add user dashboard",
                    status: "pending",
                    plan: nil,
                    isRunning: false,
                    branch: nil
                )))
            }

            // Step 3: Fetch the newly created implementation detail
            if path.contains("/implementations/impl-new-1") {
                return (self.httpResponse(), self.makeJSON(self.detailJSON(
                    id: "impl-new-1",
                    title: "Add user dashboard",
                    status: "running",
                    plan: "## Plan\n1. Create dashboard component",
                    isRunning: true,
                    branch: "feat/user-dashboard",
                    sandboxId: "sb-new-1",
                    steps: [
                        ["id": "s1", "name": "Planning", "order": 1, "status": "running",
                         "startedAt": "2026-03-07T12:00:00Z", "completedAt": NSNull(), "error": NSNull()],
                    ]
                )))
            }

            throw TervezoServiceError.httpError(statusCode: 404, message: "Not found: \(path)")
        }

        // Step 1: Fetch workspaces
        let workspaces = try await service.listWorkspaces()
        #expect(workspaces.count == 2)
        #expect(workspaces[0].name == "Production")

        // Step 2: Create implementation
        let created = try await service.createImplementation(
            prompt: "Build a user dashboard with analytics charts",
            mode: "feature",
            workspaceId: "ws-1",
            repositoryName: nil,
            baseBranch: nil
        )
        #expect(created.id == "impl-new-1")
        #expect(created.status == "pending")

        // Step 3: Navigate to detail (re-fetch to get latest state)
        let detail = try await service.getImplementation(id: created.id)
        #expect(detail.status == "running")
        #expect(detail.plan != nil)
        #expect(detail.branch == "feat/user-dashboard")

        // Verify correct request sequence
        #expect(requestPaths.count == 3)
        #expect(requestPaths[0].contains("workspaces"))
        #expect(requestPaths[1].contains("POST"))
        #expect(requestPaths[2].contains("impl-new-1"))
    }

    // MARK: - Flow 3: PR Lifecycle

    @Test("PR lifecycle: create PR, check details, merge")
    func prLifecycleFlow() async throws {
        let service = makeService()
        var step = 0

        MockURLProtocol.requestHandler = { request in
            let path = request.url?.path ?? ""
            step += 1

            // Step 1: Get implementation detail (no PR yet)
            if step == 1 && path.contains("/implementations/impl-pr-1") && !path.contains("/pr") {
                return (self.httpResponse(), self.makeJSON(self.detailJSON(
                    id: "impl-pr-1",
                    title: "Add OAuth",
                    status: "completed",
                    branch: "feat/oauth"
                )))
            }

            // Step 2: Create PR
            if step == 2 && path.contains("/pr") && request.httpMethod == "POST" && !path.contains("merge") {
                return (self.httpResponse(statusCode: 201), self.makeJSON([
                    "prUrl": "https://github.com/test/repo/pull/42",
                    "prNumber": 42,
                ]))
            }

            // Step 3: Get PR details
            if step == 3 && path.contains("/pr") && request.httpMethod == "GET" {
                return (self.httpResponse(), self.makeJSON([
                    "url": "https://github.com/test/repo/pull/42",
                    "number": 42,
                    "status": "open",
                    "title": "feat: Add OAuth",
                    "mergeable": true,
                    "merged": false,
                    "draft": false,
                ]))
            }

            // Step 4: Merge PR
            if step == 4 && path.contains("/pr/merge") && request.httpMethod == "POST" {
                return (self.httpResponse(), self.makeJSON(["success": true]))
            }

            // Step 5: Refresh implementation detail (now merged)
            if step == 5 && path.contains("/implementations/impl-pr-1") {
                return (self.httpResponse(), self.makeJSON(self.detailJSON(
                    id: "impl-pr-1",
                    title: "Add OAuth",
                    status: "merged",
                    branch: "feat/oauth",
                    prUrl: "https://github.com/test/repo/pull/42"
                )))
            }

            throw TervezoServiceError.httpError(statusCode: 404, message: "Unexpected request step \(step): \(path)")
        }

        // Step 1: View implementation
        let detail = try await service.getImplementation(id: "impl-pr-1")
        #expect(detail.prUrl == nil)
        #expect(detail.status == "completed")

        // Step 2: Create PR
        let pr = try await service.createPR(id: "impl-pr-1")
        #expect(pr.prUrl == "https://github.com/test/repo/pull/42")
        #expect(pr.prNumber == 42)

        // Step 3: Check PR details
        let prDetails = try await service.getPR(id: "impl-pr-1")
        #expect(prDetails.mergeable == true)
        #expect(prDetails.merged == false)
        #expect(prDetails.status == "open")

        // Step 4: Merge PR
        let merged = try await service.mergePR(id: "impl-pr-1")
        #expect(merged == true)

        // Step 5: Refresh — now shows merged status
        let refreshed = try await service.getImplementation(id: "impl-pr-1")
        #expect(refreshed.status == "merged")
        #expect(refreshed.prUrl != nil)
    }

    // MARK: - Flow 4: Prompt Interaction

    @Test("Prompt flow: attempt on non-waiting implementation (409), then succeed when waiting")
    func promptInteractionFlow() async throws {
        let service = makeService()
        var step = 0

        MockURLProtocol.requestHandler = { request in
            let path = request.url?.path ?? ""
            step += 1

            // Step 1: Check status — not waiting
            if step == 1 && path.contains("/status") {
                return (self.httpResponse(), self.makeJSON([
                    "status": "running",
                    "waitingForInput": false,
                    "currentStepName": "Implementation",
                    "steps": [
                        ["name": "Planning", "order": 1, "status": "completed"],
                        ["name": "Implementation", "order": 2, "status": "running"],
                    ],
                ]))
            }

            // Step 2: Try to send prompt — 409 because not waiting
            if step == 2 && path.contains("/prompt") {
                return (self.httpResponse(statusCode: 409),
                        Data("{\"error\": \"Implementation is not waiting for input\"}".utf8))
            }

            // Step 3: Check status again — now waiting
            if step == 3 && path.contains("/status") {
                return (self.httpResponse(), self.makeJSON([
                    "status": "running",
                    "waitingForInput": true,
                    "currentStepName": "Implementation",
                    "steps": [
                        ["name": "Planning", "order": 1, "status": "completed"],
                        ["name": "Implementation", "order": 2, "status": "running"],
                    ],
                ]))
            }

            // Step 4: Send prompt — success
            if step == 4 && path.contains("/prompt") {
                if let body = request.httpBody,
                   let json = try? JSONSerialization.jsonObject(with: body) as? [String: Any] {
                    let message = json["message"] as? String ?? ""
                    #expect(message == "Use OAuth2 instead of SAML")
                }
                return (self.httpResponse(), self.makeJSON([
                    "sent": true,
                    "followUpId": "fu-123",
                ]))
            }

            throw TervezoServiceError.httpError(statusCode: 404, message: "Unexpected: step \(step)")
        }

        // Step 1: Check status
        let status1 = try await service.getStatus(id: "impl-prompt-1")
        #expect(status1.waitingForInput == false)

        // Step 2: Try to send prompt — should get 409 conflict
        do {
            _ = try await service.sendPrompt(id: "impl-prompt-1", message: "test")
            Issue.record("Should have thrown conflict error")
        } catch let error as TervezoServiceError {
            switch error {
            case .conflict:
                break // Expected
            default:
                Issue.record("Expected conflict, got \(error)")
            }
        }

        // Step 3: Status changes to waiting
        let status2 = try await service.getStatus(id: "impl-prompt-1")
        #expect(status2.waitingForInput == true)

        // Step 4: Now prompt succeeds
        let response = try await service.sendPrompt(id: "impl-prompt-1", message: "Use OAuth2 instead of SAML")
        #expect(response.sent == true)
        #expect(response.followUpId == "fu-123")
    }

    // MARK: - Flow 5: Error Recovery

    @Test("Error recovery: server error then successful retry")
    func errorRecoveryFlow() async throws {
        let service = makeService()
        var attempt = 0

        MockURLProtocol.requestHandler = { request in
            attempt += 1

            // First attempt: 500 server error
            if attempt == 1 {
                return (self.httpResponse(statusCode: 500),
                        Data("{\"error\": \"Internal server error\"}".utf8))
            }

            // Second attempt: success
            if attempt == 2 {
                return (self.httpResponse(), self.makeJSON(self.listResponseJSON(implementations: [
                    self.implementationJSON(id: "impl-retry-1", title: "Recovered impl", status: "running"),
                ])))
            }

            throw TervezoServiceError.httpError(statusCode: 500, message: "Unexpected attempt \(attempt)")
        }

        // First attempt fails
        do {
            _ = try await service.listImplementations(status: nil)
            Issue.record("Should have thrown")
        } catch let error as TervezoServiceError {
            switch error {
            case .httpError(let code, _):
                #expect(code == 500)
            default:
                Issue.record("Expected httpError, got \(error)")
            }
        }

        // Retry succeeds
        let list = try await service.listImplementations(status: nil)
        #expect(list.items.count == 1)
        #expect(list.items[0].title == "Recovered impl")
    }

    // MARK: - Flow 6: Multi-Endpoint Detail Enrichment

    @Test("Detail enrichment: fetch detail, then load changes, tests, and SSH in parallel")
    func detailEnrichmentFlow() async throws {
        let service = makeService()

        MockURLProtocol.requestHandler = { request in
            let path = request.url?.path ?? ""

            // Implementation detail
            if path.hasSuffix("/implementations/impl-enrich-1") {
                return (self.httpResponse(), self.makeJSON(self.detailJSON(
                    id: "impl-enrich-1",
                    title: "Add pagination",
                    status: "completed",
                    plan: "## Plan\n1. Add cursor-based pagination",
                    branch: "feat/pagination",
                    prUrl: "https://github.com/test/repo/pull/10",
                    sandboxId: "sb-enrich-1"
                )))
            }

            // Changes
            if path.contains("/changes") {
                return (self.httpResponse(), self.makeJSON([
                    "files": [
                        ["filename": "src/api/list.rs", "status": "modified",
                         "additions": 45, "deletions": 12, "changes": 57,
                         "patch": "@@ -1,12 +1,45 @@\n+pagination logic"],
                        ["filename": "src/models/cursor.rs", "status": "added",
                         "additions": 30, "deletions": 0, "changes": 30,
                         "patch": "@@ -0,0 +1,30 @@\n+new file"],
                        ["filename": "tests/api_test.rs", "status": "modified",
                         "additions": 20, "deletions": 5, "changes": 25, "patch": NSNull()],
                    ]
                ]))
            }

            // Test output
            if path.contains("/test-output") {
                return (self.httpResponse(), self.makeJSON([
                    "testReports": [
                        [
                            "id": "tr-1",
                            "timestamp": "2026-03-07T12:30:00Z",
                            "summary": [
                                "status": "passing",
                                "message": "All 42 tests passed",
                                "stats": [
                                    "newTests": 5,
                                    "totalBefore": 37,
                                    "totalAfter": 42,
                                ],
                            ],
                            "approach": "Added cursor pagination tests",
                        ]
                    ]
                ]))
            }

            // SSH credentials
            if path.contains("/ssh") {
                return (self.httpResponse(), self.makeJSON([
                    "host": "sandbox.tervezo.ai",
                    "port": 2222,
                    "username": "dev",
                    "sshCommand": "ssh -p 2222 dev@sandbox.tervezo.ai",
                    "sandboxId": "sb-enrich-1",
                    "sandboxUrl": "https://sandbox.tervezo.ai/terminal/sb-enrich-1",
                ]))
            }

            throw TervezoServiceError.httpError(statusCode: 404, message: "Not found: \(path)")
        }

        // Fetch detail first
        let detail = try await service.getImplementation(id: "impl-enrich-1")
        #expect(detail.status == "completed")
        #expect(detail.sandboxId == "sb-enrich-1")

        // Fetch supplementary data (would be parallel in the app)
        let changes = try await service.getChanges(id: "impl-enrich-1")
        let tests = try await service.getTestOutput(id: "impl-enrich-1")
        let ssh = try await service.getSSH(id: "impl-enrich-1")

        // Verify changes
        #expect(changes.count == 3)
        let totalAdditions = changes.reduce(0) { $0 + $1.additions }
        let totalDeletions = changes.reduce(0) { $0 + $1.deletions }
        #expect(totalAdditions == 95)
        #expect(totalDeletions == 17)
        #expect(changes[1].status == "added")

        // Verify test output
        #expect(tests.count == 1)
        #expect(tests[0].summaryStatus == "passing")
        #expect(tests[0].newTests == 5)
        #expect(tests[0].totalAfter == 42)

        // Verify SSH
        #expect(ssh.port == 2222)
        #expect(ssh.sandboxId == "sb-enrich-1")
    }

    // MARK: - Flow 7: Restart Failed Implementation

    @Test("Restart flow: view failed impl, restart, verify new state")
    func restartFailedImplementation() async throws {
        let service = makeService()
        var step = 0

        MockURLProtocol.requestHandler = { request in
            let path = request.url?.path ?? ""
            step += 1

            // Step 1: Get failed implementation
            if step == 1 && path.contains("/implementations/impl-fail-1") && !path.contains("restart") {
                return (self.httpResponse(), self.makeJSON(self.detailJSON(
                    id: "impl-fail-1",
                    title: "Add search feature",
                    status: "failed",
                    error: "Test suite failed: 3 tests failing in search_tests.rs",
                    branch: "feat/search"
                )))
            }

            // Step 2: Restart
            if step == 2 && path.contains("/restart") && request.httpMethod == "POST" {
                return (self.httpResponse(), self.makeJSON([
                    "implementationId": "impl-fail-1",
                    "isNewImplementation": false,
                ]))
            }

            // Step 3: Refresh — now running again
            if step == 3 && path.contains("/implementations/impl-fail-1") {
                return (self.httpResponse(), self.makeJSON(self.detailJSON(
                    id: "impl-fail-1",
                    title: "Add search feature",
                    status: "running",
                    isRunning: true,
                    branch: "feat/search",
                    sandboxId: "sb-restart-1",
                    steps: [
                        ["id": "s1", "name": "Fix Tests", "order": 1, "status": "running",
                         "startedAt": "2026-03-07T14:00:00Z", "completedAt": NSNull(), "error": NSNull()],
                    ]
                )))
            }

            throw TervezoServiceError.httpError(statusCode: 404, message: "Unexpected step \(step)")
        }

        // View failed implementation
        let failed = try await service.getImplementation(id: "impl-fail-1")
        #expect(failed.status == "failed")
        #expect(failed.error?.contains("3 tests failing") == true)

        // Restart it
        let restart = try await service.restart(id: "impl-fail-1")
        #expect(restart.implementationId == "impl-fail-1")
        #expect(restart.isNewImplementation == false)

        // Refresh — should be running again
        let refreshed = try await service.getImplementation(id: "impl-fail-1")
        #expect(refreshed.status == "running")
        #expect(refreshed.isRunning == true)
        #expect(refreshed.error == nil)
    }

    // MARK: - Flow 8: Authentication Boundary

    @Test("Auth boundary: no API key returns clear error, valid key succeeds")
    func authBoundaryFlow() async throws {
        let keychain = KeychainService.shared

        let config = URLSessionConfiguration.ephemeral
        config.protocolClasses = [MockURLProtocol.self]
        let session = URLSession(configuration: config)

        MockURLProtocol.requestHandler = { request in
            let auth = request.value(forHTTPHeaderField: "Authorization") ?? ""
            if !auth.hasPrefix("Bearer tzv_") {
                return (self.httpResponse(statusCode: 401),
                        Data("{\"error\": \"Unauthorized\"}".utf8))
            }
            return (self.httpResponse(), self.makeJSON(self.listResponseJSON(implementations: [
                self.implementationJSON(id: "impl-auth-1", title: "Authed impl", status: "running"),
            ])))
        }

        // Without API key — should throw noAPIKey before even making request
        try keychain.deleteAPIKey()
        let serviceNoKey = TervezoService(keychain: keychain, session: session)
        do {
            _ = try await serviceNoKey.listImplementations(status: nil)
            Issue.record("Should have thrown noAPIKey")
        } catch let error as TervezoServiceError {
            switch error {
            case .noAPIKey:
                break // Expected
            default:
                Issue.record("Expected noAPIKey, got \(error)")
            }
        }

        // With valid key — succeeds
        try keychain.saveAPIKey("tzv_valid_test_key")
        let serviceWithKey = TervezoService(keychain: keychain, session: session)
        let list = try await serviceWithKey.listImplementations(status: nil)
        #expect(list.items.count == 1)
        #expect(list.items[0].title == "Authed impl")
    }

    // MARK: - Flow 9: List Filtering and Status Transitions

    @Test("Status filter flow: fetch all, then filter by running, completed, failed")
    func statusFilterFlow() async throws {
        let service = makeService()

        MockURLProtocol.requestHandler = { request in
            let statusParam = request.url?.query?.split(separator: "=").last.map(String.init)
            let allImpls = [
                self.implementationJSON(id: "impl-f1", title: "Running task", status: "running"),
                self.implementationJSON(id: "impl-f2", title: "Completed task", status: "completed"),
                self.implementationJSON(id: "impl-f3", title: "Failed task", status: "failed"),
                self.implementationJSON(id: "impl-f4", title: "Queued task", status: "queued"),
            ]

            // Simulate server-side filtering
            let filtered: [[String: Any]]
            if let statusParam {
                filtered = allImpls.filter { ($0["status"] as? String) == statusParam }
            } else {
                filtered = allImpls
            }

            return (self.httpResponse(), self.makeJSON(self.listResponseJSON(implementations: filtered)))
        }

        // All implementations
        let all = try await service.listImplementations(status: nil)
        #expect(all.items.count == 4)

        // Only running
        let running = try await service.listImplementations(status: "running")
        #expect(running.items.count == 1)
        #expect(running.items[0].status == "running")

        // Only completed
        let completed = try await service.listImplementations(status: "completed")
        #expect(completed.items.count == 1)
        #expect(completed.items[0].title == "Completed task")

        // Only failed
        let failed = try await service.listImplementations(status: "failed")
        #expect(failed.items.count == 1)
        #expect(failed.items[0].title == "Failed task")
    }

    // MARK: - Flow 10: Close and Reopen PR

    @Test("PR close/reopen flow: close an open PR, then reopen it")
    func prCloseReopenFlow() async throws {
        let service = makeService()
        var step = 0

        MockURLProtocol.requestHandler = { request in
            let path = request.url?.path ?? ""
            step += 1

            // Step 1: Get PR details (open)
            if step == 1 && path.contains("/pr") && request.httpMethod == "GET" {
                return (self.httpResponse(), self.makeJSON([
                    "url": "https://github.com/test/repo/pull/7",
                    "number": 7,
                    "status": "open",
                    "title": "feat: Add caching",
                    "mergeable": true,
                    "merged": false,
                    "draft": false,
                ]))
            }

            // Step 2: Close PR
            if step == 2 && path.contains("/pr/close") {
                return (self.httpResponse(), self.makeJSON(["success": true]))
            }

            // Step 3: Get PR details (closed)
            if step == 3 && path.contains("/pr") && request.httpMethod == "GET" {
                return (self.httpResponse(), self.makeJSON([
                    "url": "https://github.com/test/repo/pull/7",
                    "number": 7,
                    "status": "closed",
                    "title": "feat: Add caching",
                    "mergeable": false,
                    "merged": false,
                    "draft": false,
                ]))
            }

            // Step 4: Reopen PR
            if step == 4 && path.contains("/pr/reopen") {
                return (self.httpResponse(), self.makeJSON(["success": true]))
            }

            // Step 5: Get PR details (open again)
            if step == 5 && path.contains("/pr") && request.httpMethod == "GET" {
                return (self.httpResponse(), self.makeJSON([
                    "url": "https://github.com/test/repo/pull/7",
                    "number": 7,
                    "status": "open",
                    "title": "feat: Add caching",
                    "mergeable": true,
                    "merged": false,
                    "draft": false,
                ]))
            }

            throw TervezoServiceError.httpError(statusCode: 404, message: "Unexpected step \(step)")
        }

        // Check initial state
        let prOpen = try await service.getPR(id: "impl-pr-close")
        #expect(prOpen.status == "open")
        #expect(prOpen.mergeable == true)

        // Close
        let closed = try await service.closePR(id: "impl-pr-close")
        #expect(closed == true)

        // Verify closed
        let prClosed = try await service.getPR(id: "impl-pr-close")
        #expect(prClosed.status == "closed")

        // Reopen
        let reopened = try await service.reopenPR(id: "impl-pr-close")
        #expect(reopened == true)

        // Verify reopened
        let prReopened = try await service.getPR(id: "impl-pr-close")
        #expect(prReopened.status == "open")
    }

    // MARK: - Flow 11: Bearer Token in Every Request

    @Test("Auth header: every request includes correct bearer token")
    func bearerTokenConsistency() async throws {
        let service = makeService(apiKey: "tzv_consistency_check_token")
        var authHeaders: [String] = []

        MockURLProtocol.requestHandler = { request in
            if let auth = request.value(forHTTPHeaderField: "Authorization") {
                authHeaders.append(auth)
            }
            let path = request.url?.path ?? ""

            if path.contains("workspaces") {
                return (self.httpResponse(), self.makeJSON(["items": []]))
            }
            return (self.httpResponse(), self.makeJSON(self.listResponseJSON(implementations: [])))
        }

        _ = try await service.listImplementations(status: nil)
        _ = try await service.listWorkspaces()

        #expect(authHeaders.count == 2)
        for header in authHeaders {
            #expect(header == "Bearer tzv_consistency_check_token")
        }
    }

    // MARK: - Flow 12: Create Implementation with All Parameters

    @Test("Create implementation with all optional parameters populated")
    func createWithAllParams() async throws {
        let service = makeService()
        var capturedBody: [String: Any]?

        MockURLProtocol.requestHandler = { request in
            if request.httpMethod == "POST" {
                if let body = request.httpBody {
                    capturedBody = try? JSONSerialization.jsonObject(with: body) as? [String: Any]
                }
                return (self.httpResponse(statusCode: 201), self.makeJSON(self.detailJSON(
                    id: "impl-full-1",
                    title: "Full params impl",
                    status: "pending"
                )))
            }
            throw TervezoServiceError.httpError(statusCode: 404, message: "Not found")
        }

        let created = try await service.createImplementation(
            prompt: "Add comprehensive search with fuzzy matching",
            mode: "feature",
            workspaceId: "ws-production",
            repositoryName: "acme/web-app",
            baseBranch: "develop"
        )

        #expect(created.id == "impl-full-1")
        #expect(capturedBody?["prompt"] as? String == "Add comprehensive search with fuzzy matching")
        #expect(capturedBody?["mode"] as? String == "feature")
        #expect(capturedBody?["workspaceId"] as? String == "ws-production")
        #expect(capturedBody?["repositoryName"] as? String == "acme/web-app")
        #expect(capturedBody?["baseBranch"] as? String == "develop")
    }

    // MARK: - Flow 13: Timeline Pagination via Cursor

    @Test("Timeline pagination: fetch first page, then subsequent pages via cursor")
    func timelinePaginationFlow() async throws {
        let service = makeService()

        MockURLProtocol.requestHandler = { request in
            let afterParam = URLComponents(url: request.url!, resolvingAgainstBaseURL: false)?
                .queryItems?.first(where: { $0.name == "after" })?.value

            if afterParam == nil {
                // First page: oldest messages
                return (self.httpResponse(), self.makeJSON([
                    "messages": [
                        ["id": "msg-001", "type": "user_prompt", "timestamp": "2026-03-07T10:00:00Z",
                         "text": "Initial prompt"],
                        ["id": "msg-002", "type": "assistant_text", "timestamp": "2026-03-07T10:00:05Z",
                         "text": "Starting work..."],
                    ]
                ]))
            } else if afterParam == "msg-002" {
                // Second page: newer messages
                return (self.httpResponse(), self.makeJSON([
                    "messages": [
                        ["id": "msg-003", "type": "tool_call", "timestamp": "2026-03-07T10:00:10Z",
                         "toolName": "Edit"],
                        ["id": "msg-004", "type": "file_change", "timestamp": "2026-03-07T10:01:00Z",
                         "filename": "src/main.rs"],
                    ]
                ]))
            } else if afterParam == "msg-004" {
                // Third page: empty (no more messages)
                return (self.httpResponse(), self.makeJSON(["messages": []]))
            }

            throw TervezoServiceError.httpError(statusCode: 400, message: "Unexpected after param")
        }

        // Page 1
        let page1 = try await service.getTimeline(id: "impl-page-1", after: nil)
        #expect(page1.count == 2)
        #expect(page1[0].id == "msg-001")

        // Page 2 (using last ID as cursor)
        let page2 = try await service.getTimeline(id: "impl-page-1", after: page1.last?.id)
        #expect(page2.count == 2)
        #expect(page2[0].id == "msg-003")

        // Page 3 (empty — all messages fetched)
        let page3 = try await service.getTimeline(id: "impl-page-1", after: page2.last?.id)
        #expect(page3.isEmpty)

        // Combine all messages
        let allMessages = page1 + page2 + page3
        #expect(allMessages.count == 4)
    }
}
