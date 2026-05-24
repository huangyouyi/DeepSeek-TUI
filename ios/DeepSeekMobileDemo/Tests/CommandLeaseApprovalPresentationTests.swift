import XCTest
@testable import DeepSeekMobileDemo

@MainActor
final class CommandLeaseApprovalPresentationTests: XCTestCase {
    func testLeaseApprovalPresentationExplainsOneTimeExpiryBoundExecution() throws {
        let bridge = MockMobileCoreBridge()
        let command = try XCTUnwrap(firstPendingCommand(in: bridge))

        XCTAssertEqual(
            command.lease.executionConstraintSummary,
            "This approval allows this action to execute once before \(command.lease.expirySummary)."
        )
        XCTAssertEqual(
            command.lease.approvalButtonSummary,
            "Approve once before \(command.lease.expirySummary)"
        )
        XCTAssertEqual(
            command.lease.boundActionSummary,
            "One-time action: Run read-only shell command: git status --short"
        )
    }

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

    func testLeaseApprovalPresentationDoesNotLeakTokenNonceOrCommandSecrets() throws {
        let bridge = MockMobileCoreBridge()
        let command = try XCTUnwrap(firstPendingCommand(in: bridge))

        let visibleText = [
            command.title,
            command.command,
            command.workingDirectory,
            command.rationale,
            command.lease.leaseIDLabel,
            command.lease.idempotencyKeyLabel,
            command.lease.expirySummary,
            command.lease.approvedActionSummary,
            command.lease.executionConstraintSummary,
            command.lease.approvalButtonSummary,
            command.lease.boundActionSummary,
            command.approvalNonceLabel
        ].joined(separator: "\n")

        XCTAssertFalse(visibleText.localizedCaseInsensitiveContains("bearer"))
        XCTAssertFalse(visibleText.localizedCaseInsensitiveContains("pairing_token"))
        XCTAssertFalse(visibleText.localizedCaseInsensitiveContains("pairing token"))
        XCTAssertFalse(visibleText.localizedCaseInsensitiveContains("deepseek-mobile-demo-token"))
        XCTAssertFalse(visibleText.localizedCaseInsensitiveContains("deepseek-mobile-approval-nonce"))
        XCTAssertFalse(visibleText.localizedCaseInsensitiveContains("command_secret"))
        XCTAssertFalse(visibleText.localizedCaseInsensitiveContains("command secret"))
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
