import SwiftUI

/// List of changed files with expandable diffs and summary stats.
struct ChangesView: View {
    let changes: [TervezoFileChange]

    var body: some View {
        if changes.isEmpty {
            ContentUnavailableView {
                Label("No Changes Yet", systemImage: "doc.badge.plus")
            } description: {
                Text("File changes will appear here as the implementation progresses.")
            }
        } else {
            List {
                // Summary
                Section("Summary") {
                    HStack {
                        Label("\(changes.count) files", systemImage: "doc")
                        Spacer()
                        Text("+\(totalAdditions)")
                            .foregroundStyle(.green)
                            .monospacedDigit()
                        Text("-\(totalDeletions)")
                            .foregroundStyle(.red)
                            .monospacedDigit()
                    }
                    .font(.callout)
                }

                // File list
                Section("Files") {
                    ForEach(changes) { change in
                        DisclosureGroup {
                            if let patch = change.patch {
                                DiffView(patch: patch)
                                    .frame(maxHeight: 400)
                            } else {
                                Text("No diff available")
                                    .foregroundStyle(.secondary)
                                    .font(.caption)
                            }
                        } label: {
                            fileRow(change)
                        }
                    }
                }
            }
            .listStyle(.insetGrouped)
        }
    }

    private func fileRow(_ change: TervezoFileChange) -> some View {
        HStack {
            Image(systemName: fileIcon(change.status))
                .foregroundStyle(fileIconColor(change.status))
                .frame(width: 16)

            Text(change.filename)
                .font(.system(.caption, design: .monospaced))
                .lineLimit(1)

            Spacer()

            if change.additions > 0 {
                Text("+\(change.additions)")
                    .font(.caption2)
                    .foregroundStyle(.green)
                    .monospacedDigit()
            }
            if change.deletions > 0 {
                Text("-\(change.deletions)")
                    .font(.caption2)
                    .foregroundStyle(.red)
                    .monospacedDigit()
            }
        }
    }

    private var totalAdditions: Int { changes.reduce(0) { $0 + $1.additions } }
    private var totalDeletions: Int { changes.reduce(0) { $0 + $1.deletions } }

    private func fileIcon(_ status: String) -> String {
        switch status {
        case "added": "plus.circle.fill"
        case "removed": "minus.circle.fill"
        case "renamed": "arrow.right.circle.fill"
        default: "pencil.circle.fill"
        }
    }

    private func fileIconColor(_ status: String) -> Color {
        switch status {
        case "added": .green
        case "removed": .red
        case "renamed": .orange
        default: .blue
        }
    }
}
