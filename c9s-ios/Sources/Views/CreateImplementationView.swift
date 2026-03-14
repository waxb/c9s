import SwiftUI

/// Form view for creating a new Tervezo implementation.
/// Presented as a sheet from the implementation list.
struct CreateImplementationView: View {
    @State private var viewModel: CreateImplementationVM
    @Environment(\.dismiss) private var dismiss

    /// Called with the ID of the newly created implementation for navigation.
    var onCreated: (String) -> Void

    init(
        service: TervezoServiceProtocol = TervezoService(),
        onCreated: @escaping (String) -> Void
    ) {
        _viewModel = State(initialValue: CreateImplementationVM(service: service))
        self.onCreated = onCreated
    }

    var body: some View {
        NavigationStack {
            Form {
                promptSection
                workspaceSection
                modeSection
                repositorySection

                if let error = viewModel.errorMessage {
                    Section {
                        Label(error, systemImage: "exclamationmark.triangle.fill")
                            .foregroundStyle(.red)
                            .font(.callout)
                    }
                }
            }
            .navigationTitle("New Implementation")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") {
                        dismiss()
                    }
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Create") {
                        Task { await submitForm() }
                    }
                    .fontWeight(.semibold)
                    .disabled(!viewModel.isValid || viewModel.isSubmitting)
                }
            }
            .task {
                await viewModel.loadWorkspaces()
            }
            .interactiveDismissDisabled(viewModel.isSubmitting)
        }
    }

    // MARK: - Sections

    private var promptSection: some View {
        Section {
            TextEditor(text: $viewModel.prompt)
                .frame(minHeight: 120)
                .font(.body)
                .overlay(alignment: .topLeading) {
                    if viewModel.prompt.isEmpty {
                        Text("Describe what you want to implement...")
                            .foregroundStyle(.tertiary)
                            .padding(.top, 8)
                            .padding(.leading, 4)
                            .allowsHitTesting(false)
                    }
                }

            if let error = viewModel.promptValidationError {
                Text(error)
                    .font(.caption)
                    .foregroundStyle(.orange)
            }
        } header: {
            Text("Prompt")
        } footer: {
            Text("Describe the feature, bug fix, or test you want Tervezo to implement.")
        }
    }

    private var workspaceSection: some View {
        Section("Workspace") {
            if viewModel.isLoadingWorkspaces {
                HStack {
                    ProgressView()
                    Text("Loading workspaces...")
                        .foregroundStyle(.secondary)
                }
            } else if viewModel.workspaces.isEmpty {
                Label("No workspaces available", systemImage: "exclamationmark.triangle")
                    .foregroundStyle(.secondary)
            } else {
                Picker("Workspace", selection: $viewModel.selectedWorkspaceId) {
                    Text("Select a workspace")
                        .tag(nil as String?)
                    ForEach(viewModel.workspaces) { workspace in
                        Text(workspace.name)
                            .tag(workspace.id as String?)
                    }
                }
            }
        }
    }

    private var modeSection: some View {
        Section {
            Picker("Mode", selection: $viewModel.selectedMode) {
                ForEach(CreateImplementationVM.ImplementationMode.allCases) { mode in
                    Label(mode.displayName, systemImage: mode.icon)
                        .tag(mode)
                }
            }
            .pickerStyle(.menu)

            Text(viewModel.selectedMode.description)
                .font(.caption)
                .foregroundStyle(.secondary)
        } header: {
            Text("Mode")
        }
    }

    private var repositorySection: some View {
        Section {
            TextField("Repository name (optional)", text: $viewModel.repositoryName)
                .textInputAutocapitalization(.never)
                .autocorrectionDisabled()

            TextField("Base branch (optional)", text: $viewModel.baseBranch)
                .textInputAutocapitalization(.never)
                .autocorrectionDisabled()
        } header: {
            Text("Repository")
        } footer: {
            Text("Leave blank to use the workspace defaults.")
        }
    }

    // MARK: - Submit

    private func submitForm() async {
        await viewModel.submit()

        if let impl = viewModel.createdImplementation {
            dismiss()
            onCreated(impl.id)
        }
    }
}
