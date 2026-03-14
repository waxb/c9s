import Foundation
import OpenAPIRuntime
import OpenAPIURLSession

// MARK: - Service Protocol

/// Protocol for the Tervezo API service, enabling mock implementations for testing.
protocol TervezoServiceProtocol: Sendable {
    func listImplementations(status: String?) async throws -> ImplementationList
    func getImplementation(id: String) async throws -> ImplementationDetail
    func getTimeline(id: String, after: String?) async throws -> [TervezoTimelineMessage]
    func getPlan(id: String) async throws -> String
    func getAnalysis(id: String) async throws -> String
    func getChanges(id: String) async throws -> [TervezoFileChange]
    func getTestOutput(id: String) async throws -> [TervezoTestReport]
    func getSSH(id: String) async throws -> TervezoSSHCredentials
    func getStatus(id: String) async throws -> TervezoStatusResponse
    func sendPrompt(id: String, message: String) async throws -> TervezoPromptResponse
    func createPR(id: String) async throws -> TervezoPRCreateResponse
    func getPR(id: String) async throws -> TervezoPRDetails
    func mergePR(id: String) async throws -> Bool
    func closePR(id: String) async throws -> Bool
    func reopenPR(id: String) async throws -> Bool
    func restart(id: String) async throws -> TervezoRestartResponse
    func listWorkspaces() async throws -> [TervezoWorkspace]
    func createImplementation(
        prompt: String,
        mode: String,
        workspaceId: String,
        repositoryName: String?,
        baseBranch: String?
    ) async throws -> ImplementationDetail
}

// MARK: - App-Level Domain Models

/// Lightweight model for the implementation list (subset of fields from the list endpoint).
struct ImplementationList: Sendable {
    let items: [ImplementationSummary]
    let total: Int?
}

struct ImplementationSummary: Sendable, Identifiable {
    let id: String
    let title: String?
    let status: String
    let mode: String
    let repoUrl: String?
    let branch: String?
    let prUrl: String?
    let prStatus: String?
    let createdAt: Date
    let updatedAt: Date?
}

/// Full implementation detail (from the getById endpoint).
struct ImplementationDetail: Sendable, Identifiable {
    let id: String
    let title: String?
    let status: String
    let mode: String
    let prompt: String?
    let plan: String?
    let analysis: String?
    let error: String?
    let isRunning: Bool
    let repoUrl: String?
    let branch: String?
    let baseBranch: String?
    let branchPushed: Bool?
    let prUrl: String?
    let prStatus: String?
    let sandboxId: String?
    let iterations: Int
    let currentIteration: Int
    let createdAt: Date
    let updatedAt: Date?
    let steps: [TervezoStep]
    let timelineMessageCount: Int
}

struct TervezoStep: Sendable, Identifiable {
    let id: String
    let name: String
    let order: Int
    let status: String
    let startedAt: Date?
    let completedAt: Date?
    let error: String?
}

struct TervezoTimelineMessage: Sendable, Identifiable {
    let id: String
    let type: String
    let timestamp: String
    let rawJSON: [String: AnySendable]
}

struct TervezoFileChange: Sendable, Identifiable {
    var id: String { filename }
    let filename: String
    let status: String
    let additions: Int
    let deletions: Int
    let changes: Int
    let patch: String?
}

struct TervezoTestReport: Sendable, Identifiable {
    var id: String { reportId }
    let reportId: String
    let timestamp: String
    let summaryStatus: String?
    let summaryMessage: String?
    let newTests: Int?
    let totalBefore: Int?
    let totalAfter: Int?
    let approach: String?
}

struct TervezoSSHCredentials: Sendable {
    let host: String
    let port: Int
    let username: String
    let sshCommand: String
    let sandboxId: String
    let sandboxUrl: String
}

struct TervezoStatusResponse: Sendable {
    let status: String
    let waitingForInput: Bool
    let currentStepName: String?
    let startedAt: Date?
    let completedAt: Date?
    let duration: Double?
    let steps: [TervezoStatusStep]
}

struct TervezoStatusStep: Sendable, Identifiable {
    var id: String { name }
    let name: String
    let order: Int
    let status: String
    let duration: Double?
    let error: String?
}

struct TervezoPromptResponse: Sendable {
    let sent: Bool
    let followUpId: String?
}

struct TervezoPRCreateResponse: Sendable {
    let prUrl: String
    let prNumber: Int
}

struct TervezoPRDetails: Sendable {
    let url: String
    let number: Int
    let status: String
    let title: String
    let mergeable: Bool?
    let merged: Bool
    let draft: Bool
}

struct TervezoRestartResponse: Sendable {
    let implementationId: String
    let isNewImplementation: Bool
}

struct TervezoWorkspace: Sendable, Identifiable {
    let id: String
    let name: String
    let slug: String
    let logo: String?
}

/// Type-erased Sendable wrapper for JSON values in timeline messages.
enum AnySendable: Sendable {
    case string(String)
    case int(Int)
    case double(Double)
    case bool(Bool)
    case null
}

// MARK: - Service Errors

enum TervezoServiceError: Error, LocalizedError {
    case noAPIKey
    case invalidBaseURL(String)
    case httpError(statusCode: Int, message: String)
    case decodingError(String)
    case networkError(String)
    case conflict(String)

    var errorDescription: String? {
        switch self {
        case .noAPIKey:
            return "No API key configured. Add your Tervezo API key in Settings."
        case .invalidBaseURL(let url):
            return "Invalid base URL: \(url)"
        case .httpError(let code, let message):
            return "API error (\(code)): \(message)"
        case .decodingError(let detail):
            return "Failed to parse API response: \(detail)"
        case .networkError(let detail):
            return "Network error: \(detail)"
        case .conflict(let detail):
            return "Conflict: \(detail)"
        }
    }
}

// MARK: - Service Implementation

/// Wraps the Tervezo REST API with bearer auth from Keychain,
/// configurable base URL, and user-friendly error mapping.
final class TervezoService: TervezoServiceProtocol, @unchecked Sendable {

    static let defaultBaseURL = "https://app.tervezo.ai/api/v1"

    private let keychain: KeychainService
    private let session: URLSession

    init(keychain: KeychainService = .shared, session: URLSession = .shared) {
        self.keychain = keychain
        self.session = session
    }

    // MARK: - Implementations

    func listImplementations(status: String?) async throws -> ImplementationList {
        var url = try baseURL().appending(path: "implementations")
        if let status {
            url.append(queryItems: [URLQueryItem(name: "status", value: status)])
        }
        let data = try await performGET(url: url)
        return try decodeListResponse(data)
    }

    func getImplementation(id: String) async throws -> ImplementationDetail {
        let url = try baseURL().appending(path: "implementations/\(id)")
        let data = try await performGET(url: url)
        return try decodeImplementationDetail(data)
    }

    func getTimeline(id: String, after: String?) async throws -> [TervezoTimelineMessage] {
        var url = try baseURL().appending(path: "implementations/\(id)/timeline")
        if let after {
            url.append(queryItems: [URLQueryItem(name: "after", value: after)])
        }
        let data = try await performGET(url: url, timeout: 60)
        return try decodeTimeline(data)
    }

    func getPlan(id: String) async throws -> String {
        let url = try baseURL().appending(path: "implementations/\(id)/plan")
        let data = try await performGET(url: url)
        let json = try decodeJSON(data)
        guard let plan = json["plan"] as? String else {
            throw TervezoServiceError.decodingError("Missing 'plan' field")
        }
        return plan
    }

    func getAnalysis(id: String) async throws -> String {
        let url = try baseURL().appending(path: "implementations/\(id)/analysis")
        let data = try await performGET(url: url)
        let json = try decodeJSON(data)
        guard let analysis = json["analysis"] as? String else {
            throw TervezoServiceError.decodingError("Missing 'analysis' field")
        }
        return analysis
    }

    func getChanges(id: String) async throws -> [TervezoFileChange] {
        let url = try baseURL().appending(path: "implementations/\(id)/changes")
        let data = try await performGET(url: url)
        return try decodeChanges(data)
    }

    func getTestOutput(id: String) async throws -> [TervezoTestReport] {
        let url = try baseURL().appending(path: "implementations/\(id)/test-output")
        let data = try await performGET(url: url)
        return try decodeTestOutput(data)
    }

    func getSSH(id: String) async throws -> TervezoSSHCredentials {
        let url = try baseURL().appending(path: "implementations/\(id)/ssh")
        let data = try await performGET(url: url)
        return try decodeSSHCredentials(data)
    }

    func getStatus(id: String) async throws -> TervezoStatusResponse {
        let url = try baseURL().appending(path: "implementations/\(id)/status")
        let data = try await performGET(url: url)
        return try decodeStatusResponse(data)
    }

    // MARK: - Actions

    func sendPrompt(id: String, message: String) async throws -> TervezoPromptResponse {
        let url = try baseURL().appending(path: "implementations/\(id)/prompt")
        let body = try JSONSerialization.data(withJSONObject: ["message": message])
        let data = try await performPOST(url: url, body: body)
        let json = try decodeJSON(data)
        return TervezoPromptResponse(
            sent: json["sent"] as? Bool ?? false,
            followUpId: json["followUpId"] as? String
        )
    }

    func createPR(id: String) async throws -> TervezoPRCreateResponse {
        let url = try baseURL().appending(path: "implementations/\(id)/pr")
        let data = try await performPOST(url: url, body: Data("{}".utf8))
        let json = try decodeJSON(data)
        return TervezoPRCreateResponse(
            prUrl: json["prUrl"] as? String ?? "",
            prNumber: json["prNumber"] as? Int ?? 0
        )
    }

    func getPR(id: String) async throws -> TervezoPRDetails {
        let url = try baseURL().appending(path: "implementations/\(id)/pr")
        let data = try await performGET(url: url)
        return try decodePRDetails(data)
    }

    func mergePR(id: String) async throws -> Bool {
        let url = try baseURL().appending(path: "implementations/\(id)/pr/merge")
        let data = try await performPOST(url: url, body: Data("{}".utf8))
        let json = try decodeJSON(data)
        return json["success"] as? Bool ?? false
    }

    func closePR(id: String) async throws -> Bool {
        let url = try baseURL().appending(path: "implementations/\(id)/pr/close")
        let data = try await performPOST(url: url, body: Data("{}".utf8))
        let json = try decodeJSON(data)
        return json["success"] as? Bool ?? false
    }

    func reopenPR(id: String) async throws -> Bool {
        let url = try baseURL().appending(path: "implementations/\(id)/pr/reopen")
        let data = try await performPOST(url: url, body: Data("{}".utf8))
        let json = try decodeJSON(data)
        return json["success"] as? Bool ?? false
    }

    func restart(id: String) async throws -> TervezoRestartResponse {
        let url = try baseURL().appending(path: "implementations/\(id)/restart")
        let data = try await performPOST(url: url, body: Data("{}".utf8))
        let json = try decodeJSON(data)
        return TervezoRestartResponse(
            implementationId: json["implementationId"] as? String ?? "",
            isNewImplementation: json["isNewImplementation"] as? Bool ?? false
        )
    }

    // MARK: - Workspaces

    func listWorkspaces() async throws -> [TervezoWorkspace] {
        let url = try baseURL().appending(path: "workspaces")
        let data = try await performGET(url: url)
        let json = try decodeJSON(data)
        guard let items = json["items"] as? [[String: Any]] else {
            throw TervezoServiceError.decodingError("Missing 'items' in workspaces response")
        }
        return items.compactMap { item in
            guard let id = item["id"] as? String,
                  let name = item["name"] as? String,
                  let slug = item["slug"] as? String else { return nil }
            return TervezoWorkspace(id: id, name: name, slug: slug, logo: item["logo"] as? String)
        }
    }

    // MARK: - Create Implementation

    func createImplementation(
        prompt: String,
        mode: String,
        workspaceId: String,
        repositoryName: String?,
        baseBranch: String?
    ) async throws -> ImplementationDetail {
        let url = try baseURL().appending(path: "implementations")
        var body: [String: Any] = [
            "prompt": prompt,
            "mode": mode,
            "workspaceId": workspaceId,
        ]
        if let repositoryName { body["repositoryName"] = repositoryName }
        if let baseBranch { body["baseBranch"] = baseBranch }
        let bodyData = try JSONSerialization.data(withJSONObject: body)
        let data = try await performPOST(url: url, body: bodyData)
        return try decodeImplementationDetail(data)
    }

    // MARK: - HTTP Helpers

    private func baseURL() throws -> URL {
        let urlString = keychain.loadBaseURL() ?? Self.defaultBaseURL
        guard let url = URL(string: urlString) else {
            throw TervezoServiceError.invalidBaseURL(urlString)
        }
        return url
    }

    private func apiKey() throws -> String {
        guard let key = keychain.loadAPIKey(), !key.isEmpty else {
            throw TervezoServiceError.noAPIKey
        }
        return key
    }

    private func performGET(url: URL, timeout: TimeInterval = 10) async throws -> Data {
        var request = URLRequest(url: url, timeoutInterval: timeout)
        request.httpMethod = "GET"
        request.setValue("Bearer \(try apiKey())", forHTTPHeaderField: "Authorization")
        request.setValue("application/json", forHTTPHeaderField: "Accept")
        request.setValue("c9s-ios/1.0", forHTTPHeaderField: "User-Agent")
        return try await execute(request)
    }

    private func performPOST(url: URL, body: Data, timeout: TimeInterval = 10) async throws -> Data {
        var request = URLRequest(url: url, timeoutInterval: timeout)
        request.httpMethod = "POST"
        request.httpBody = body
        request.setValue("Bearer \(try apiKey())", forHTTPHeaderField: "Authorization")
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.setValue("application/json", forHTTPHeaderField: "Accept")
        request.setValue("c9s-ios/1.0", forHTTPHeaderField: "User-Agent")
        return try await execute(request)
    }

    private func execute(_ request: URLRequest) async throws -> Data {
        let (data, response): (Data, URLResponse)
        do {
            (data, response) = try await session.data(for: request)
        } catch {
            throw TervezoServiceError.networkError(error.localizedDescription)
        }

        guard let httpResponse = response as? HTTPURLResponse else {
            throw TervezoServiceError.networkError("Invalid response type")
        }

        let statusCode = httpResponse.statusCode
        guard (200...299).contains(statusCode) else {
            let body = String(data: data, encoding: .utf8) ?? "(unreadable)"
            if statusCode == 409 {
                throw TervezoServiceError.conflict(body)
            }
            throw TervezoServiceError.httpError(statusCode: statusCode, message: body)
        }

        return data
    }

    // MARK: - Decoding Helpers

    private func decodeJSON(_ data: Data) throws -> [String: Any] {
        guard let json = try JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            throw TervezoServiceError.decodingError("Expected JSON object")
        }
        return json
    }

    private let iso8601Formatter: ISO8601DateFormatter = {
        let formatter = ISO8601DateFormatter()
        formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        return formatter
    }()

    private func parseDate(_ string: String?) -> Date? {
        guard let string else { return nil }
        return iso8601Formatter.date(from: string)
            ?? ISO8601DateFormatter().date(from: string)
    }

    private func decodeListResponse(_ data: Data) throws -> ImplementationList {
        let json = try decodeJSON(data)
        guard let items = json["items"] as? [[String: Any]] else {
            throw TervezoServiceError.decodingError("Missing 'items' in list response")
        }
        let summaries = items.compactMap { item -> ImplementationSummary? in
            guard let id = item["id"] as? String,
                  let status = item["status"] as? String,
                  let mode = item["mode"] as? String,
                  let createdAtStr = item["createdAt"] as? String,
                  let createdAt = parseDate(createdAtStr) else { return nil }
            return ImplementationSummary(
                id: id,
                title: item["title"] as? String,
                status: status,
                mode: mode,
                repoUrl: item["repoUrl"] as? String,
                branch: item["branch"] as? String,
                prUrl: item["prUrl"] as? String,
                prStatus: item["prStatus"] as? String,
                createdAt: createdAt,
                updatedAt: parseDate(item["updatedAt"] as? String)
            )
        }
        return ImplementationList(items: summaries, total: json["total"] as? Int)
    }

    private func decodeImplementationDetail(_ data: Data) throws -> ImplementationDetail {
        let json = try decodeJSON(data)
        guard let id = json["id"] as? String,
              let status = json["status"] as? String,
              let mode = json["mode"] as? String,
              let createdAtStr = json["createdAt"] as? String,
              let createdAt = parseDate(createdAtStr) else {
            throw TervezoServiceError.decodingError("Missing required fields in implementation detail")
        }

        let stepsRaw = json["steps"] as? [[String: Any]] ?? []
        let steps = stepsRaw.compactMap { step -> TervezoStep? in
            guard let stepId = step["id"] as? String,
                  let name = step["name"] as? String else { return nil }
            return TervezoStep(
                id: stepId,
                name: name,
                order: step["order"] as? Int ?? 0,
                status: step["status"] as? String ?? "pending",
                startedAt: parseDate(step["startedAt"] as? String),
                completedAt: parseDate(step["completedAt"] as? String),
                error: step["error"] as? String
            )
        }

        return ImplementationDetail(
            id: id,
            title: json["title"] as? String,
            status: status,
            mode: mode,
            prompt: json["prompt"] as? String,
            plan: json["plan"] as? String,
            analysis: json["analysis"] as? String,
            error: json["error"] as? String,
            isRunning: json["isRunning"] as? Bool ?? false,
            repoUrl: json["repoUrl"] as? String,
            branch: json["branch"] as? String,
            baseBranch: json["baseBranch"] as? String,
            branchPushed: json["branchPushed"] as? Bool,
            prUrl: json["prUrl"] as? String,
            prStatus: json["prStatus"] as? String,
            sandboxId: json["sandboxId"] as? String,
            iterations: json["iterations"] as? Int ?? 0,
            currentIteration: json["currentIteration"] as? Int ?? 0,
            createdAt: createdAt,
            updatedAt: parseDate(json["updatedAt"] as? String),
            steps: steps,
            timelineMessageCount: json["timelineMessageCount"] as? Int ?? 0
        )
    }

    private func decodeTimeline(_ data: Data) throws -> [TervezoTimelineMessage] {
        let json = try decodeJSON(data)
        guard let messages = json["messages"] as? [Any] else {
            throw TervezoServiceError.decodingError("Missing 'messages' in timeline response")
        }
        return messages.compactMap { raw -> TervezoTimelineMessage? in
            guard let msg = raw as? [String: Any],
                  let id = msg["id"] as? String,
                  let type = msg["type"] as? String,
                  let timestamp = msg["timestamp"] as? String else { return nil }
            let simplified = msg.compactMapValues { value -> AnySendable? in
                if let s = value as? String { return .string(s) }
                if let i = value as? Int { return .int(i) }
                if let d = value as? Double { return .double(d) }
                if let b = value as? Bool { return .bool(b) }
                if value is NSNull { return .null }
                return nil
            }
            return TervezoTimelineMessage(id: id, type: type, timestamp: timestamp, rawJSON: simplified)
        }
    }

    private func decodeChanges(_ data: Data) throws -> [TervezoFileChange] {
        let json = try decodeJSON(data)
        guard let files = json["files"] as? [[String: Any]] else {
            throw TervezoServiceError.decodingError("Missing 'files' in changes response")
        }
        return files.compactMap { file -> TervezoFileChange? in
            guard let filename = file["filename"] as? String,
                  let status = file["status"] as? String else { return nil }
            return TervezoFileChange(
                filename: filename,
                status: status,
                additions: file["additions"] as? Int ?? 0,
                deletions: file["deletions"] as? Int ?? 0,
                changes: file["changes"] as? Int ?? 0,
                patch: file["patch"] as? String
            )
        }
    }

    private func decodeTestOutput(_ data: Data) throws -> [TervezoTestReport] {
        let json = try decodeJSON(data)
        guard let reports = json["testReports"] as? [[String: Any]] else {
            throw TervezoServiceError.decodingError("Missing 'testReports' in test output response")
        }
        return reports.compactMap { report -> TervezoTestReport? in
            guard let id = report["id"] as? String,
                  let timestamp = report["timestamp"] as? String else { return nil }
            let summary = report["summary"] as? [String: Any]
            let stats = summary?["stats"] as? [String: Any]
            return TervezoTestReport(
                reportId: id,
                timestamp: timestamp,
                summaryStatus: summary?["status"] as? String,
                summaryMessage: summary?["message"] as? String,
                newTests: stats?["newTests"] as? Int,
                totalBefore: stats?["totalBefore"] as? Int,
                totalAfter: stats?["totalAfter"] as? Int,
                approach: report["approach"] as? String
            )
        }
    }

    private func decodeSSHCredentials(_ data: Data) throws -> TervezoSSHCredentials {
        let json = try decodeJSON(data)
        guard let host = json["host"] as? String,
              let port = json["port"] as? Int,
              let username = json["username"] as? String,
              let sshCommand = json["sshCommand"] as? String,
              let sandboxId = json["sandboxId"] as? String,
              let sandboxUrl = json["sandboxUrl"] as? String else {
            throw TervezoServiceError.decodingError("Missing required SSH credential fields")
        }
        return TervezoSSHCredentials(
            host: host, port: port, username: username,
            sshCommand: sshCommand, sandboxId: sandboxId, sandboxUrl: sandboxUrl
        )
    }

    private func decodeStatusResponse(_ data: Data) throws -> TervezoStatusResponse {
        let json = try decodeJSON(data)
        guard let status = json["status"] as? String else {
            throw TervezoServiceError.decodingError("Missing 'status' in status response")
        }
        let stepsRaw = json["steps"] as? [[String: Any]] ?? []
        let steps = stepsRaw.compactMap { step -> TervezoStatusStep? in
            guard let name = step["name"] as? String else { return nil }
            return TervezoStatusStep(
                name: name,
                order: step["order"] as? Int ?? 0,
                status: step["status"] as? String ?? "pending",
                duration: step["duration"] as? Double,
                error: step["error"] as? String
            )
        }
        return TervezoStatusResponse(
            status: status,
            waitingForInput: json["waitingForInput"] as? Bool ?? false,
            currentStepName: json["currentStepName"] as? String,
            startedAt: parseDate(json["startedAt"] as? String),
            completedAt: parseDate(json["completedAt"] as? String),
            duration: json["duration"] as? Double,
            steps: steps
        )
    }

    private func decodePRDetails(_ data: Data) throws -> TervezoPRDetails {
        let json = try decodeJSON(data)
        guard let url = json["url"] as? String,
              let number = json["number"] as? Int,
              let status = json["status"] as? String,
              let title = json["title"] as? String else {
            throw TervezoServiceError.decodingError("Missing required PR detail fields")
        }
        return TervezoPRDetails(
            url: url, number: number, status: status, title: title,
            mergeable: json["mergeable"] as? Bool,
            merged: json["merged"] as? Bool ?? false,
            draft: json["draft"] as? Bool ?? false
        )
    }
}
