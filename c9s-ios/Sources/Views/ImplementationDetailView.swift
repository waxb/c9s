import SwiftUI

/// Implementation detail view with header, step progress, tabbed content, and prompt input.
struct ImplementationDetailView: View {
    @State private var viewModel: ImplementationDetailVM
    @State private var showActionSheet = false
    @State private var actionResult: String?

    init(implementationId: String, service: TervezoServiceProtocol = TervezoService()) {
        _viewModel = State(initialValue: ImplementationDetailVM(implementationId: implementationId, service: service))
    }

    var body: some View {
        Group {
            if viewModel.isLoading && viewModel.implementation == nil {
                ProgressView("Loading...")
            } else if let impl = viewModel.implementation {
                detailContent(impl)
            } else if let error = viewModel.errorMessage {
                ContentUnavailableView {
                    Label("Error", systemImage: "exclamationmark.triangle")
                } description: {
                    Text(error)
                }
            }
        }
        .task {
            await viewModel.loadAll()
            viewModel.startPolling()
        }
        .onDisappear {
            viewModel.stopPolling()
        }
        .refreshable {
            await viewModel.refresh()
        }
    }

    @ViewBuilder
    private func detailContent(_ impl: ImplementationDetail) -> some View {
        VStack(spacing: 0) {
            // Header
            headerView(impl)

            // Step Progress
            if !impl.steps.isEmpty {
                StepProgressView(steps: impl.steps)
                    .padding(.horizontal)
                    .padding(.vertical, 8)
                Divider()
            }

            // Tab Picker
            Picker("Tab", selection: $viewModel.selectedTab) {
                ForEach(ImplementationDetailVM.DetailTab.allCases) { tab in
                    Label(tab.rawValue, systemImage: tab.icon)
                        .tag(tab)
                }
            }
            .pickerStyle(.segmented)
            .padding(.horizontal)
            .padding(.vertical, 8)

            // Tab Content
            tabContent

            // Prompt Input (shown for running implementations)
            if impl.isRunning || viewModel.isWaitingForInput {
                promptInputView
            }
        }
        .navigationTitle(impl.title ?? "Implementation")
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                actionMenu(impl)
            }
        }
        .alert("Action Result", isPresented: .init(
            get: { actionResult != nil },
            set: { if !$0 { actionResult = nil } }
        )) {
            Button("OK") { actionResult = nil }
        } message: {
            if let result = actionResult {
                Text(result)
            }
        }
    }

    // MARK: - Header

    private func headerView(_ impl: ImplementationDetail) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            // Status + Mode
            HStack {
                StatusBadge(status: impl.status)

                Text(impl.mode)
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .textCase(.uppercase)

                Spacer()

                if impl.isRunning {
                    HStack(spacing: 4) {
                        Circle()
                            .fill(.green)
                            .frame(width: 6, height: 6)
                        Text("Live")
                            .font(.caption2)
                            .foregroundStyle(.green)
                    }
                }
            }

            // Repo + Branch
            HStack(spacing: 8) {
                if let branch = impl.branch {
                    Label(branch, systemImage: "arrow.triangle.branch")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                }

                if let prUrl = impl.prUrl {
                    Link(destination: URL(string: prUrl)!) {
                        Label("PR", systemImage: "arrow.triangle.pull")
                            .font(.caption)
                    }
                }
            }

            // Iterations
            if impl.iterations > 0 {
                Text("Iteration \(impl.currentIteration)/\(impl.iterations)")
                    .font(.caption2)
                    .foregroundStyle(.tertiary)
            }

            if let error = impl.error {
                Text(error)
                    .font(.caption)
                    .foregroundStyle(.red)
                    .lineLimit(3)
            }
        }
        .padding(.horizontal)
        .padding(.vertical, 8)
        .background(.fill.quaternary)
    }

    // MARK: - Tab Content

    @ViewBuilder
    private var tabContent: some View {
        switch viewModel.selectedTab {
        case .timeline:
            TimelineView(messages: viewModel.timeline)
        case .plan:
            PlanView(plan: viewModel.plan)
        case .changes:
            ChangesView(changes: viewModel.changes)
        case .tests:
            TestOutputView(reports: viewModel.testReports)
        }
    }

    // MARK: - Prompt Input

    private var promptInputView: some View {
        VStack(spacing: 8) {
            if viewModel.isWaitingForInput {
                HStack {
                    Image(systemName: "exclamationmark.circle.fill")
                        .foregroundStyle(.orange)
                    Text("Waiting for your input")
                        .font(.caption)
                        .fontWeight(.medium)
                }
                .frame(maxWidth: .infinity)
                .padding(.vertical, 4)
                .background(.orange.opacity(0.1))
            }

            HStack(alignment: .bottom, spacing: 8) {
                TextField("Send a message...", text: $viewModel.promptText, axis: .vertical)
                    .textFieldStyle(.roundedBorder)
                    .lineLimit(1...5)

                Button {
                    Task { await viewModel.sendPrompt() }
                } label: {
                    if viewModel.isSendingPrompt {
                        ProgressView()
                            .frame(width: 28, height: 28)
                    } else {
                        Image(systemName: "arrow.up.circle.fill")
                            .font(.title2)
                    }
                }
                .disabled(viewModel.promptText.trimmingCharacters(in: .whitespaces).isEmpty || viewModel.isSendingPrompt)
            }
            .padding(.horizontal)
            .padding(.vertical, 8)

            if let error = viewModel.promptError {
                Text(error)
                    .font(.caption)
                    .foregroundStyle(.red)
                    .padding(.horizontal)
            }
        }
        .background(.fill.quaternary)
    }

    // MARK: - Action Menu

    private func actionMenu(_ impl: ImplementationDetail) -> some View {
        Menu {
            if viewModel.canCreatePR {
                Button {
                    Task {
                        do {
                            let result = try await viewModel.createPR()
                            actionResult = "PR created: \(result.prUrl)"
                        } catch {
                            actionResult = "Failed: \(error.localizedDescription)"
                        }
                    }
                } label: {
                    Label("Create PR", systemImage: "arrow.triangle.pull")
                }
            }

            if viewModel.canMergePR {
                Button {
                    Task {
                        do {
                            _ = try await viewModel.mergePR()
                            actionResult = "PR merged successfully"
                        } catch {
                            actionResult = "Failed: \(error.localizedDescription)"
                        }
                    }
                } label: {
                    Label("Merge PR", systemImage: "arrow.triangle.merge")
                }
            }

            if impl.prUrl != nil && impl.prStatus == "open" {
                Button(role: .destructive) {
                    Task {
                        do {
                            _ = try await viewModel.closePR()
                            actionResult = "PR closed"
                        } catch {
                            actionResult = "Failed: \(error.localizedDescription)"
                        }
                    }
                } label: {
                    Label("Close PR", systemImage: "xmark.circle")
                }
            }

            if viewModel.canRestart {
                Button {
                    Task {
                        do {
                            _ = try await viewModel.restart()
                            actionResult = "Implementation restarted"
                        } catch {
                            actionResult = "Failed: \(error.localizedDescription)"
                        }
                    }
                } label: {
                    Label("Restart", systemImage: "arrow.counterclockwise")
                }
            }

            if viewModel.canSSH {
                Button {
                    // SSH navigation handled by parent
                    actionResult = "SSH terminal not yet implemented"
                } label: {
                    Label("SSH to Sandbox", systemImage: "terminal")
                }
            }

            if let prUrl = impl.prUrl, let url = URL(string: prUrl) {
                Link(destination: url) {
                    Label("View PR in Safari", systemImage: "safari")
                }
            }
        } label: {
            Image(systemName: "ellipsis.circle")
        }
    }
}
