import SwiftUI

/// Settings screen for managing API key, base URL, and preferences.
struct SettingsView: View {
    @State private var viewModel = SettingsVM()
    @State private var showDeleteConfirmation = false

    var onSignOut: (() -> Void)?

    var body: some View {
        Form {
            // MARK: - API Key Section
            Section {
                if viewModel.hasAPIKey {
                    LabeledContent("Current Key") {
                        Text(viewModel.apiKeyMasked)
                            .font(.system(.caption, design: .monospaced))
                            .foregroundStyle(.secondary)
                    }
                } else {
                    Text("No API key configured")
                        .foregroundStyle(.red)
                }

                if viewModel.hasAPIKey {
                    DisclosureGroup("Update API Key") {
                        SecureField("New API key (tzv_...)", text: $viewModel.newAPIKey)
                            .textContentType(.password)
                            .textInputAutocapitalization(.never)
                            .autocorrectionDisabled()
                            .font(.system(.body, design: .monospaced))

                        Button("Save New Key") {
                            Task { await viewModel.updateAPIKey() }
                        }
                        .disabled(viewModel.newAPIKey.isEmpty || viewModel.isUpdatingKey)
                    }
                } else {
                    SecureField("API key (tzv_...)", text: $viewModel.newAPIKey)
                        .textContentType(.password)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                        .font(.system(.body, design: .monospaced))

                    Button("Save API Key") {
                        Task { await viewModel.updateAPIKey() }
                    }
                    .disabled(viewModel.newAPIKey.isEmpty || viewModel.isUpdatingKey)
                }
            } header: {
                Text("API Key")
            } footer: {
                Text("Your Tervezo API key. Find it in account settings.")
            }

            // MARK: - Advanced Section
            Section("Advanced") {
                VStack(alignment: .leading) {
                    TextField("Custom API URL (leave empty for default)", text: $viewModel.baseURLOverride)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                        .font(.system(.body, design: .monospaced))

                    Button("Save URL") {
                        viewModel.saveBaseURL()
                    }
                    .font(.caption)
                }

                LabeledContent("Default URL") {
                    Text(TervezoService.defaultBaseURL)
                        .font(.system(.caption2, design: .monospaced))
                        .foregroundStyle(.secondary)
                }
            }

            // MARK: - Messages
            if let error = viewModel.errorMessage {
                Section {
                    Label(error, systemImage: "exclamationmark.triangle.fill")
                        .foregroundStyle(.red)
                }
            }

            if let success = viewModel.successMessage {
                Section {
                    Label(success, systemImage: "checkmark.circle.fill")
                        .foregroundStyle(.green)
                }
            }

            // MARK: - Notifications
            Section {
                Button {
                    Task {
                        _ = await NotificationService.shared.requestAuthorization()
                    }
                } label: {
                    HStack {
                        Label("Enable Notifications", systemImage: "bell.badge")
                        Spacer()
                        if NotificationService.shared.isAuthorized {
                            Image(systemName: "checkmark.circle.fill")
                                .foregroundStyle(.green)
                        }
                    }
                }
            } header: {
                Text("Notifications")
            } footer: {
                Text("Get notified when implementations complete, fail, or need input.")
            }

            // MARK: - About Section
            Section("About") {
                LabeledContent("App") { Text("c9s Mobile") }
                LabeledContent("Version") { Text("1.0.0") }
                LabeledContent("Platform") { Text("iOS 18+") }
            }

            // MARK: - Danger Zone
            if viewModel.hasAPIKey {
                Section {
                    Button("Remove API Key", role: .destructive) {
                        showDeleteConfirmation = true
                    }
                } footer: {
                    Text("Removing the API key will sign you out.")
                }
            }
        }
        .navigationTitle("Settings")
        .confirmationDialog(
            "Remove API Key?",
            isPresented: $showDeleteConfirmation,
            titleVisibility: .visible
        ) {
            Button("Remove", role: .destructive) {
                viewModel.deleteAPIKey()
                onSignOut?()
            }
            Button("Cancel", role: .cancel) {}
        } message: {
            Text("This will remove your API key and return to the onboarding screen.")
        }
    }
}
