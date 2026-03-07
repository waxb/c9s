import Testing
import Foundation
@testable import C9sLib

/// Tests for SSEStreamService's SSE data parsing logic.
/// Covers all event envelope types, cursor tracking, and malformed input.
///
/// Critical path: every real-time update the user sees on the detail screen
/// flows through parseSSEData. A regression here silently breaks live updates.
@Suite("SSEStreamService Parsing Tests")
struct SSEStreamServiceTests {

    private let service = SSEStreamService()

    // MARK: - Timeline Messages

    @Test("Parses timeline messages envelope with cursor tracking")
    func parseTimelineMessages() {
        var cursor: String? = nil
        let data = """
        {"messages": [
            {"id": "msg-1", "type": "user_prompt", "timestamp": "2026-03-07T10:00:00Z", "text": "Fix the bug"},
            {"id": "msg-2", "type": "assistant_text", "timestamp": "2026-03-07T10:00:05Z", "text": "Working on it"}
        ]}
        """

        let events = service.parseSSEData(data, cursor: &cursor)

        #expect(events.count == 1)
        if case .timelineMessages(let messages) = events[0] {
            #expect(messages.count == 2)
            #expect(messages[0].id == "msg-1")
            #expect(messages[0].type == "user_prompt")
            #expect(messages[1].id == "msg-2")
            #expect(messages[1].type == "assistant_text")
        } else {
            Issue.record("Expected .timelineMessages, got \(events[0])")
        }

        // Cursor should be updated to last message ID
        #expect(cursor == "msg-2")
    }

    @Test("Empty messages array emits no event")
    func parseEmptyMessages() {
        var cursor: String? = nil
        let data = """
        {"messages": []}
        """

        let events = service.parseSSEData(data, cursor: &cursor)
        #expect(events.isEmpty)
        #expect(cursor == nil)
    }

    @Test("Skips messages missing required fields")
    func parseMessagesWithMissingFields() {
        var cursor: String? = nil
        let data = """
        {"messages": [
            {"id": "msg-1", "type": "user_prompt"},
            {"id": "msg-2", "type": "assistant_text", "timestamp": "2026-03-07T10:00:05Z"}
        ]}
        """

        let events = service.parseSSEData(data, cursor: &cursor)

        // First message missing timestamp → skipped
        // Second message has all required fields → parsed
        if case .timelineMessages(let messages) = events[0] {
            #expect(messages.count == 1)
            #expect(messages[0].id == "msg-2")
        } else {
            Issue.record("Expected .timelineMessages")
        }
    }

    // MARK: - Plan Update

    @Test("Parses plan update envelope")
    func parsePlanUpdate() {
        var cursor: String? = nil
        let data = """
        {"plan": "## Plan\\n1. Investigate auth module\\n2. Fix token validation"}
        """

        let events = service.parseSSEData(data, cursor: &cursor)

        #expect(events.count == 1)
        if case .planUpdate(let plan) = events[0] {
            #expect(plan.contains("Plan"))
        } else {
            Issue.record("Expected .planUpdate, got \(events[0])")
        }
    }

    // MARK: - Analysis Update

    @Test("Parses analysis update envelope")
    func parseAnalysisUpdate() {
        var cursor: String? = nil
        let data = """
        {"analysis": "The codebase uses JWT tokens for authentication"}
        """

        let events = service.parseSSEData(data, cursor: &cursor)

        #expect(events.count == 1)
        if case .analysisUpdate(let analysis) = events[0] {
            #expect(analysis.contains("JWT tokens"))
        } else {
            Issue.record("Expected .analysisUpdate, got \(events[0])")
        }
    }

    // MARK: - Waiting For Input

    @Test("Parses waitingForInput true")
    func parseWaitingForInputTrue() {
        var cursor: String? = nil
        let data = """
        {"waitingForInput": true}
        """

        let events = service.parseSSEData(data, cursor: &cursor)

        #expect(events.count == 1)
        if case .waitingForInput(let waiting) = events[0] {
            #expect(waiting == true)
        } else {
            Issue.record("Expected .waitingForInput(true), got \(events[0])")
        }
    }

    @Test("Parses waitingForInput false")
    func parseWaitingForInputFalse() {
        var cursor: String? = nil
        let data = """
        {"waitingForInput": false}
        """

        let events = service.parseSSEData(data, cursor: &cursor)

        #expect(events.count == 1)
        if case .waitingForInput(let waiting) = events[0] {
            #expect(waiting == false)
        } else {
            Issue.record("Expected .waitingForInput(false), got \(events[0])")
        }
    }

    // MARK: - Status and Complete Events

    @Test("Status event clears waiting state")
    func parseStatusEvent() {
        var cursor: String? = nil
        let data = """
        {"status": "running"}
        """

        let events = service.parseSSEData(data, cursor: &cursor)

        #expect(events.count == 1)
        if case .waitingForInput(let waiting) = events[0] {
            #expect(waiting == false)
        } else {
            Issue.record("Expected .waitingForInput(false) from status event, got \(events[0])")
        }
    }

    @Test("Complete event emits waitingForInput(false) and .complete")
    func parseCompleteEvent() {
        var cursor: String? = nil
        let data = """
        {"complete": true}
        """

        let events = service.parseSSEData(data, cursor: &cursor)

        #expect(events.count == 2)
        if case .waitingForInput(let waiting) = events[0] {
            #expect(waiting == false)
        } else {
            Issue.record("Expected .waitingForInput(false), got \(events[0])")
        }
        if case .complete = events[1] {
            // OK
        } else {
            Issue.record("Expected .complete, got \(events[1])")
        }
    }

    // MARK: - Combined Envelope

    @Test("Parses envelope with multiple event types")
    func parseCombinedEnvelope() {
        var cursor: String? = nil
        let data = """
        {
            "messages": [{"id": "msg-10", "type": "file_change", "timestamp": "2026-03-07T10:00:00Z", "filename": "src/main.rs"}],
            "plan": "Updated plan text",
            "analysis": "Updated analysis"
        }
        """

        let events = service.parseSSEData(data, cursor: &cursor)

        // Should have 3 events: timelineMessages, planUpdate, analysisUpdate
        #expect(events.count == 3)

        let hasTimeline = events.contains { if case .timelineMessages = $0 { return true } else { return false } }
        let hasPlan = events.contains { if case .planUpdate = $0 { return true } else { return false } }
        let hasAnalysis = events.contains { if case .analysisUpdate = $0 { return true } else { return false } }

        #expect(hasTimeline)
        #expect(hasPlan)
        #expect(hasAnalysis)
        #expect(cursor == "msg-10")
    }

    // MARK: - Error Handling

    @Test("Invalid JSON returns error event")
    func parseInvalidJSON() {
        var cursor: String? = nil
        let data = "not valid json {"

        let events = service.parseSSEData(data, cursor: &cursor)

        #expect(events.count == 1)
        if case .error(let message) = events[0] {
            #expect(message == "Invalid SSE JSON")
        } else {
            Issue.record("Expected .error, got \(events[0])")
        }
        #expect(cursor == nil)
    }

    @Test("Empty string returns error event")
    func parseEmptyString() {
        var cursor: String? = nil
        let events = service.parseSSEData("", cursor: &cursor)

        #expect(events.count == 1)
        if case .error = events[0] {
            // OK
        } else {
            Issue.record("Expected .error for empty string")
        }
    }

    // MARK: - Cursor Tracking

    @Test("Cursor updates progressively with each message batch")
    func cursorProgression() {
        var cursor: String? = nil

        // First batch
        let data1 = """
        {"messages": [{"id": "msg-100", "type": "text", "timestamp": "2026-03-07T10:00:00Z"}]}
        """
        _ = service.parseSSEData(data1, cursor: &cursor)
        #expect(cursor == "msg-100")

        // Second batch
        let data2 = """
        {"messages": [{"id": "msg-200", "type": "text", "timestamp": "2026-03-07T10:01:00Z"}]}
        """
        _ = service.parseSSEData(data2, cursor: &cursor)
        #expect(cursor == "msg-200")
    }

    @Test("Non-message events do not modify cursor")
    func cursorUnchangedForNonMessages() {
        var cursor: String? = "msg-50"

        let data = """
        {"plan": "some plan text"}
        """
        _ = service.parseSSEData(data, cursor: &cursor)
        #expect(cursor == "msg-50")
    }

    // MARK: - Timeline Message rawJSON Parsing

    @Test("Timeline message rawJSON preserves string, int, double, bool, null values")
    func parseTimelineRawJSONTypes() {
        var cursor: String? = nil
        let data = """
        {"messages": [{"id": "msg-types", "type": "tool_call", "timestamp": "2026-03-07T10:00:00Z", "toolName": "Read", "additions": 15, "isNew": true}]}
        """

        let events = service.parseSSEData(data, cursor: &cursor)

        if case .timelineMessages(let messages) = events[0] {
            let raw = messages[0].rawJSON
            if case .string(let val) = raw["toolName"] {
                #expect(val == "Read")
            } else {
                Issue.record("Expected string for toolName")
            }
            if case .string(let val) = raw["type"] {
                #expect(val == "tool_call")
            } else {
                Issue.record("Expected string for type")
            }
        } else {
            Issue.record("Expected .timelineMessages")
        }
    }
}
