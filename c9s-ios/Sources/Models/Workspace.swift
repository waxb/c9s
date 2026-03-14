import Foundation
import SwiftData

/// Cached workspace for offline display and implementation creation.
@Model
final class CachedWorkspace {
    @Attribute(.unique)
    var id: String

    var name: String
    var slug: String
    var logo: String?
    var lastSyncedAt: Date

    init(id: String, name: String, slug: String, logo: String? = nil, lastSyncedAt: Date = .now) {
        self.id = id
        self.name = name
        self.slug = slug
        self.logo = logo
        self.lastSyncedAt = lastSyncedAt
    }

    func updateFrom(_ workspace: TervezoWorkspace) {
        name = workspace.name
        slug = workspace.slug
        logo = workspace.logo
        lastSyncedAt = .now
    }

    static func from(_ workspace: TervezoWorkspace) -> CachedWorkspace {
        CachedWorkspace(id: workspace.id, name: workspace.name, slug: workspace.slug, logo: workspace.logo)
    }
}
