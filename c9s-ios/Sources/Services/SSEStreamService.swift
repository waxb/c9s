import Foundation

/// Events emitted by the SSE stream, matching the Rust SseMessage enum.
enum SSEEvent: Sendable {
    /// New or updated timeline messages.
    case timelineMessages([TervezoTimelineMessage])
    /// Plan text updated.
    case planUpdate(String)
    /// Analysis text updated.
    case analysisUpdate(String)
    /// Implementation is waiting for user input.
    case waitingForInput(Bool)
    /// Implementation completed.
    case complete
    /// Error from the stream.
    case error(String)
    /// Stream connected.
    case connected
    /// Stream disconnected (will attempt reconnect).
    case disconnected
}

/// Service for connecting to the Tervezo SSE stream.
/// Provides an AsyncStream of typed events with auto-reconnect.
///
/// Uses URLSession's bytes(for:) for SSE parsing, which works with
/// the standard text/event-stream format.
final class SSEStreamService: Sendable {

    private let keychain: KeychainService

    init(keychain: KeychainService = .shared) {
        self.keychain = keychain
    }

    /// Connect to an implementation's SSE stream and return an AsyncStream of events.
    /// The stream auto-reconnects with exponential backoff (1s to 30s).
    /// Cancel the Task to disconnect.
    func connect(
        implementationId: String,
        lastCursor: String? = nil
    ) -> AsyncStream<SSEEvent> {
        let keychain = self.keychain
        let maxBackoff: TimeInterval = 30
        let healthyConnectionThreshold: TimeInterval = 10

        return AsyncStream { continuation in
            let task = Task {
                var cursor = lastCursor
                var backoff: TimeInterval = 1

                while !Task.isCancelled {
                    let connectedAt = Date()

                    do {
                        guard let apiKey = keychain.loadAPIKey(), !apiKey.isEmpty else {
                            continuation.yield(.error("No API key configured"))
                            break
                        }
                        guard let baseURL = URL(string: keychain.loadBaseURL() ?? TervezoService.defaultBaseURL) else {
                            continuation.yield(.error("Invalid base URL"))
                            break
                        }

                        var url = baseURL.appending(path: "implementations/\(implementationId)/stream")
                        if let cursor {
                            url.append(queryItems: [URLQueryItem(name: "after", value: cursor)])
                        }

                        var request = URLRequest(url: url)
                        request.setValue("Bearer \(apiKey)", forHTTPHeaderField: "Authorization")
                        request.setValue("text/event-stream", forHTTPHeaderField: "Accept")
                        request.setValue("c9s-ios/1.0", forHTTPHeaderField: "User-Agent")
                        request.timeoutInterval = 0 // No timeout for SSE

                        let (bytes, response) = try await URLSession.shared.bytes(for: request)

                        guard let httpResponse = response as? HTTPURLResponse,
                              httpResponse.statusCode == 200 else {
                            let code = (response as? HTTPURLResponse)?.statusCode ?? 0
                            continuation.yield(.error("SSE HTTP \(code)"))
                            break
                        }

                        continuation.yield(.connected)

                        // Parse SSE events from the byte stream
                        var dataBuf = ""
                        var eventId: String?

                        for try await line in bytes.lines {
                            if Task.isCancelled { break }

                            if line.isEmpty {
                                // Empty line = end of event
                                if !dataBuf.isEmpty {
                                    let events = parseSSEData(dataBuf, cursor: &cursor)
                                    for event in events {
                                        continuation.yield(event)
                                    }
                                    // Update cursor from event ID
                                    if let eid = eventId, cursor == nil {
                                        cursor = eid
                                    }
                                    dataBuf = ""
                                    eventId = nil
                                }
                            } else if line.hasPrefix("data: ") {
                                dataBuf += String(line.dropFirst(6))
                            } else if line.hasPrefix("id: ") {
                                eventId = String(line.dropFirst(4))
                            }
                            // Ignore other lines (event:, retry:, comments)
                        }
                    } catch is CancellationError {
                        break
                    } catch {
                        continuation.yield(.error(error.localizedDescription))
                    }

                    continuation.yield(.disconnected)

                    if Task.isCancelled { break }

                    // Reset backoff if connection was healthy
                    let alive = Date().timeIntervalSince(connectedAt)
                    if alive >= healthyConnectionThreshold {
                        backoff = 1
                    }

                    // Wait before reconnecting
                    try? await Task.sleep(for: .seconds(backoff))
                    if Task.isCancelled { break }

                    backoff = min(backoff * 2, maxBackoff)
                }

                continuation.finish()
            }

            continuation.onTermination = { _ in
                task.cancel()
            }
        }
    }

    // MARK: - SSE Data Parsing

    /// Parse a JSON envelope from SSE data.
    /// Matches the Rust SSE parser: extracts timeline messages, plan/analysis updates,
    /// waitingForInput flags, and completion signals.
    ///
    /// Internal visibility for testing.
    func parseSSEData(_ data: String, cursor: inout String?) -> [SSEEvent] {
        var events: [SSEEvent] = []

        guard let jsonData = data.data(using: .utf8),
              let envelope = try? JSONSerialization.jsonObject(with: jsonData) as? [String: Any] else {
            return [.error("Invalid SSE JSON")]
        }

        // Timeline messages: {"messages": [...]}
        if let messages = envelope["messages"] as? [Any] {
            var parsed: [TervezoTimelineMessage] = []
            for raw in messages {
                guard let msg = raw as? [String: Any],
                      let id = msg["id"] as? String,
                      let type = msg["type"] as? String,
                      let timestamp = msg["timestamp"] as? String else { continue }

                let simplified = msg.compactMapValues { value -> AnySendable? in
                    if let s = value as? String { return .string(s) }
                    if let i = value as? Int { return .int(i) }
                    if let d = value as? Double { return .double(d) }
                    if let b = value as? Bool { return .bool(b) }
                    if value is NSNull { return .null }
                    return nil
                }

                parsed.append(TervezoTimelineMessage(
                    id: id, type: type, timestamp: timestamp, rawJSON: simplified
                ))

                // Track cursor from message ID
                cursor = id
            }
            if !parsed.isEmpty {
                events.append(.timelineMessages(parsed))
            }
        }

        // Plan update: {"plan": "..."}
        if let plan = envelope["plan"] as? String {
            events.append(.planUpdate(plan))
        }

        // Analysis update: {"analysis": "..."}
        if let analysis = envelope["analysis"] as? String {
            events.append(.analysisUpdate(analysis))
        }

        // Waiting for input: {"waitingForInput": true/false}
        if let waiting = envelope["waitingForInput"] as? Bool {
            events.append(.waitingForInput(waiting))
        }

        // Status/complete events clear waiting state
        if envelope["status"] != nil || envelope["complete"] != nil {
            events.append(.waitingForInput(false))
            if envelope["complete"] != nil {
                events.append(.complete)
            }
        }

        return events
    }
}
