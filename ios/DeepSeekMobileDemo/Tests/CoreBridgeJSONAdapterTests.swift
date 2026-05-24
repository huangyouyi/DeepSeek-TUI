import XCTest
@testable import DeepSeekMobileDemo

@MainActor
final class CoreBridgeJSONAdapterTests: XCTestCase {
    func testPairingUpgradeCallSiteReturnsRunnerProfileWithoutLeakingToken() throws {
        let provider = RecordingPairingUpgradeProvider(
            profilePayload: """
            {
              "endpoint": "https://runner.example.test",
              "account_label": "Core upgraded runner",
              "token_label": "PAIRING-TOKEN-SECRET",
              "capabilities": [
                {"name": "shell", "summary": "Run approved shell commands"},
                {"name": "browser", "summary": "Drive browser automation"}
              ]
            }
            """
        )

        let profile = CoreBridgeJSONAdapter(provider: provider).pairingUpgradeProfile(
            endpoint: "https://runner.example.test",
            pairingToken: "PAIRING-TOKEN-SECRET",
            capabilitiesJSON: """
            {"capabilities": [{"name": "shell"}, {"name": "browser"}]}
            """
        )

        XCTAssertEqual(provider.requests.count, 1)
        XCTAssertTrue(provider.requests[0].contains("PAIRING-TOKEN-SECRET"))
        XCTAssertEqual(profile.endpoint, "https://runner.example.test")
        XCTAssertEqual(profile.accountLabel, "Core upgraded runner")
        XCTAssertEqual(profile.tokenLabel, "redacted pairing token")
        XCTAssertFalse(profile.isMockFallback)
        XCTAssertTrue(profile.capabilities.supports(.shell))
        XCTAssertTrue(profile.capabilities.supports(.browser))

        assertNoPairingUpgradeSecretMaterial(in: try profile.runnerProfileJSON())
    }

    func testPairingUpgradeFallbackMapsCapabilitiesJSON() throws {
        let profile = CoreBridgeJSONAdapter(provider: FailingPairingUpgradeProvider()).pairingUpgradeProfile(
            endpoint: "https://runner.example.test",
            pairingToken: "PAIRING-TOKEN-SECRET",
            capabilitiesJSON: """
            {
              "capabilities": [
                {"name": "powershell", "summary": "Run approved PowerShell commands"},
                "browser",
                {"name": "mcp_proxy", "summary": "Proxy approved MCP tools"}
              ]
            }
            """
        )

        XCTAssertEqual(profile.endpoint, "https://runner.example.test")
        XCTAssertEqual(profile.accountLabel, "Mock upgraded runner")
        XCTAssertEqual(profile.tokenLabel, "Mock pairing credential")
        XCTAssertTrue(profile.isMockFallback)
        XCTAssertTrue(profile.capabilities.supports(.shell))
        XCTAssertTrue(profile.capabilities.supports(.browser))
        XCTAssertTrue(profile.capabilities.supports(.mcp))
        XCTAssertEqual(profile.capabilities.first?.summary, "Run approved PowerShell commands")
        assertNoPairingUpgradeSecretMaterial(in: try profile.runnerProfileJSON())
    }

    func testPairingUpgradeFallbackSurvivesInvalidCapabilitiesJSONWithoutTokenLeak() throws {
        let profile = CoreBridgeJSONAdapter(provider: FailingPairingUpgradeProvider()).pairingUpgradeProfile(
            endpoint: "https://runner.example.test",
            pairingToken: "PAIRING-TOKEN-SECRET",
            capabilitiesJSON: "PAIRING-TOKEN-SECRET is not JSON"
        )

        XCTAssertTrue(profile.isMockFallback)
        XCTAssertEqual(profile.capabilities, [])
        assertNoPairingUpgradeSecretMaterial(in: try profile.runnerProfileJSON())
    }

    func testUniFFICallSiteRequestsEachJSONPayloadThroughInjectedProvider() throws {
        let provider = RecordingUniFFIProvider(
            bootstrapPayload: Self.bootstrapPayload(),
            auditPayload: Self.auditPayload(),
            browserPayload: Self.browserPayload(),
            maintenancePayload: Self.maintenancePayload()
        )

        let state = try CoreBridgeJSONAdapter(provider: provider).initialState(
            fallbackSnapshot: Self.emptySnapshot(),
            fallbackSettings: .demo
        )

        XCTAssertEqual(provider.calls, [.bootstrap, .audit, .browser, .maintenance])
        XCTAssertEqual(state.connectionSettings.mode, .runner)
        assertNoCoreSecretMaterial(in: state.connectionSettings.bootstrapOutput)
        assertNoCoreSecretMaterial(in: state.snapshot.auditEvents.map(\.detail).joined(separator: "\n"))
        assertNoCoreSecretMaterial(
            in: state.snapshot.browserSessionsBySession.values.map(\.clickApprovalNonceLabel).joined(separator: "\n")
        )
        assertNoCoreSecretMaterial(
            in: state.snapshot.maintenanceRequestsBySession.values.map(\.auditSummary).joined(separator: "\n")
        )
    }

    func testUniFFICallSiteFallbackKeepsMockSnapshotWhenAnyJSONPayloadIsUnavailable() {
        for missingCall in CoreBridgeUniFFICall.allCases {
            let provider = SelectivelyFailingUniFFIProvider(missingCall: missingCall)
            let bridge = MockMobileCoreBridge(coreUniFFIProvider: provider)

            XCTAssertFalse(bridge.sessions.isEmpty, "Expected mock sessions for missing \(missingCall)")
            XCTAssertEqual(bridge.connectionSettings, .demo, "Expected demo settings for missing \(missingCall)")
            assertNoCoreSecretMaterial(in: bridge.auditEvents.map(\.detail).joined(separator: "\n"))
        }
    }

    func testMockBridgeCanSeedDemoStateFromCoreJSONShape() throws {
        let sessionID = UUID(uuidString: "11111111-1111-1111-1111-111111111111")!
        let browserApprovalID = UUID(uuidString: "22222222-2222-2222-2222-222222222222")!
        let maintenanceID = UUID(uuidString: "33333333-3333-3333-3333-333333333333")!
        let auditID = UUID(uuidString: "44444444-4444-4444-4444-444444444444")!
        let api = MockCoreBridgeJSONAPI(
            bootstrapPayload: """
            {
              "connection": {
                "mode": "runner",
                "endpoint": "http://127.0.0.1:8765",
                "account_label": "Core paired runner",
                "bootstrap_output": "token: deepseek-mobile-demo-token-CORE\\nnonce: deepseek-mobile-approval-nonce-CORE",
                "capabilities": [
                  {"name": "browser", "summary": "Drive browser automation"},
                  {"name": "maintenance", "summary": "Plan dry-runs"}
                ]
              },
              "sessions": [
                {
                  "id": "\(sessionID.uuidString)",
                  "title": "Core seeded chat",
                  "subtitle": "From UniFFI JSON mock",
                  "status": "waiting_for_approval",
                  "updated_at_unix": 1700000000,
                  "unread_count": 2
                }
              ],
              "messages": [
                {
                  "id": "55555555-5555-5555-5555-555555555555",
                  "session_id": "\(sessionID.uuidString)",
                  "role": "assistant",
                  "body": "Seeded from core JSON.",
                  "timestamp_unix": 1700000010,
                  "is_thinking": false
                }
              ]
            }
            """,
            auditPayload: """
            {
              "events": [
                {
                  "id": "\(auditID.uuidString)",
                  "timestamp_unix": 1700000020,
                  "kind": "bootstrap",
                  "title": "Imported core bootstrap",
                  "detail": "token deepseek-mobile-demo-token-CORE nonce deepseek-mobile-approval-nonce-CORE"
                }
              ]
            }
            """,
            browserPayload: """
            {
              "sessions": [
                {
                  "app_session_id": "\(sessionID.uuidString)",
                  "browser_session_id": "browser-core",
                  "status": "waiting_for_click_approval",
                  "page_title": "Core Browser",
                  "page_url": "https://runner.example.test/core",
                  "extracted_text_preview": "Core extracted text",
                  "click_approval_nonce_label": "deepseek-mobile-browser-click-nonce-CORE",
                  "click_approval_nonce_status": "will_issue_on_approval",
                  "pending_click_approval": {
                    "id": "\(browserApprovalID.uuidString)",
                    "target_description": "Continue",
                    "page_url": "https://runner.example.test/core",
                    "rationale": "Core requested a browser click.",
                    "approval_nonce_label": "deepseek-mobile-browser-click-nonce-CORE",
                    "approval_nonce_status": "will_issue_on_approval"
                  }
                }
              ]
            }
            """,
            maintenancePayload: """
            {
              "requests": [
                {
                  "id": "\(maintenanceID.uuidString)",
                  "app_session_id": "\(sessionID.uuidString)",
                  "operation": "self_update",
                  "title": "Core maintenance dry-run",
                  "connection_id_label": "runner-core",
                  "target_summary": "deepseek runner 0.6.6 -> 0.6.7",
                  "risk": "high",
                  "approval_status": "waiting_for_dry_run_approval",
                  "dry_run_steps": [
                    {
                      "id": "66666666-6666-6666-6666-666666666666",
                      "title": "Verify manifest",
                      "detail": "Would verify release metadata.",
                      "destructive": false
                    }
                  ],
                  "artifact_verification": {
                    "artifact_name": "deepseek-tui-aarch64-apple-darwin.tar.gz",
                    "checksum_source": "release manifest sha256",
                    "signature_source": "minisign release signature",
                    "verify_step": "Verify sha256 checksum and minisign signature before staging.",
                    "rollback_guidance": "Keep current runner binary until execution approval succeeds."
                  },
                  "audit_summary": "planned with deepseek-mobile-approval-nonce-CORE",
                  "requested_by": "core mock"
                }
              ]
            }
            """
        )

        let bridge = MockMobileCoreBridge(coreJSONAPI: api)
        let settings = bridge.connectionSettings
        let auth = bridge.runnerRequestAuthMetadata()
        let browser = bridge.browserSession(for: sessionID)
        let maintenance = bridge.maintenanceApproval(for: sessionID)

        XCTAssertEqual(bridge.sessions.map(\.id), [sessionID])
        XCTAssertEqual(bridge.messages(for: sessionID).first?.body, "Seeded from core JSON.")
        XCTAssertEqual(settings.mode, .runner)
        XCTAssertEqual(settings.endpoint, "http://127.0.0.1:8765")
        XCTAssertEqual(settings.pairedAccountLabel, "Core paired runner")
        XCTAssertTrue(settings.discoveredRunnerCapabilities.supports(.browser))
        XCTAssertTrue(settings.discoveredRunnerCapabilities.supports(.maintenance))
        XCTAssertEqual(auth.availability, .ready)
        XCTAssertEqual(browser?.id, "browser-core")
        XCTAssertEqual(browser?.clickApprovalNonceLabel, "redacted browser nonce")
        XCTAssertEqual(browser?.pendingClickApproval?.approvalNonceLabel, "redacted browser nonce")
        XCTAssertEqual(maintenance?.operation, .selfUpdate)
        XCTAssertEqual(maintenance?.auditSummary, "planned with redacted nonce")
        XCTAssertEqual(maintenance?.artifactVerification?.artifactName, "deepseek-tui-aarch64-apple-darwin.tar.gz")
        XCTAssertEqual(maintenance?.artifactVerification?.checksumSource, "release manifest sha256")
        XCTAssertEqual(maintenance?.artifactVerification?.signatureSource, "minisign release signature")
        XCTAssertEqual(maintenance?.artifactVerification?.verifyStep, "Verify sha256 checksum and minisign signature before staging.")
        XCTAssertEqual(maintenance?.artifactVerification?.rollbackGuidance, "Keep current runner binary until execution approval succeeds.")

        let visibleText = [
            settings.bootstrapOutput,
            bridge.auditEvents.map { AuditTimelineEntry(event: $0).runnerAuditSummary }.joined(separator: "\n"),
            browser?.clickApprovalNonceLabel ?? "",
            browser?.pendingClickApproval?.approvalNonceLabel ?? "",
            maintenance?.auditSummary ?? "",
            maintenance?.artifactVerification?.artifactName ?? "",
            maintenance?.artifactVerification?.checksumSource ?? "",
            maintenance?.artifactVerification?.signatureSource ?? "",
            maintenance?.artifactVerification?.verifyStep ?? "",
            maintenance?.artifactVerification?.rollbackGuidance ?? ""
        ].joined(separator: "\n")
        assertNoCoreSecretMaterial(in: visibleText)
    }

    func testAdapterFallsBackToMockSnapshotWhenCoreJSONIsUnavailable() {
        let bridge = MockMobileCoreBridge(coreJSONAPI: FailingCoreBridgeJSONAPI())

        XCTAssertFalse(bridge.sessions.isEmpty)
        XCTAssertEqual(bridge.connectionSettings, .demo)
    }

    private func assertNoCoreSecretMaterial(in text: String, file: StaticString = #filePath, line: UInt = #line) {
        XCTAssertFalse(text.contains("deepseek-mobile-demo-token"), file: file, line: line)
        XCTAssertFalse(text.contains("deepseek-mobile-approval-nonce"), file: file, line: line)
        XCTAssertFalse(text.contains("deepseek-mobile-browser-click-nonce"), file: file, line: line)
    }

    private func assertNoPairingUpgradeSecretMaterial(in text: String, file: StaticString = #filePath, line: UInt = #line) {
        XCTAssertFalse(text.contains("PAIRING-TOKEN-SECRET"), file: file, line: line)
        XCTAssertFalse(text.contains("pairing_token"), file: file, line: line)
    }

    fileprivate static func emptySnapshot() -> DemoSnapshot {
        DemoSnapshot(
            sessions: [],
            messagesBySession: [:],
            pendingCommandsBySession: [:],
            browserSessionsBySession: [:],
            maintenanceRequestsBySession: [:],
            auditEvents: []
        )
    }

    fileprivate static func bootstrapPayload() -> String {
        """
        {
          "connection": {
            "mode": "runner",
            "endpoint": "http://127.0.0.1:8765",
            "account_label": "Core paired runner",
            "bootstrap_output": "token: deepseek-mobile-demo-token-CALLSITE\\nnonce: deepseek-mobile-approval-nonce-CALLSITE",
            "capabilities": [
              {"name": "browser", "summary": "Drive browser automation"},
              {"name": "maintenance", "summary": "Plan dry-runs"}
            ]
          },
          "sessions": [
            {
              "id": "11111111-1111-1111-1111-111111111111",
              "title": "Call-site seeded chat",
              "subtitle": "From UniFFI JSON scaffold",
              "status": "ready",
              "updated_at_unix": 1700000000,
              "unread_count": 0
            }
          ],
          "messages": []
        }
        """
    }

    fileprivate static func auditPayload() -> String {
        """
        {
          "events": [
            {
              "id": "44444444-4444-4444-4444-444444444444",
              "timestamp_unix": 1700000020,
              "kind": "bootstrap",
              "title": "Imported core bootstrap",
              "detail": "token deepseek-mobile-demo-token-CALLSITE nonce deepseek-mobile-approval-nonce-CALLSITE"
            }
          ]
        }
        """
    }

    fileprivate static func browserPayload() -> String {
        """
        {
          "sessions": [
            {
              "app_session_id": "11111111-1111-1111-1111-111111111111",
              "browser_session_id": "browser-callsite",
              "status": "waiting_for_click_approval",
              "page_title": "Core Browser",
              "page_url": "https://runner.example.test/core",
              "extracted_text_preview": "Core extracted text with deepseek-mobile-demo-token-CALLSITE",
              "click_approval_nonce_label": "deepseek-mobile-browser-click-nonce-CALLSITE",
              "click_approval_nonce_status": "will_issue_on_approval",
              "pending_click_approval": {
                "id": "22222222-2222-2222-2222-222222222222",
                "target_description": "Continue",
                "page_url": "https://runner.example.test/core",
                "rationale": "Core requested a browser click with deepseek-mobile-approval-nonce-CALLSITE.",
                "approval_nonce_label": "deepseek-mobile-browser-click-nonce-CALLSITE",
                "approval_nonce_status": "will_issue_on_approval"
              }
            }
          ]
        }
        """
    }

    fileprivate static func maintenancePayload() -> String {
        """
        {
          "requests": [
            {
              "id": "33333333-3333-3333-3333-333333333333",
              "app_session_id": "11111111-1111-1111-1111-111111111111",
              "operation": "self_update",
              "title": "Core maintenance dry-run",
              "connection_id_label": "runner-core",
              "target_summary": "deepseek runner 0.6.6 -> 0.6.7",
              "risk": "high",
              "approval_status": "waiting_for_dry_run_approval",
              "dry_run_steps": [
                {
                  "id": "66666666-6666-6666-6666-666666666666",
                  "title": "Verify manifest",
                  "detail": "Would verify release metadata with deepseek-mobile-demo-token-CALLSITE.",
                  "destructive": false
                }
              ],
              "audit_summary": "planned with deepseek-mobile-approval-nonce-CALLSITE",
              "requested_by": "core mock"
            }
          ]
        }
        """
    }
}

private struct FailingCoreBridgeJSONAPI: CoreBridgeJSONAPI {
    func bootstrapJSON() throws -> String { throw CoreBridgeJSONError.missingPayload("bootstrap") }
    func auditJSON() throws -> String { throw CoreBridgeJSONError.missingPayload("audit") }
    func browserJSON() throws -> String { throw CoreBridgeJSONError.missingPayload("browser") }
    func maintenanceJSON() throws -> String { throw CoreBridgeJSONError.missingPayload("maintenance") }
}

private final class RecordingPairingUpgradeProvider: CoreBridgeUniFFIJSONProvider {
    private let profilePayload: String
    private(set) var requests: [String] = []

    init(profilePayload: String) {
        self.profilePayload = profilePayload
    }

    func bootstrapJSON() throws -> String { CoreBridgeJSONAdapterTests.bootstrapPayload() }
    func auditJSON() throws -> String { CoreBridgeJSONAdapterTests.auditPayload() }
    func browserJSON() throws -> String { CoreBridgeJSONAdapterTests.browserPayload() }
    func maintenanceJSON() throws -> String { CoreBridgeJSONAdapterTests.maintenancePayload() }

    func pairingUpgradeProfileJSON(requestJSON: String) throws -> String {
        requests.append(requestJSON)
        return profilePayload
    }
}

private struct FailingPairingUpgradeProvider: CoreBridgeUniFFIJSONProvider {
    func bootstrapJSON() throws -> String { CoreBridgeJSONAdapterTests.bootstrapPayload() }
    func auditJSON() throws -> String { CoreBridgeJSONAdapterTests.auditPayload() }
    func browserJSON() throws -> String { CoreBridgeJSONAdapterTests.browserPayload() }
    func maintenanceJSON() throws -> String { CoreBridgeJSONAdapterTests.maintenancePayload() }
}

private final class RecordingUniFFIProvider: CoreBridgeUniFFIJSONProvider {
    private let bootstrapPayload: String
    private let auditPayload: String
    private let browserPayload: String
    private let maintenancePayload: String
    private(set) var calls: [CoreBridgeUniFFICall] = []

    init(bootstrapPayload: String, auditPayload: String, browserPayload: String, maintenancePayload: String) {
        self.bootstrapPayload = bootstrapPayload
        self.auditPayload = auditPayload
        self.browserPayload = browserPayload
        self.maintenancePayload = maintenancePayload
    }

    func bootstrapJSON() throws -> String {
        calls.append(.bootstrap)
        return bootstrapPayload
    }

    func auditJSON() throws -> String {
        calls.append(.audit)
        return auditPayload
    }

    func browserJSON() throws -> String {
        calls.append(.browser)
        return browserPayload
    }

    func maintenanceJSON() throws -> String {
        calls.append(.maintenance)
        return maintenancePayload
    }
}

private struct SelectivelyFailingUniFFIProvider: CoreBridgeUniFFIJSONProvider {
    var missingCall: CoreBridgeUniFFICall

    func bootstrapJSON() throws -> String {
        try payload(for: .bootstrap, text: CoreBridgeJSONAdapterTests.bootstrapPayload())
    }

    func auditJSON() throws -> String {
        try payload(for: .audit, text: CoreBridgeJSONAdapterTests.auditPayload())
    }

    func browserJSON() throws -> String {
        try payload(for: .browser, text: CoreBridgeJSONAdapterTests.browserPayload())
    }

    func maintenanceJSON() throws -> String {
        try payload(for: .maintenance, text: CoreBridgeJSONAdapterTests.maintenancePayload())
    }

    private func payload(for call: CoreBridgeUniFFICall, text: String) throws -> String {
        if call == missingCall {
            throw CoreBridgeJSONError.missingPayload(call.rawValue)
        }
        return text
    }
}
