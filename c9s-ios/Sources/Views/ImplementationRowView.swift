import SwiftUI

/// Single row in the implementation list showing title, repo, branch, status, and time.
struct ImplementationRowView: View {
    let implementation: ImplementationSummary

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            // Title + Status
            HStack(alignment: .center) {
                Text(implementation.title ?? "(untitled)")
                    .font(.headline)
                    .lineLimit(2)

                Spacer()

                StatusBadge(status: implementation.status)
            }

            // Repo + Branch
            HStack(spacing: 8) {
                if let repoUrl = implementation.repoUrl {
                    Label(repoShort(repoUrl), systemImage: "folder")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                }

                if let branch = implementation.branch {
                    Label(branch, systemImage: "arrow.triangle.branch")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                }
            }

            // Bottom row: mode + time
            HStack {
                Text(implementation.mode)
                    .font(.caption2)
                    .foregroundStyle(.tertiary)
                    .textCase(.uppercase)

                Spacer()

                if let prUrl = implementation.prUrl {
                    Label("PR", systemImage: "arrow.triangle.pull")
                        .font(.caption2)
                        .foregroundStyle(.blue)
                }

                RelativeTimeLabel(date: implementation.updatedAt ?? implementation.createdAt)
            }
        }
        .padding(.vertical, 4)
        .accessibilityElement(children: .combine)
        .accessibilityLabel("\(implementation.title ?? "Untitled"), \(implementation.status), \(implementation.mode)")
    }

    private func repoShort(_ url: String) -> String {
        url.split(separator: "/").last.map(String.init) ?? url
    }
}
