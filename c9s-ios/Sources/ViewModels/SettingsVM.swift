import Foundation
import Observation

/// ViewModel for the Settings screen.
/// Manages API key state, base URL override, and user preferences.
@Observable
@MainActor
final class SettingsVM {
    var apiKeyMasked: String = ""
    var hasAPIKey: Bool = false
    var newAPIKey: String = ""
    var baseURLOverride: String = ""
    var pollIntervalSeconds: Int = 30
    var isUpdatingKey: Bool = false
    var errorMessage: String?
    var successMessage: String?

    private let keychain: KeychainService

    init(keychain: KeychainService = .shared) {
        self.keychain = keychain
        loadState()
    }

    func loadState() {
        if let key = keychain.loadAPIKey() {
            hasAPIKey = true
            // Mask all but first 4 and last 4 chars
            if key.count > 12 {
                let prefix = String(key.prefix(4))
                let suffix = String(key.suffix(4))
                let masked = String(repeating: "*", count: min(key.count - 8, 20))
                apiKeyMasked = "\(prefix)\(masked)\(suffix)"
            } else {
                apiKeyMasked = String(repeating: "*", count: key.count)
            }
        } else {
            hasAPIKey = false
            apiKeyMasked = ""
        }

        baseURLOverride = keychain.loadBaseURL() ?? ""
    }

    func updateAPIKey() async {
        let trimmed = newAPIKey.trimmingCharacters(in: .whitespaces)
        guard !trimmed.isEmpty else {
            errorMessage = "API key cannot be empty."
            return
        }
        guard trimmed.hasPrefix("tzv_") else {
            errorMessage = "API key should start with \"tzv_\"."
            return
        }

        isUpdatingKey = true
        errorMessage = nil
        successMessage = nil

        do {
            try keychain.saveAPIKey(trimmed)

            // Validate with API call
            let service = TervezoService(keychain: keychain)
            _ = try await service.listImplementations(status: nil)

            newAPIKey = ""
            successMessage = "API key updated successfully."
            loadState()
        } catch let error as TervezoServiceError {
            switch error {
            case .httpError(statusCode: 401, _), .httpError(statusCode: 403, _):
                try? keychain.deleteAPIKey()
                errorMessage = "Invalid API key."
                loadState()
            case .networkError:
                // Accept — might be offline
                newAPIKey = ""
                successMessage = "API key saved (network unavailable to verify)."
                loadState()
            default:
                newAPIKey = ""
                successMessage = "API key saved."
                loadState()
            }
        } catch {
            errorMessage = "Failed to save: \(error.localizedDescription)"
        }
        isUpdatingKey = false
    }

    func saveBaseURL() {
        let trimmed = baseURLOverride.trimmingCharacters(in: .whitespaces)
        do {
            if trimmed.isEmpty {
                try keychain.deleteBaseURL()
                successMessage = "Using default API URL."
            } else {
                guard URL(string: trimmed) != nil else {
                    errorMessage = "Invalid URL format."
                    return
                }
                try keychain.saveBaseURL(trimmed)
                successMessage = "Custom API URL saved."
            }
            errorMessage = nil
        } catch {
            errorMessage = "Failed to save URL: \(error.localizedDescription)"
        }
    }

    func deleteAPIKey() {
        do {
            try keychain.deleteAPIKey()
            hasAPIKey = false
            apiKeyMasked = ""
            successMessage = "API key removed."
        } catch {
            errorMessage = "Failed to remove key: \(error.localizedDescription)"
        }
    }
}
