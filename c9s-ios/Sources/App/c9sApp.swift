import SwiftUI
import SwiftData

@main
struct C9sApp: App {
    let modelContainer: ModelContainer

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
            ContentView()
        }
        .modelContainer(modelContainer)
    }
}

struct ContentView: View {
    var body: some View {
        NavigationStack {
            Text("c9s Mobile")
                .font(.largeTitle)
                .fontWeight(.bold)
                .navigationTitle("c9s")
        }
    }
}
