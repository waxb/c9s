import Testing
import Foundation
@testable import C9sLib

/// Tests for KeychainService secure storage operations.
///
/// Note: These tests use the real Keychain via KeychainService.shared.
/// On iOS simulators and devices, this works as expected. On Linux CI,
/// these tests will need to be skipped (Keychain is Apple-platform only).
@Suite("KeychainService Tests")
struct KeychainServiceTests {

    private let keychain = KeychainService.shared

    // MARK: - API Key

    @Test("Save and load API key round-trips successfully")
    func saveAndLoadAPIKey() throws {
        let testKey = "tzv_test_key_\(UUID().uuidString)"

        try keychain.saveAPIKey(testKey)
        let loaded = keychain.loadAPIKey()

        #expect(loaded == testKey)

        // Clean up
        try keychain.deleteAPIKey()
    }

    @Test("Load API key returns nil when no key stored")
    func loadAPIKeyWhenEmpty() throws {
        // Ensure no key exists
        try keychain.deleteAPIKey()

        let loaded = keychain.loadAPIKey()
        #expect(loaded == nil)
    }

    @Test("Save API key overwrites existing key")
    func saveAPIKeyOverwrite() throws {
        let firstKey = "tzv_first_key"
        let secondKey = "tzv_second_key"

        try keychain.saveAPIKey(firstKey)
        #expect(keychain.loadAPIKey() == firstKey)

        try keychain.saveAPIKey(secondKey)
        #expect(keychain.loadAPIKey() == secondKey)

        // Clean up
        try keychain.deleteAPIKey()
    }

    @Test("Delete API key removes stored value")
    func deleteAPIKey() throws {
        try keychain.saveAPIKey("tzv_to_delete")
        #expect(keychain.loadAPIKey() != nil)

        try keychain.deleteAPIKey()
        #expect(keychain.loadAPIKey() == nil)
    }

    @Test("Delete API key succeeds when no key exists")
    func deleteAPIKeyWhenEmpty() throws {
        try keychain.deleteAPIKey() // Ensure empty
        try keychain.deleteAPIKey() // Should not throw
    }

    // MARK: - Base URL

    @Test("Save and load base URL round-trips successfully")
    func saveAndLoadBaseURL() throws {
        let testURL = "https://custom.tervezo.ai/api/v1"

        try keychain.saveBaseURL(testURL)
        let loaded = keychain.loadBaseURL()

        #expect(loaded == testURL)

        // Clean up
        try keychain.deleteBaseURL()
    }

    @Test("Load base URL returns nil when no URL stored")
    func loadBaseURLWhenEmpty() throws {
        try keychain.deleteBaseURL()

        let loaded = keychain.loadBaseURL()
        #expect(loaded == nil)
    }

    @Test("API key and base URL are independent")
    func independentStorage() throws {
        let apiKey = "tzv_independent_test"
        let baseURL = "https://custom.example.com"

        try keychain.saveAPIKey(apiKey)
        try keychain.saveBaseURL(baseURL)

        // Delete API key should not affect base URL
        try keychain.deleteAPIKey()
        #expect(keychain.loadAPIKey() == nil)
        #expect(keychain.loadBaseURL() == baseURL)

        // Clean up
        try keychain.deleteBaseURL()
    }

    // MARK: - Edge Cases

    @Test("Handles empty string API key")
    func emptyStringAPIKey() throws {
        try keychain.saveAPIKey("")
        let loaded = keychain.loadAPIKey()
        #expect(loaded == "")

        // Clean up
        try keychain.deleteAPIKey()
    }

    @Test("Handles long API key")
    func longAPIKey() throws {
        let longKey = String(repeating: "a", count: 1000)
        try keychain.saveAPIKey(longKey)
        let loaded = keychain.loadAPIKey()
        #expect(loaded == longKey)

        // Clean up
        try keychain.deleteAPIKey()
    }

    @Test("Handles special characters in API key")
    func specialCharactersAPIKey() throws {
        let specialKey = "tzv_key+/=!@#$%^&*()"
        try keychain.saveAPIKey(specialKey)
        let loaded = keychain.loadAPIKey()
        #expect(loaded == specialKey)

        // Clean up
        try keychain.deleteAPIKey()
    }

    @Test("Handles unicode in base URL")
    func unicodeBaseURL() throws {
        let unicodeURL = "https://tervezo.例え.ai/api/v1"
        try keychain.saveBaseURL(unicodeURL)
        let loaded = keychain.loadBaseURL()
        #expect(loaded == unicodeURL)

        // Clean up
        try keychain.deleteBaseURL()
    }
}
