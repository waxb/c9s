import SwiftUI
import SwiftData

@main
struct C9sApp: App {
    let modelContainer: ModelContainer
    @UIApplicationDelegateAdaptor(AppDelegate.self) private var appDelegate

    init() {
        let schema = Schema([
            CachedImplementation.self,
            CachedWorkspace.self,
            CachedTimelineMessage.self,
            AppSettings.self,
        ])
        let configuration = ModelConfiguration(
            "c9s",
            schema: schema,
            isStoredInMemoryOnly: false
        )
        do {
            modelContainer = try ModelContainer(
                for: schema,
                configurations: [configuration]
            )
        } catch {
            fatalError("Failed to initialize SwiftData ModelContainer: \(error)")
        }
    }

    var body: some Scene {
        WindowGroup {
            RootView()
                .onOpenURL { url in
                    _ = DeepLinkRouter.shared.handleURL(url)
                }
        }
        .modelContainer(modelContainer)
    }
}

// MARK: - AppDelegate for Push Notifications

class AppDelegate: NSObject, UIApplicationDelegate {

    func application(
        _ application: UIApplication,
        didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]? = nil
    ) -> Bool {
        UNUserNotificationCenter.current().delegate = NotificationService.shared
        return true
    }

    func application(
        _ application: UIApplication,
        didRegisterForRemoteNotificationsWithDeviceToken deviceToken: Data
    ) {
        Task { @MainActor in
            NotificationService.shared.didRegisterForRemoteNotifications(deviceToken: deviceToken)
        }
    }

    func application(
        _ application: UIApplication,
        didFailToRegisterForRemoteNotificationsWithError error: Error
    ) {
        Task { @MainActor in
            NotificationService.shared.didFailToRegisterForRemoteNotifications(error: error)
        }
    }
}

/// Root view that decides whether to show onboarding or the main app.
/// Checks the Keychain for an API key on launch and observes changes.
struct RootView: View {
    @State private var isAuthenticated = false
    @State private var hasChecked = false

    var body: some View {
        Group {
            if !hasChecked {
                ProgressView("Loading...")
                    .task { checkAuth() }
            } else if isAuthenticated {
                MainTabView(onSignOut: {
                    isAuthenticated = false
                })
            } else {
                OnboardingView {
                    isAuthenticated = true
                }
            }
        }
        .animation(.default, value: isAuthenticated)
    }

    private func checkAuth() {
        isAuthenticated = KeychainService.shared.loadAPIKey() != nil
        hasChecked = true
    }
}

/// Main app view shown after authentication.
/// Wraps ImplementationListView with proper navigation and settings access.
struct MainTabView: View {
    var onSignOut: () -> Void

    var body: some View {
        ImplementationListView(onSignOut: onSignOut)
    }
}
