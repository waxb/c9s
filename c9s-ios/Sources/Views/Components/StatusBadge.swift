import SwiftUI

/// Colored pill showing implementation status.
/// Colors match the Rust TUI's status theming.
struct StatusBadge: View {
    let status: String

    var body: some View {
        Text(displayText)
            .font(.caption2)
            .fontWeight(.medium)
            .foregroundStyle(textColor)
            .padding(.horizontal, 8)
            .padding(.vertical, 3)
            .background(backgroundColor)
            .clipShape(Capsule())
            .accessibilityLabel("Status: \(displayText)")
    }

    private var displayText: String {
        switch status.lowercased() {
        case "running": "Running"
        case "pending": "Pending"
        case "queued": "Queued"
        case "completed": "Completed"
        case "failed": "Failed"
        case "stopped": "Stopped"
        case "merged": "Merged"
        case "cancelled": "Cancelled"
        default: status.capitalized
        }
    }

    private var textColor: Color {
        switch status.lowercased() {
        case "running": .white
        case "pending", "queued": .orange
        case "completed": .white
        case "failed": .white
        case "stopped": .white
        case "merged": .white
        case "cancelled": .secondary
        default: .primary
        }
    }

    private var backgroundColor: Color {
        switch status.lowercased() {
        case "running": .blue
        case "pending", "queued": .orange.opacity(0.2)
        case "completed": .green
        case "failed": .red
        case "stopped": .orange
        case "merged": .purple
        case "cancelled": .gray.opacity(0.2)
        default: .gray.opacity(0.2)
        }
    }
}
