import SwiftUI

@main
struct C9sApp: App {
    var body: some Scene {
        WindowGroup {
            ContentView()
        }
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
