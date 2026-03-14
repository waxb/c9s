import Foundation
import Security

/// Provides secure storage for the Tervezo API key using the iOS Keychain.
///
/// Why Keychain over UserDefaults: API keys are sensitive credentials that
/// should survive app reinstalls and be protected by device encryption.
/// Keychain items are stored in the Secure Enclave on supported hardware.
final class KeychainService: Sendable {

    static let shared = KeychainService()

    private let service = "ai.tervezo.c9s"
    private let apiKeyAccount = "tervezo-api-key"
    private let baseURLAccount = "tervezo-base-url"

    private init() {}

    // MARK: - API Key

    /// Save the API key to the Keychain.
    /// Overwrites any existing value.
    func saveAPIKey(_ key: String) throws {
        try save(account: apiKeyAccount, value: key)
    }

    /// Load the API key from the Keychain.
    /// Returns nil if no key is stored.
    func loadAPIKey() -> String? {
        load(account: apiKeyAccount)
    }

    /// Delete the API key from the Keychain.
    func deleteAPIKey() throws {
        try delete(account: apiKeyAccount)
    }

    // MARK: - Base URL Override

    /// Save a custom base URL to the Keychain.
    func saveBaseURL(_ url: String) throws {
        try save(account: baseURLAccount, value: url)
    }

    /// Load the custom base URL from the Keychain.
    /// Returns nil if no custom URL is stored (use default).
    func loadBaseURL() -> String? {
        load(account: baseURLAccount)
    }

    /// Delete the custom base URL from the Keychain.
    func deleteBaseURL() throws {
        try delete(account: baseURLAccount)
    }

    // MARK: - Generic Keychain Operations

    private func save(account: String, value: String) throws {
        guard let data = value.data(using: .utf8) else {
            throw KeychainError.encodingFailed
        }

        // Delete existing item first (update = delete + add)
        let deleteQuery: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account,
        ]
        SecItemDelete(deleteQuery as CFDictionary)

        let addQuery: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account,
            kSecValueData as String: data,
            kSecAttrAccessible as String: kSecAttrAccessibleAfterFirstUnlock,
        ]

        let status = SecItemAdd(addQuery as CFDictionary, nil)
        guard status == errSecSuccess else {
            throw KeychainError.saveFailed(status)
        }
    }

    private func load(account: String) -> String? {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account,
            kSecReturnData as String: true,
            kSecMatchLimit as String: kSecMatchLimitOne,
        ]

        var result: AnyObject?
        let status = SecItemCopyMatching(query as CFDictionary, &result)

        guard status == errSecSuccess,
              let data = result as? Data,
              let string = String(data: data, encoding: .utf8)
        else {
            return nil
        }
        return string
    }

    private func delete(account: String) throws {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account,
        ]

        let status = SecItemDelete(query as CFDictionary)
        guard status == errSecSuccess || status == errSecItemNotFound else {
            throw KeychainError.deleteFailed(status)
        }
    }
}

enum KeychainError: Error, LocalizedError {
    case encodingFailed
    case saveFailed(OSStatus)
    case deleteFailed(OSStatus)

    var errorDescription: String? {
        switch self {
        case .encodingFailed:
            return "Failed to encode value for Keychain storage"
        case .saveFailed(let status):
            return "Keychain save failed with status \(status)"
        case .deleteFailed(let status):
            return "Keychain delete failed with status \(status)"
        }
    }
}
