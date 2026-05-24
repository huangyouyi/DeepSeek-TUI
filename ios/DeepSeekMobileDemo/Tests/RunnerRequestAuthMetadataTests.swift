import XCTest
@testable import DeepSeekMobileDemo

@MainActor
final class RunnerRequestAuthMetadataTests: XCTestCase {
    func testRunnerAuthMetadataIsUnavailableBeforePairing() {
        let bridge = MockMobileCoreBridge()
        bridge.connectionSettings.mode = .runner
        bridge.connectionSettings.endpoint = "https://runner.example.test"

        let metadata = bridge.runnerRequestAuthMetadata()

        XCTAssertEqual(metadata.endpoint, "https://runner.example.test")
        XCTAssertEqual(metadata.availability, .unpaired)
        XCTAssertFalse(metadata.tokenPresent)
        XCTAssertNil(metadata.tokenAccountLabel)
        XCTAssertEqual(metadata.readinessLabel, "Unpaired")
    }

    func testRunnerAuthMetadataIsReadyAfterPairingRedeemWithoutExposingToken() {
        let bridge = MockMobileCoreBridge()
        bridge.connectionSettings.mode = .remoteMcp
        bridge.connectionSettings.endpoint = "https://mcp.example.test"

        let code = bridge.requestPairingCode()
        XCTAssertTrue(bridge.redeemPairingCode(code))

        let metadata = bridge.runnerRequestAuthMetadata()

        XCTAssertEqual(metadata.endpoint, "https://mcp.example.test")
        XCTAssertEqual(metadata.availability, .ready)
        XCTAssertTrue(metadata.tokenPresent)
        XCTAssertEqual(metadata.tokenAccountLabel, "Demo remote MCP pairing")
        XCTAssertEqual(metadata.readinessLabel, "Ready")
        XCTAssertFalse(metadata.detailLabel.contains("deepseek-mobile-demo-token"))
        XCTAssertNil(metadata.approvalNonce)
    }

    func testApprovedCommandAddsRedactedApprovalNonceToNextRunnerRequest() throws {
        let bridge = MockMobileCoreBridge()
        bridge.connectionSettings.mode = .runner
        let code = bridge.requestPairingCode()
        XCTAssertTrue(bridge.redeemPairingCode(code))

        let command = try XCTUnwrap(firstPendingCommand(in: bridge))
        bridge.decide(commandID: command.id, decision: .approved)

        let metadata = bridge.runnerRequestAuthMetadata()
        let approvalNonce = try XCTUnwrap(metadata.approvalNonce)
        XCTAssertTrue(approvalNonce.isPresent)
        XCTAssertEqual(approvalNonce.status, .bound)
        XCTAssertTrue(approvalNonce.label.hasPrefix("nonce-"))
        XCTAssertFalse(metadata.detailLabel.contains(command.id.uuidString))
        XCTAssertTrue(bridge.auditEvents.first?.detail.contains("approval nonce issued and bound to command") == true)
    }

    func testDeniedCommandDoesNotIssueApprovalNonce() throws {
        let bridge = MockMobileCoreBridge()
        bridge.connectionSettings.mode = .runner
        let code = bridge.requestPairingCode()
        XCTAssertTrue(bridge.redeemPairingCode(code))

        let command = try XCTUnwrap(firstPendingCommand(in: bridge))
        bridge.decide(commandID: command.id, decision: .denied)

        let metadata = bridge.runnerRequestAuthMetadata()
        XCTAssertNil(metadata.approvalNonce)
        XCTAssertTrue(bridge.auditEvents.first?.detail.contains("approval nonce not issued") == true)
    }

    func testPairingOpensMockBrowserSessionAndClickApprovalWithoutRawNonce() throws {
        let bridge = MockMobileCoreBridge()
        bridge.connectionSettings.mode = .runner
        let selectedSessionID = try XCTUnwrap(bridge.selectedSessionID)

        let code = bridge.requestPairingCode()
        XCTAssertTrue(bridge.redeemPairingCode(code))

        let browserSession = try XCTUnwrap(bridge.browserSession(for: selectedSessionID))
        XCTAssertEqual(browserSession.status, .waitingForClickApproval)
        XCTAssertTrue(browserSession.id.hasPrefix("browser-"))
        XCTAssertTrue(browserSession.extractedTextPreview?.contains("Runner pairing ready") == true)
        XCTAssertNotNil(browserSession.pendingClickApproval)
        XCTAssertFalse(browserSession.clickApprovalNonceLabel.contains("deepseek-mobile-browser-click-nonce"))
        XCTAssertTrue(bridge.auditEvents.contains { $0.title == "Opened browser session" })
        XCTAssertTrue(bridge.auditEvents.contains { $0.title == "Extracted browser text" })

        let approval = try XCTUnwrap(browserSession.pendingClickApproval)
        bridge.decide(browserClickApprovalID: approval.id, decision: .approved)

        let approvedSession = try XCTUnwrap(bridge.browserSession(for: selectedSessionID))
        XCTAssertNil(approvedSession.pendingClickApproval)
        XCTAssertEqual(approvedSession.clickApprovalNonceStatus, .bound)
        XCTAssertTrue(approvedSession.clickApprovalNonceLabel.hasPrefix("click-nonce-"))
        XCTAssertFalse(bridge.auditEvents.first?.detail.contains("deepseek-mobile-browser-click-nonce") == true)
        XCTAssertTrue(bridge.auditEvents.first?.detail.contains("approval nonce issued and bound") == true)
    }

    func testPairingUpgradeBridgeUpdatesRunnerProfileWithoutRawToken() throws {
        let provider = BridgePairingUpgradeProvider(
            profilePayload: """
            {
              "endpoint": "https://runner.example.test",
              "account_label": "Core paired runner",
              "token_label": "PAIRING-TOKEN-SECRET",
              "capabilities": [
                {"name": "shell", "summary": "Run approved shell commands"},
                {"name": "browser", "summary": "Drive browser automation"}
              ]
            }
            """
        )
        let bridge = MockMobileCoreBridge(coreUniFFIProvider: provider)

        let profile = bridge.upgradePairing(
            endpoint: "https://runner.example.test",
            pairingToken: "PAIRING-TOKEN-SECRET",
            capabilitiesJSON: """
            {"capabilities": [{"name": "shell"}, {"name": "browser"}]}
            """
        )

        XCTAssertEqual(provider.pairingUpgradeRequests.count, 1)
        XCTAssertEqual(profile.accountLabel, "Core paired runner")
        XCTAssertEqual(profile.tokenLabel, "redacted pairing token")
        XCTAssertFalse(profile.isMockFallback)
        XCTAssertEqual(bridge.connectionSettings.mode, .runner)
        XCTAssertEqual(bridge.connectionSettings.endpoint, "https://runner.example.test")
        XCTAssertEqual(bridge.connectionSettings.pairedAccountLabel, "Core paired runner")
        XCTAssertEqual(bridge.connectionSettings.tokenLabel, "redacted pairing token")
        XCTAssertTrue(bridge.connectionSettings.discoveredRunnerCapabilities.supports(.shell))
        XCTAssertTrue(bridge.connectionSettings.discoveredRunnerCapabilities.supports(.browser))

        let visibleText = [
            profile.accountLabel,
            profile.tokenLabel,
            profile.capabilities.map(\.summary).joined(separator: "\n"),
            bridge.connectionSettings.statusSummary,
            bridge.auditEvents.map(\.detail).joined(separator: "\n")
        ].joined(separator: "\n")
        XCTAssertFalse(visibleText.contains("PAIRING-TOKEN-SECRET"))
        XCTAssertFalse(visibleText.contains("pairing_token"))
    }

    func testPairingUpgradeBridgeFallbackUsesCapabilitiesJSONWithoutRawToken() throws {
        let bridge = MockMobileCoreBridge(coreUniFFIProvider: BridgeFailingPairingUpgradeProvider())

        let profile = bridge.upgradePairing(
            endpoint: "https://runner.example.test",
            pairingToken: "PAIRING-TOKEN-SECRET",
            capabilitiesJSON: """
            {"capabilities": [{"name": "powershell", "summary": "Run approved PowerShell commands with PAIRING-TOKEN-SECRET"}, "mcp_proxy"]}
            """
        )

        XCTAssertTrue(profile.isMockFallback)
        XCTAssertTrue(profile.capabilities.supports(.shell))
        XCTAssertTrue(profile.capabilities.supports(.mcp))
        XCTAssertEqual(bridge.connectionSettings.pairedAccountLabel, "Mock upgraded runner")
        XCTAssertEqual(bridge.connectionSettings.tokenLabel, "Mock pairing credential")
        XCTAssertFalse(try profile.runnerProfileJSON().contains("PAIRING-TOKEN-SECRET"))
        XCTAssertTrue(bridge.auditEvents.first?.detail.contains("mock fallback") == true)
    }

    func testImportingRunnerCapabilityReportPromotesBootstrapToRunnerWithoutRawSecrets() {
        let bridge = MockMobileCoreBridge()

        bridge.importBootstrapOutput(
            """
            deepseek bootstrap handoff
            endpoint: http://127.0.0.1:8765
            pairing: ready
            runner: connected
            account: Demo imported runner
            capabilities:
            - shell: Run approved shell commands
            - browser: Drive browser automation
            - maintenance: Plan self-update and uninstall dry-runs
            """
        )

        XCTAssertEqual(bridge.connectionSettings.mode, .runner)
        XCTAssertEqual(bridge.connectionSettings.endpoint, "http://127.0.0.1:8765")
        XCTAssertEqual(bridge.connectionSettings.pairedAccountLabel, "Demo imported runner")
        XCTAssertTrue(bridge.connectionSettings.discoveredRunnerCapabilities.supports(.shell))
        XCTAssertTrue(bridge.connectionSettings.discoveredRunnerCapabilities.supports(.browser))
        XCTAssertTrue(bridge.connectionSettings.discoveredRunnerCapabilities.supports(.maintenance))
        XCTAssertFalse(bridge.connectionSettings.bootstrapOutput.contains("deepseek-mobile-demo-token"))
        XCTAssertFalse(bridge.connectionSettings.bootstrapOutput.contains("deepseek-mobile-approval-nonce"))
    }

    func testImportingOfflineFallbackKeepsBootstrapModeAndNoRunnerCapabilities() {
        let bridge = MockMobileCoreBridge()

        bridge.importBootstrapOutput(
            """
            offline: cached installer metadata unavailable
            fallback: save handoff request and retry later
            endpoint: http://127.0.0.1:8765
            capabilities: unavailable until runner starts
            """
        )

        XCTAssertEqual(bridge.connectionSettings.mode, .bootstrap)
        XCTAssertTrue(bridge.connectionSettings.discoveredRunnerCapabilities.isEmpty)
        XCTAssertTrue(bridge.connectionSettings.pairedAccountLabel.isEmpty)
        XCTAssertTrue(bridge.auditEvents.first?.detail.contains("offline fallback retained bootstrap mode") == true)
    }

    private func firstPendingCommand(in bridge: MockMobileCoreBridge) -> PendingCommand? {
        for session in bridge.sessions {
            if let command = bridge.pendingCommand(for: session.id) {
                return command
            }
        }

        return nil
    }
}

private final class BridgePairingUpgradeProvider: CoreBridgeUniFFIJSONProvider {
    private let profilePayload: String
    private(set) var pairingUpgradeRequests: [String] = []

    init(profilePayload: String) {
        self.profilePayload = profilePayload
    }

    func bootstrapJSON() throws -> String { BridgePayloads.bootstrap }
    func auditJSON() throws -> String { BridgePayloads.audit }
    func browserJSON() throws -> String { BridgePayloads.browser }
    func maintenanceJSON() throws -> String { BridgePayloads.maintenance }

    func pairingUpgradeProfileJSON(requestJSON: String) throws -> String {
        pairingUpgradeRequests.append(requestJSON)
        return profilePayload
    }
}

private struct BridgeFailingPairingUpgradeProvider: CoreBridgeUniFFIJSONProvider {
    func bootstrapJSON() throws -> String { BridgePayloads.bootstrap }
    func auditJSON() throws -> String { BridgePayloads.audit }
    func browserJSON() throws -> String { BridgePayloads.browser }
    func maintenanceJSON() throws -> String { BridgePayloads.maintenance }
}

private enum BridgePayloads {
    static let bootstrap = """
    {
      "connection": {"mode": "bootstrap"},
      "sessions": [],
      "messages": []
    }
    """

    static let audit = #"{"events": []}"#
    static let browser = #"{"sessions": []}"#
    static let maintenance = #"{"requests": []}"#
}
