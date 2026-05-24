import XCTest
@testable import DeepSeekMobileDemo

final class AuditTimelinePresentationTests: XCTestCase {
    func testCommandLeaseLifecycleDisplaysWhitelistedMetadataAndRedactsSecrets() {
        let cases: [(title: String, detail: String, lifecycle: String, status: String)] = [
            (
                "Command lease nonce issued",
                "nonce issued call_id=call_001 tool=remote.shell.exec lease_id=lease-raw-123 idempotency_key=idem-secret command=\"git status\" env={TOKEN=secret}",
                "Nonce issued",
                "Waiting"
            ),
            (
                "Command lease accepted",
                "lease accepted call_id=call_002 tool=remote.shell.exec bearer=Bearer raw-token pairing_token=pair-raw",
                "Lease accepted",
                "Approved"
            ),
            (
                "Command lease consumed",
                "lease consumed call_id=call_003 tool=remote.shell.exec",
                "Lease consumed",
                "Approved"
            ),
            (
                "Command lease replay rejected",
                "lease replay rejected call_id=call_004 tool=remote.shell.exec error_code=lease_replay lease_id=lease-raw-456",
                "Replay rejected",
                "Denied"
            ),
            (
                "Command lease expired rejected",
                "lease expired rejected call_id=call_005 tool=remote.shell.exec error_code=lease_expired",
                "Expired rejected",
                "Denied"
            ),
            (
                "Command lease invalid rejected",
                "lease invalid rejected call_id=call_006 tool=remote.shell.exec error_code=lease_invalid idempotency_secret=idem-secret-2",
                "Invalid rejected",
                "Denied"
            )
        ]

        for testCase in cases {
            let entry = AuditTimelineEntry(event: AuditEvent(
                id: UUID(),
                timestamp: Date(),
                kind: .command,
                title: testCase.title,
                detail: testCase.detail
            ))

            XCTAssertEqual(entry.leaseLifecycleLabel, testCase.lifecycle)
            XCTAssertEqual(entry.status, testCase.status)
            XCTAssertTrue(entry.commandLeaseMetadata.contains("call_id=call_"))
            XCTAssertTrue(entry.commandLeaseMetadata.contains("tool=remote.shell.exec"))
            XCTAssertFalse(entry.commandLeaseMetadata.contains("lease_id"))
            XCTAssertFalse(entry.commandLeaseMetadata.contains("idempotency"))
            XCTAssertFalse(entry.commandLeaseMetadata.contains("command="))
            XCTAssertFalse(entry.commandLeaseMetadata.contains("env="))
            assertNoCommandLeaseSecretMaterial(in: entry.runnerAuditSummary)
            assertNoCommandLeaseSecretMaterial(in: entry.commandLeaseMetadata)
        }
    }

    func testTimelineEntryRedactsRawTokenAndNonceMaterial() {
        let event = AuditEvent(
            id: UUID(),
            timestamp: Date(),
            kind: .browser,
            title: "Issued browser click approval",
            detail: "token deepseek-mobile-demo-token-123 nonce deepseek-mobile-browser-click-nonce-456 approval nonce issued"
        )

        let entry = AuditTimelineEntry(event: event)

        XCTAssertFalse(entry.runnerAuditSummary.contains("deepseek-mobile-demo-token"))
        XCTAssertFalse(entry.runnerAuditSummary.contains("deepseek-mobile-browser-click-nonce"))
        XCTAssertTrue(entry.runnerAuditSummary.contains("redacted token"))
        XCTAssertTrue(entry.runnerAuditSummary.contains("redacted browser nonce"))
    }

    func testTimelineFiltersMatchMockAuditGroups() {
        let events = [
            AuditEvent(id: UUID(), timestamp: Date(), kind: .maintenance, title: "Maintenance", detail: "planned"),
            AuditEvent(id: UUID(), timestamp: Date(), kind: .bootstrap, title: "Bootstrap", detail: "imported"),
            AuditEvent(id: UUID(), timestamp: Date(), kind: .windows, title: "Windows", detail: "rescue"),
            AuditEvent(id: UUID(), timestamp: Date(), kind: .browser, title: "Browser", detail: "pending"),
            AuditEvent(id: UUID(), timestamp: Date(), kind: .connection, title: "Connection", detail: "saved")
        ]

        XCTAssertEqual(events.filter { AuditTimelineFilter.all.includes($0) }.count, 5)
        XCTAssertEqual(events.filter { AuditTimelineFilter.maintenance.includes($0) }.map(\.kind), [.maintenance])
        XCTAssertEqual(events.filter { AuditTimelineFilter.bootstrap.includes($0) }.map(\.kind), [.bootstrap])
        XCTAssertEqual(events.filter { AuditTimelineFilter.windows.includes($0) }.map(\.kind), [.windows])
        XCTAssertEqual(events.filter { AuditTimelineFilter.browser.includes($0) }.map(\.kind), [.browser])
    }

    private func assertNoCommandLeaseSecretMaterial(in text: String, file: StaticString = #filePath, line: UInt = #line) {
        XCTAssertFalse(text.contains("Bearer"), file: file, line: line)
        XCTAssertFalse(text.contains("raw-token"), file: file, line: line)
        XCTAssertFalse(text.contains("pairing_token"), file: file, line: line)
        XCTAssertFalse(text.contains("pair-raw"), file: file, line: line)
        XCTAssertFalse(text.contains("lease-raw"), file: file, line: line)
        XCTAssertFalse(text.contains("idem-secret"), file: file, line: line)
        XCTAssertFalse(text.contains("command="), file: file, line: line)
        XCTAssertFalse(text.contains("command=\"git status\""), file: file, line: line)
        XCTAssertFalse(text.contains("env="), file: file, line: line)
        XCTAssertFalse(text.contains("env={TOKEN=secret}"), file: file, line: line)
    }
}
