import SwiftUI

/// First-launch screen for entering the Tervezo API key.
/// Shown when no API key is found in the Keychain.
struct OnboardingView: View {
    @State private var apiKey = ""
    @State private var isValidating = false
    @State private var errorMessage: String?
    @State private var showKey = false

    var onComplete: () -> Void

    private let keychain = KeychainService.shared

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(spacing: 32) {
                    // Header
                    VStack(spacing: 12) {
                        Image(systemName: "terminal.fill")
                            .font(.system(size: 64))
                            .foregroundStyle(.tint)

                        Text("Welcome to c9s")
                            .font(.largeTitle)
                            .fontWeight(.bold)

                        Text("Monitor and manage your Tervezo AI implementations from anywhere.")
                            .font(.body)
                            .foregroundStyle(.secondary)
                            .multilineTextAlignment(.center)
                    }
                    .padding(.top, 40)

                    // API Key Input
                    VStack(alignment: .leading, spacing: 8) {
                        Text("API Key")
                            .font(.headline)

                        HStack {
                            Group {
                                if showKey {
                                    TextField("tzv_...", text: $apiKey)
                                        .textContentType(.password)
                                } else {
                                    SecureField("tzv_...", text: $apiKey)
                                        .textContentType(.password)
                                }
                            }
                            .textInputAutocapitalization(.never)
                            .autocorrectionDisabled()
                            .font(.system(.body, design: .monospaced))

                            Button {
                                showKey.toggle()
                            } label: {
                                Image(systemName: showKey ? "eye.slash" : "eye")
                                    .foregroundStyle(.secondary)
                            }
                        }
                        .padding()
                        .background(.fill.tertiary)
                        .clipShape(RoundedRectangle(cornerRadius: 10))

                        Text("Find your API key in Tervezo account settings.")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }

                    // Error
                    if let errorMessage {
                        HStack {
                            Image(systemName: "exclamationmark.triangle.fill")
                                .foregroundStyle(.red)
                            Text(errorMessage)
                                .font(.callout)
                                .foregroundStyle(.red)
                        }
                        .padding()
                        .background(.red.opacity(0.1))
                        .clipShape(RoundedRectangle(cornerRadius: 10))
                    }

                    // Continue Button
                    Button {
                        Task {
                            await validateAndSave()
                        }
                    } label: {
                        if isValidating {
                            ProgressView()
                                .frame(maxWidth: .infinity)
                                .padding(.vertical, 4)
                        } else {
                            Text("Continue")
                                .fontWeight(.semibold)
                                .frame(maxWidth: .infinity)
                                .padding(.vertical, 4)
                        }
                    }
                    .buttonStyle(.borderedProminent)
                    .controlSize(.large)
                    .disabled(apiKey.trimmingCharacters(in: .whitespaces).isEmpty || isValidating)
                }
                .padding(.horizontal, 24)
            }
        }
    }

    private func validateAndSave() async {
        isValidating = true
        errorMessage = nil

        let trimmed = apiKey.trimmingCharacters(in: .whitespaces)

        // Basic format validation
        guard trimmed.hasPrefix("tzv_") else {
            errorMessage = "API key should start with \"tzv_\"."
            isValidating = false
            return
        }

        guard trimmed.count >= 10 else {
            errorMessage = "API key is too short."
            isValidating = false
            return
        }

        // Save the key
        do {
            try keychain.saveAPIKey(trimmed)
        } catch {
            errorMessage = "Failed to save API key: \(error.localizedDescription)"
            isValidating = false
            return
        }

        // Test the key by making a real API call
        let service = TervezoService(keychain: keychain)
        do {
            _ = try await service.listImplementations(status: nil)
        } catch let error as TervezoServiceError {
            switch error {
            case .httpError(statusCode: 401, _), .httpError(statusCode: 403, _):
                try? keychain.deleteAPIKey()
                errorMessage = "Invalid API key. Please check and try again."
                isValidating = false
                return
            case .networkError:
                // Network might be down — key could still be valid
                // Accept it and let the user proceed
                break
            default:
                // Other errors (500, etc.) — key might be valid
                break
            }
        } catch {
            // Non-service errors — proceed optimistically
        }

        isValidating = false
        onComplete()
    }
}
