import XCTest
@testable import DeepSeekMobileDemo

final class AuditTimelinePresentationTests: XCTestCase {
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
}
