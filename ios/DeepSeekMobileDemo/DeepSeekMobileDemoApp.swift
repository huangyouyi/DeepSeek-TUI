import Foundation
import SwiftUI

@main
struct DeepSeekMobileDemoApp: App {
    @StateObject private var bridge: MockMobileCoreBridge

    init() {
        _bridge = StateObject(wrappedValue: Self.makeBridge())
    }

    var body: some Scene {
        WindowGroup {
            SessionListView()
                .environmentObject(bridge)
        }
    }

    private static func makeBridge() -> MockMobileCoreBridge {
        MockMobileCoreBridge(
            credentialStore: KeychainCredentialStore(),
            storeAdapter: makeStoreAdapter()
        )
    }

    private static func makeStoreAdapter() -> MobileStoreAdapter? {
        guard let directory = FileManager.default.urls(
            for: .applicationSupportDirectory,
            in: .userDomainMask
        ).first else {
            return nil
        }

        do {
            try FileManager.default.createDirectory(
                at: directory,
                withIntermediateDirectories: true
            )
        } catch {
            return nil
        }

        return SQLiteStoreAdapter(databaseURL: directory.appendingPathComponent("DeepSeekMobileDemo.sqlite"))
    }
}
