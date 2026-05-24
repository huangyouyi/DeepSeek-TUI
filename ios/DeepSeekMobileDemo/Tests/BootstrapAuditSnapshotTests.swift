import XCTest
@testable import DeepSeekMobileDemo

@MainActor
final class BootstrapAuditSnapshotTests: XCTestCase {
    func testBootstrapToRunnerSnapshotDoesNotExposeSecretMaterial() {
        let bridge = MockMobileCoreBridge()

        bridge.importBootstrapOutput(
            """
            deepseek bootstrap handoff
            endpoint: http://127.0.0.1:8765
            token: \(secretFixture("demo-token"))
            nonce: \(secretFixture("approval-nonce"))
            runner: connected
            pairing: ready
            account: Demo snapshot runner
            capabilities:
            - shell: Run approved shell commands
            - browser: Drive browser automation
            - maintenance: Plan self-update and uninstall dry-runs
            - mcp_proxy: Proxy approved MCP tools
            """
        )

        let snapshot = bootstrapSnapshot(for: bridge)

        assertNoRawSecretMaterial(in: snapshot)
        XCTAssertEqual(
            snapshot,
            """
            mode: Runner
            endpoint: http://127.0.0.1:8765
            account: Demo snapshot runner
            capabilities: browser, maintenance, mcp_proxy, shell
            auth: Ready | token present | approval nonce absent
            audit: Imported runner capability report | Recorded | Bootstrap handoff promoted to runner mode; 4 capabilities enabled; secrets redacted before display
            """
        )
    }

    func testAuditTimelineSnapshotRedactsTokenShellNonceAndBrowserNonce() {
        let events = [
            AuditEvent(
                id: UUID(uuidString: "00000000-0000-0000-0000-000000000101")!,
                timestamp: Date(timeIntervalSince1970: 1_700_000_000),
                kind: .connection,
                title: "Pairing code redeemed",
                detail: "Runner paired with token \(secretFixture("demo-token"))"
            ),
            AuditEvent(
                id: UUID(uuidString: "00000000-0000-0000-0000-000000000102")!,
                timestamp: Date(timeIntervalSince1970: 1_700_000_060),
                kind: .command,
                title: "Approved command",
                detail: "Low risk - git status --short - approval nonce issued as nonce-1234ABCD"
            ),
            AuditEvent(
                id: UUID(uuidString: "00000000-0000-0000-0000-000000000103")!,
                timestamp: Date(timeIntervalSince1970: 1_700_000_120),
                kind: .browser,
                title: "Issued browser click approval",
                detail: "browser-ABCD1234 - Continue - approval nonce issued as click-nonce-5678EFGH from \(secretFixture("browser-click-nonce"))"
            )
        ]

        let snapshot = events
            .map(AuditTimelineEntry.init(event:))
            .map(timelineSnapshotLine(for:))
            .joined(separator: "\n")

        assertNoRawSecretMaterial(in: snapshot)
        XCTAssertEqual(
            snapshot,
            """
            Connection | Approved | No action needed | Runner paired with token redacted token
            Command approval | Approved | Approved by user | Low risk - git status --short - approval nonce issued as nonce-redacted
            Browser runner | Approved | Approved by user | browser-ABCD1234 - Continue - approval nonce issued as click-nonce-redacted from redacted browser nonce
            """
        )
    }

    func testOfflineFallbackSnapshotRetainsBootstrapModeAndDisablesRunnerTools() {
        let bridge = MockMobileCoreBridge()

        bridge.importBootstrapOutput(
            """
            offline: cached installer metadata unavailable
            fallback: save handoff request and retry later
            endpoint: http://127.0.0.1:8765
            capabilities:
            - shell: unavailable until runner starts
            """
        )

        let snapshot = bootstrapSnapshot(for: bridge)

        assertNoRawSecretMaterial(in: snapshot)
        XCTAssertEqual(
            snapshot,
            """
            mode: Bootstrap
            endpoint: http://127.0.0.1:8765
            account: none
            capabilities: none
            auth: Unavailable | token missing | approval nonce absent
            audit: Imported offline bootstrap fallback | Rescue | Bootstrap handoff imported; offline fallback retained bootstrap mode; secrets redacted before display
            """
        )
    }

    private func bootstrapSnapshot(for bridge: MockMobileCoreBridge) -> String {
        let settings = bridge.connectionSettings
        let auth = bridge.runnerRequestAuthMetadata()
        let auditEvent = bridge.auditEvents.first { $0.kind == .bootstrap } ?? bridge.auditEvents[0]
        let auditEntry = AuditTimelineEntry(event: auditEvent)
        let account = settings.pairedAccountLabel.isEmpty ? "none" : settings.pairedAccountLabel
        let capabilities = settings.discoveredRunnerCapabilities
            .map(\.name)
            .sorted()
            .joined(separator: ", ")

        return """
        mode: \(settings.mode.rawValue)
        endpoint: \(settings.endpoint)
        account: \(account)
        capabilities: \(capabilities.isEmpty ? "none" : capabilities)
        auth: \(auth.readinessLabel) | \(auth.tokenPresent ? "token present" : "token missing") | \(auth.approvalNonce?.isPresent == true ? "approval nonce present" : "approval nonce absent")
        audit: \(auditEntry.title) | \(auditEntry.status) | \(auditEntry.runnerAuditSummary)
        """
    }

    private func timelineSnapshotLine(for entry: AuditTimelineEntry) -> String {
        "\(entry.source) | \(entry.status) | \(entry.userAction) | \(entry.runnerAuditSummary)"
    }

    private func secretFixture(_ kind: String) -> String {
        "deepseek-mobile-\(kind)-SNAPSHOT"
    }

    private func assertNoRawSecretMaterial(in text: String, file: StaticString = #filePath, line: UInt = #line) {
        XCTAssertFalse(text.contains("deepseek-mobile-demo-token"), file: file, line: line)
        XCTAssertFalse(text.contains("deepseek-mobile-approval-nonce"), file: file, line: line)
        XCTAssertFalse(text.contains("deepseek-mobile-browser-click-nonce"), file: file, line: line)
    }
}
