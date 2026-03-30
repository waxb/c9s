import SwiftUI

/// Test report display with summary stats and individual test details.
struct TestOutputView: View {
    let reports: [TervezoTestReport]

    var body: some View {
        if reports.isEmpty {
            ContentUnavailableView {
                Label("No Test Results", systemImage: "checkmark.circle")
            } description: {
                Text("Test results will appear here when tests are run.")
            }
        } else {
            List {
                ForEach(reports) { report in
                    Section {
                        // Status
                        if let status = report.summaryStatus {
                            HStack {
                                Image(systemName: status == "pass" ? "checkmark.circle.fill" : "xmark.circle.fill")
                                    .foregroundStyle(status == "pass" ? .green : .red)
                                Text(status.capitalized)
                                    .fontWeight(.medium)
                            }
                        }

                        // Message
                        if let message = report.summaryMessage {
                            Text(message)
                                .font(.callout)
                                .foregroundStyle(.secondary)
                        }

                        // Stats
                        HStack(spacing: 16) {
                            if let total = report.totalAfter {
                                Label("\(total) total", systemImage: "number")
                                    .font(.caption)
                            }
                            if let newTests = report.newTests {
                                Label("+\(newTests) new", systemImage: "plus.circle")
                                    .font(.caption)
                                    .foregroundStyle(.green)
                            }
                        }

                        // Approach
                        if let approach = report.approach {
                            Text(approach)
                                .font(.caption)
                                .foregroundStyle(.secondary)
                        }
                    } header: {
                        Text("Report \(report.timestamp.prefix(10))")
                    }
                }
            }
            .listStyle(.insetGrouped)
        }
    }
}
