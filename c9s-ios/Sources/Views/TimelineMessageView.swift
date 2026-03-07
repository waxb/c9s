import SwiftUI

/// Renders a single timeline message based on its type.
struct TimelineMessageView: View {
    let message: TervezoTimelineMessage

    var body: some View {
        HStack(alignment: .top, spacing: 8) {
            messageIcon
                .frame(width: 20)

            VStack(alignment: .leading, spacing: 4) {
                messageContent
                    .font(.callout)

                Text(message.timestamp.prefix(19).replacingOccurrences(of: "T", with: " "))
                    .font(.caption2)
                    .foregroundStyle(.tertiary)
            }

            Spacer(minLength: 0)
        }
        .padding(.vertical, 4)
    }

    @ViewBuilder
    private var messageIcon: some View {
        switch message.type {
        case "user_prompt", "user_message":
            Image(systemName: "person.fill")
                .foregroundStyle(.blue)
        case "assistant_text", "assistant_thinking":
            Image(systemName: "sparkles")
                .foregroundStyle(.purple)
        case "tool_call":
            Image(systemName: "wrench.fill")
                .foregroundStyle(.orange)
        case "tool_result":
            Image(systemName: "checkmark.circle.fill")
                .foregroundStyle(.green)
        case "file_change":
            Image(systemName: "doc.fill")
                .foregroundStyle(.cyan)
        case "status_change":
            Image(systemName: "arrow.right.circle.fill")
                .foregroundStyle(.yellow)
        case "error":
            Image(systemName: "exclamationmark.triangle.fill")
                .foregroundStyle(.red)
        case "test_report":
            Image(systemName: "checkmark.circle")
                .foregroundStyle(.green)
        default:
            Image(systemName: "bubble.left.fill")
                .foregroundStyle(.gray)
        }
    }

    @ViewBuilder
    private var messageContent: some View {
        switch message.type {
        case "user_prompt", "user_message":
            userMessageView

        case "assistant_text":
            assistantTextView

        case "assistant_thinking":
            thinkingView

        case "tool_call":
            toolCallView

        case "tool_result":
            toolResultView

        case "file_change":
            fileChangeView

        case "status_change":
            statusChangeView

        case "error":
            errorView

        case "test_report":
            testReportView

        default:
            Text(extractText())
                .foregroundStyle(.secondary)
        }
    }

    // MARK: - Message Type Views

    private var userMessageView: some View {
        Text(extractText())
            .padding(8)
            .background(.blue.opacity(0.1))
            .clipShape(RoundedRectangle(cornerRadius: 8))
    }

    private var assistantTextView: some View {
        MarkdownContentView(content: extractText())
    }

    private var thinkingView: some View {
        DisclosureGroup("Thinking...") {
            Text(extractText())
                .font(.caption)
                .foregroundStyle(.secondary)
        }
        .foregroundStyle(.purple.opacity(0.7))
    }

    private var toolCallView: some View {
        HStack {
            Text(extractString("toolName") ?? "Tool")
                .font(.caption)
                .fontWeight(.semibold)
                .foregroundStyle(.orange)
                .padding(.horizontal, 6)
                .padding(.vertical, 2)
                .background(.orange.opacity(0.1))
                .clipShape(Capsule())

            if let text = extractText(), !text.isEmpty {
                Text(text)
                    .lineLimit(2)
                    .foregroundStyle(.secondary)
            }
        }
    }

    private var toolResultView: some View {
        HStack {
            Image(systemName: "checkmark")
                .foregroundStyle(.green)
                .font(.caption)
            Text(extractText().prefix(200))
                .font(.caption)
                .foregroundStyle(.secondary)
                .lineLimit(3)
        }
    }

    private var fileChangeView: some View {
        HStack {
            let op = extractString("operation") ?? "changed"
            Image(systemName: op == "create" ? "plus.circle" : op == "delete" ? "minus.circle" : "pencil.circle")
                .foregroundStyle(op == "create" ? .green : op == "delete" ? .red : .blue)

            Text(extractString("filePath") ?? "file")
                .font(.system(.caption, design: .monospaced))
                .lineLimit(1)
        }
    }

    private var statusChangeView: some View {
        HStack(spacing: 4) {
            if let from = extractString("fromStatus") {
                StatusBadge(status: from)
            }
            Image(systemName: "arrow.right")
                .font(.caption2)
            if let to = extractString("toStatus") {
                StatusBadge(status: to)
            }
            if let reason = extractString("reason") {
                Text(reason)
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
        }
    }

    private var errorView: some View {
        HStack {
            Text(extractText())
                .foregroundStyle(.red)
        }
        .padding(8)
        .background(.red.opacity(0.1))
        .clipShape(RoundedRectangle(cornerRadius: 8))
    }

    private var testReportView: some View {
        HStack {
            Image(systemName: "checkmark.circle")
            Text("Test Report")
                .fontWeight(.medium)
            if let text = extractText(), !text.isEmpty {
                Text(text)
                    .foregroundStyle(.secondary)
            }
        }
    }

    // MARK: - Helpers

    private func extractText() -> String {
        for key in ["text", "message", "thinking", "content", "reason"] {
            if case .string(let s) = message.rawJSON[key] {
                return s
            }
        }
        return ""
    }

    private func extractString(_ key: String) -> String? {
        if case .string(let s) = message.rawJSON[key] {
            return s
        }
        return nil
    }
}
