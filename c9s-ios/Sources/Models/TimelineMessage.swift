import Foundation
import SwiftData

/// Cached timeline message for offline display.
/// Stores the essential fields plus a JSON blob for type-specific data,
/// matching the loosely-typed schema of the Tervezo API.
@Model
final class CachedTimelineMessage {
    /// Unique message ID from the API.
    @Attribute(.unique)
    var messageId: String

    /// Implementation this message belongs to.
    var implementationId: String

    /// Message type discriminator: user_prompt, assistant_text, tool_call, etc.
    var type: String

    /// ISO8601 timestamp string from the API.
    var timestamp: String

    /// Primary text content (text, message, thinking, etc. — depends on type).
    var text: String?

    /// Tool name for tool_call and tool_result messages.
    var toolName: String?

    /// Tool call ID for correlating tool_call with tool_result.
    var toolCallId: String?

    /// File path for file_change messages.
    var filePath: String?

    /// Operation type: create, edit, delete (file_change); push, commit (git_operation).
    var operation: String?

    /// Diff content for file_change messages.
    var diff: String?

    /// Status change fields.
    var fromStatus: String?
    var toStatus: String?
    var reason: String?

    /// Error severity for error messages.
    var severity: String?

    /// PR fields for pr_created messages.
    var prUrl: String?
    var prNumber: Int?

    /// Whether this is a partial (streaming) message.
    var isPartial: Bool

    /// When this entry was cached.
    var cachedAt: Date

    init(
        messageId: String,
        implementationId: String,
        type: String,
        timestamp: String,
        text: String? = nil,
        toolName: String? = nil,
        toolCallId: String? = nil,
        filePath: String? = nil,
        operation: String? = nil,
        diff: String? = nil,
        fromStatus: String? = nil,
        toStatus: String? = nil,
        reason: String? = nil,
        severity: String? = nil,
        prUrl: String? = nil,
        prNumber: Int? = nil,
        isPartial: Bool = false,
        cachedAt: Date = .now
    ) {
        self.messageId = messageId
        self.implementationId = implementationId
        self.type = type
        self.timestamp = timestamp
        self.text = text
        self.toolName = toolName
        self.toolCallId = toolCallId
        self.filePath = filePath
        self.operation = operation
        self.diff = diff
        self.fromStatus = fromStatus
        self.toStatus = toStatus
        self.reason = reason
        self.severity = severity
        self.prUrl = prUrl
        self.prNumber = prNumber
        self.isPartial = isPartial
        self.cachedAt = cachedAt
    }

    /// Best-effort one-line display text, matching the Rust TUI's display_text() logic.
    var displayText: String {
        // Status change
        if let reason { return reason }

        // Tool call
        if let toolName {
            return text.map { "\(toolName) \($0)" } ?? toolName
        }

        // File change
        if type == "file_change" {
            let op = operation ?? "Changed"
            let opLabel = switch op {
            case "create": "Created"
            case "delete": "Deleted"
            default: "Changed"
            }
            if let filePath { return "\(opLabel) \(filePath)" }
        }

        // PR created
        if type == "pr_created" {
            if let prUrl { return "PR created: \(prUrl)" }
            if let prNumber { return "PR #\(prNumber) created" }
            return "PR created"
        }

        // Error
        if type == "error" {
            let sev = severity ?? "error"
            let msg = text ?? "Unknown error"
            return "[\(sev)] \(msg)"
        }

        // Default: use text
        return text ?? ""
    }

    /// Create from an API timeline message.
    static func from(_ msg: TervezoTimelineMessage, implementationId: String) -> CachedTimelineMessage {
        let text: String? = switch msg.rawJSON["text"] {
        case .string(let s): s
        default: switch msg.rawJSON["message"] {
            case .string(let s): s
            default: switch msg.rawJSON["thinking"] {
                case .string(let s): s
                default: switch msg.rawJSON["content"] {
                    case .string(let s): s
                    default: nil
                }
            }
        }
        }

        return CachedTimelineMessage(
            messageId: msg.id,
            implementationId: implementationId,
            type: msg.type,
            timestamp: msg.timestamp,
            text: text,
            toolName: msg.rawJSON["toolName"].flatMap { if case .string(let s) = $0 { return s } else { return nil } },
            toolCallId: msg.rawJSON["toolCallId"].flatMap { if case .string(let s) = $0 { return s } else { return nil } },
            filePath: msg.rawJSON["filePath"].flatMap { if case .string(let s) = $0 { return s } else { return nil } },
            operation: msg.rawJSON["operation"].flatMap { if case .string(let s) = $0 { return s } else { return nil } },
            diff: msg.rawJSON["diff"].flatMap { if case .string(let s) = $0 { return s } else { return nil } },
            fromStatus: msg.rawJSON["fromStatus"].flatMap { if case .string(let s) = $0 { return s } else { return nil } },
            toStatus: msg.rawJSON["toStatus"].flatMap { if case .string(let s) = $0 { return s } else { return nil } },
            reason: msg.rawJSON["reason"].flatMap { if case .string(let s) = $0 { return s } else { return nil } },
            severity: msg.rawJSON["severity"].flatMap { if case .string(let s) = $0 { return s } else { return nil } },
            prUrl: msg.rawJSON["prUrl"].flatMap { if case .string(let s) = $0 { return s } else { return nil } },
            prNumber: msg.rawJSON["prNumber"].flatMap { if case .int(let n) = $0 { return n } else { return nil } },
            isPartial: msg.rawJSON["isPartial"].flatMap { if case .bool(let b) = $0 { return b } else { return nil } } ?? false
        )
    }
}
