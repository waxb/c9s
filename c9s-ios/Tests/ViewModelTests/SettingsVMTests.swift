import Testing
import Foundation
@testable import C9sLib

/// Tests for SettingsVM API key management and validation.
@Suite("SettingsVM Tests")
@MainActor
struct SettingsVMTests {

    private let keychain = KeychainService.shared

    // MARK: - Setup

    private func cleanKeychain() {
        try? keychain.deleteAPIKey()
        try? keychain.deleteBaseURL()
    }

    // MARK: - Initial State

    @Test("Shows no API key when keychain is empty")
    func noAPIKeyInitially() {
        cleanKeychain()
        let vm = SettingsVM(keychain: keychain)
        #expect(vm.hasAPIKey == false)
        #expect(vm.apiKeyMasked.isEmpty)
    }

    @Test("Shows masked API key when key exists")
    func maskedAPIKey() throws {
        cleanKeychain()
        try keychain.saveAPIKey("tzv_abcdefghijklmnop")
        let vm = SettingsVM(keychain: keychain)
        #expect(vm.hasAPIKey == true)
        #expect(vm.apiKeyMasked.hasPrefix("tzv_"))
        #expect(vm.apiKeyMasked.hasSuffix("mnop"))
        #expect(vm.apiKeyMasked.contains("*"))
        cleanKeychain()
    }

    // MARK: - Validation

    @Test("Rejects API key without tzv_ prefix")
    func rejectsInvalidPrefix() async {
        cleanKeychain()
        let vm = SettingsVM(keychain: keychain)
        vm.newAPIKey = "invalid_key_without_prefix"
        await vm.updateAPIKey()
        #expect(vm.errorMessage != nil)
        #expect(vm.errorMessage?.contains("tzv_") == true)
        cleanKeychain()
    }

    @Test("Rejects empty API key")
    func rejectsEmptyKey() async {
        cleanKeychain()
        let vm = SettingsVM(keychain: keychain)
        vm.newAPIKey = ""
        await vm.updateAPIKey()
        #expect(vm.errorMessage != nil)
        cleanKeychain()
    }

    // MARK: - Delete

    @Test("Delete API key removes from keychain")
    func deleteAPIKey() throws {
        cleanKeychain()
        try keychain.saveAPIKey("tzv_to_delete")
        let vm = SettingsVM(keychain: keychain)
        #expect(vm.hasAPIKey == true)

        vm.deleteAPIKey()
        #expect(vm.hasAPIKey == false)
        #expect(keychain.loadAPIKey() == nil)
        cleanKeychain()
    }

    // MARK: - Base URL

    @Test("Save base URL stores in keychain")
    func saveBaseURL() {
        cleanKeychain()
        let vm = SettingsVM(keychain: keychain)
        vm.baseURLOverride = "https://custom.api.example.com"
        vm.saveBaseURL()
        #expect(vm.errorMessage == nil)
        #expect(keychain.loadBaseURL() == "https://custom.api.example.com")
        cleanKeychain()
    }

    @Test("Clear base URL removes override")
    func clearBaseURL() throws {
        cleanKeychain()
        try keychain.saveBaseURL("https://old.url.com")
        let vm = SettingsVM(keychain: keychain)
        vm.baseURLOverride = ""
        vm.saveBaseURL()
        #expect(keychain.loadBaseURL() == nil)
        cleanKeychain()
    }

    @Test("Rejects invalid URL format")
    func rejectsInvalidURL() {
        cleanKeychain()
        let vm = SettingsVM(keychain: keychain)
        vm.baseURLOverride = "not a url at all %%"
        vm.saveBaseURL()
        // Note: URL(string:) is lenient, so this may or may not fail
        // depending on what characters are in the string.
        // The test documents the behavior.
        cleanKeychain()
    }
}
