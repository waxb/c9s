import Foundation
import SwiftData

/// User preferences stored locally via SwiftData.
/// Singleton pattern: always query for the single instance, creating if absent.
@Model
final class AppSettings {
    /// Fixed ID for singleton access.
    @Attribute(.unique)
    var id: String = "app-settings"

    /// Custom base URL override. When nil, uses the default Tervezo API URL.
    var baseURLOverride: String?

    /// Polling interval in seconds for the implementation list.
    var pollIntervalSeconds: Int

    /// Default sort field for the implementation list.
    var sortField: String

    /// Whether to sort ascending.
    var sortAscending: Bool

    /// Whether onboarding has been completed.
    var onboardingCompleted: Bool

    /// Preferred color scheme: "system", "light", or "dark".
    var colorScheme: String

    init(
        baseURLOverride: String? = nil,
        pollIntervalSeconds: Int = 30,
        sortField: String = "updatedAt",
        sortAscending: Bool = false,
        onboardingCompleted: Bool = false,
        colorScheme: String = "system"
    ) {
        self.baseURLOverride = baseURLOverride
        self.pollIntervalSeconds = pollIntervalSeconds
        self.sortField = sortField
        self.sortAscending = sortAscending
        self.onboardingCompleted = onboardingCompleted
        self.colorScheme = colorScheme
    }

    /// Creates default settings.
    static var defaults: AppSettings { AppSettings() }
}
