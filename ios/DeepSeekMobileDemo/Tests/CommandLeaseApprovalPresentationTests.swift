import XCTest
@testable import DeepSeekMobileDemo

@MainActor
final class CommandLeaseApprovalPresentationTests: XCTestCase {
    func testMockPendingCommandCarriesLeaseApprovalMetadataWithoutTokenMaterial() throws {
        let bridge = MockMobileCoreBridge()
        let command = try XCTUnwrap(firstPendingCommand(in: bridge))

        XCTAssertTrue(command.lease.leaseIDLabel.hasPrefix("lease-"))
        XCTAssertTrue(command.lease.idempotencyKeyLabel.hasPrefix("idem-"))
        XCTAssertGreaterThan(command.lease.expiresAt, Date())
        XCTAssertEqual(
            command.lease.approvedActionSummary,
            "Run read-only shell command: git status --short"
        )

        let visibleText = [
            command.lease.leaseIDLabel,
            command.lease.idempotencyKeyLabel,
            command.lease.expirySummary,
            command.lease.approvedActionSummary,
            command.approvalNonceLabel
        ].joined(separator: "\n")

        XCTAssertFalse(visibleText.localizedCaseInsensitiveContains("bearer"))
        XCTAssertFalse(visibleText.localizedCaseInsensitiveContains("pairing token"))
        XCTAssertFalse(visibleText.localizedCaseInsensitiveContains("deepseek-mobile-demo-token"))
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
