import SwiftUI
import SwiftTerm

/// SSH terminal screen for connecting to a Tervezo implementation sandbox.
/// Shows connection info, provides copy/open actions, and wraps SwiftTerm
/// for terminal rendering once an SSH library is integrated.
struct SandboxTerminalView: View {
    @State private var viewModel: TerminalVM
    @State private var copiedCommand = false
    @Environment(\.dismiss) private var dismiss
    @Environment(\.openURL) private var openURL

    init(implementationId: String) {
        _viewModel = State(initialValue: TerminalVM(implementationId: implementationId))
    }

    var body: some View {
        NavigationStack {
            Group {
                if viewModel.isLoadingCredentials {
                    loadingView
                } else if let credentials = viewModel.credentials {
                    connectedView(credentials)
                } else if let error = viewModel.errorMessage {
                    errorView(error)
                } else {
                    loadingView
                }
            }
            .navigationTitle("Sandbox Terminal")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Close") { dismiss() }
                }
            }
            .task {
                await viewModel.loadCredentials()
            }
        }
    }

    // MARK: - Subviews

    private var loadingView: some View {
        VStack(spacing: 16) {
            ProgressView()
            Text(viewModel.connectionState.rawValue)
                .foregroundStyle(.secondary)
        }
    }

    private func connectedView(_ credentials: TervezoSSHCredentials) -> some View {
        VStack(spacing: 0) {
            // Connection info header
            connectionInfoHeader(credentials)

            Divider()

            // Terminal placeholder / Web link
            terminalContent(credentials)
        }
    }

    private func connectionInfoHeader(_ credentials: TervezoSSHCredentials) -> some View {
        VStack(alignment: .leading, spacing: 12) {
            // SSH Command
            VStack(alignment: .leading, spacing: 4) {
                Text("SSH Command")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .textCase(.uppercase)

                HStack {
                    Text(credentials.sshCommand)
                        .font(.system(.caption, design: .monospaced))
                        .lineLimit(2)
                        .foregroundStyle(.primary)

                    Spacer()

                    Button {
                        UIPasteboard.general.string = credentials.sshCommand
                        copiedCommand = true
                        Task {
                            try? await Task.sleep(for: .seconds(2))
                            copiedCommand = false
                        }
                    } label: {
                        Image(systemName: copiedCommand ? "checkmark" : "doc.on.doc")
                            .font(.caption)
                    }
                    .buttonStyle(.bordered)
                    .controlSize(.mini)
                }
            }

            // Connection details
            HStack(spacing: 16) {
                Label(credentials.host, systemImage: "server.rack")
                    .font(.caption2)
                    .foregroundStyle(.secondary)

                Label(":\(credentials.port)", systemImage: "number")
                    .font(.caption2)
                    .foregroundStyle(.secondary)

                Label(credentials.username, systemImage: "person")
                    .font(.caption2)
                    .foregroundStyle(.secondary)
            }
        }
        .padding()
        .background(.fill.quaternary)
    }

    private func terminalContent(_ credentials: TervezoSSHCredentials) -> some View {
        VStack(spacing: 20) {
            Spacer()

            Image(systemName: "terminal.fill")
                .font(.system(size: 48))
                .foregroundStyle(.secondary)

            Text("Sandbox Terminal")
                .font(.title3)
                .fontWeight(.semibold)

            Text("Connect to the sandbox using one of the options below.")
                .font(.body)
                .foregroundStyle(.secondary)
                .multilineTextAlignment(.center)
                .padding(.horizontal)

            VStack(spacing: 12) {
                // Open in browser (sandbox web URL)
                if let webURL = viewModel.sandboxWebURL {
                    Button {
                        openURL(webURL)
                    } label: {
                        Label("Open in Browser", systemImage: "safari")
                            .frame(maxWidth: .infinity)
                    }
                    .buttonStyle(.borderedProminent)
                    .controlSize(.large)
                }

                // Copy SSH command
                Button {
                    UIPasteboard.general.string = credentials.sshCommand
                    copiedCommand = true
                    Task {
                        try? await Task.sleep(for: .seconds(2))
                        copiedCommand = false
                    }
                } label: {
                    Label(
                        copiedCommand ? "Copied!" : "Copy SSH Command",
                        systemImage: copiedCommand ? "checkmark.circle" : "doc.on.doc"
                    )
                    .frame(maxWidth: .infinity)
                }
                .buttonStyle(.bordered)
                .controlSize(.large)
            }
            .padding(.horizontal, 32)

            Spacer()
        }
    }

    private func errorView(_ error: String) -> some View {
        ContentUnavailableView {
            Label("Connection Failed", systemImage: "wifi.exclamationmark")
        } description: {
            Text(error)
        } actions: {
            Button("Retry") {
                Task { await viewModel.loadCredentials() }
            }
            .buttonStyle(.borderedProminent)
        }
    }
}

// MARK: - SwiftTerm UIViewRepresentable

/// UIViewRepresentable wrapper for SwiftTerm's TerminalView.
/// Bridges the UIKit terminal emulator into SwiftUI.
/// Ready for integration with an SSH library (e.g. swift-nio-ssh, NMSSH).
struct SwiftTerminalView: UIViewRepresentable {

    /// Called when the terminal receives data from user input.
    var onDataFromTerminal: ((Data) -> Void)?

    /// Called when the terminal view is ready.
    var onTerminalReady: ((SwiftTerm.TerminalView) -> Void)?

    func makeUIView(context: Context) -> SwiftTerm.TerminalView {
        let terminalView = SwiftTerm.TerminalView(frame: .zero)
        terminalView.terminalDelegate = context.coordinator
        terminalView.backgroundColor = .black
        context.coordinator.onDataFromTerminal = onDataFromTerminal
        onTerminalReady?(terminalView)
        return terminalView
    }

    func updateUIView(_ uiView: SwiftTerm.TerminalView, context: Context) {
        context.coordinator.onDataFromTerminal = onDataFromTerminal
    }

    func makeCoordinator() -> Coordinator {
        Coordinator()
    }

    class Coordinator: NSObject, SwiftTerm.TerminalViewDelegate {
        func hostCurrentDirectoryUpdate(source: SwiftTerm.TerminalView, directory: String?) {
            
        }
        
        func requestOpenLink(source: SwiftTerm.TerminalView, link: String, params: [String : String]) {
            if let url = URL(string: link) {
                Task { @MainActor in
                    UIApplication.shared.open(url)
                }
            }
        }
        
        var onDataFromTerminal: ((Data) -> Void)?

        func send(source: SwiftTerm.TerminalView, data: ArraySlice<UInt8>) {
            onDataFromTerminal?(Data(data))
        }

        func scrolled(source: SwiftTerm.TerminalView, position: Double) {}
        func setTerminalTitle(source: SwiftTerm.TerminalView, title: String) {}
        func sizeChanged(source: SwiftTerm.TerminalView, newCols: Int, newRows: Int) {}
        func clipboardCopy(source: SwiftTerm.TerminalView, content: Data) {
            if let text = String(data: content, encoding: .utf8) {
                UIPasteboard.general.string = text
            }
        }
        func rangeChanged(source: SwiftTerm.TerminalView, startY: Int, endY: Int) {}
    }
}
