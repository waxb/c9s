import Foundation
@testable import C9sLib

/// Mock SSE event for testing the SSE stream service.
struct MockSSEEvent: Sendable {
    let event: String
    let data: String
    let id: String?
}

/// MockSSEStream emits predetermined events via an AsyncStream.
/// Used in ImplementationDetailVM tests.
final class MockSSEStream: Sendable {
    private let events: [MockSSEEvent]

    init(events: [MockSSEEvent]) {
        self.events = events
    }

    func makeStream() -> AsyncStream<MockSSEEvent> {
        let events = self.events
        return AsyncStream { continuation in
            for event in events {
                continuation.yield(event)
            }
            continuation.finish()
        }
    }
}
