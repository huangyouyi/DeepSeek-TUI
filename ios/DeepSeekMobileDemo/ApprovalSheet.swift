import SwiftUI

struct ApprovalSheet: View {
    @Environment(\.dismiss) private var dismiss
    @EnvironmentObject private var bridge: MockMobileCoreBridge
    let command: PendingCommand

    var body: some View {
        NavigationStack {
            VStack(alignment: .leading, spacing: 18) {
                VStack(alignment: .leading, spacing: 6) {
                    Text(command.title)
                        .font(.title3.weight(.semibold))
                }

                RiskBanner(risk: command.risk)

                VStack(alignment: .leading, spacing: 8) {
                    Text("Command")
                        .font(.caption.weight(.semibold))
                        .foregroundStyle(.secondary)
                    Text(command.command)
                        .font(.system(.body, design: .monospaced))
                        .textSelection(.enabled)
                        .padding()
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .background(Color(.secondarySystemBackground))
                        .clipShape(RoundedRectangle(cornerRadius: 8, style: .continuous))
                }

                VStack(alignment: .leading, spacing: 8) {
                    DetailRow(label: "cwd", value: command.workingDirectory, systemImage: "folder")
                    DetailRow(label: "rationale", value: command.rationale, systemImage: "text.alignleft")
                    DetailRow(label: "lease id", value: command.lease.leaseIDLabel, systemImage: "checkmark.seal")
                    DetailRow(
                        label: "idempotency",
                        value: command.lease.idempotencyKeyLabel,
                        systemImage: "arrow.triangle.2.circlepath"
                    )
                    DetailRow(label: "expires", value: command.lease.expirySummary, systemImage: "clock")
                    DetailRow(
                        label: "approved action",
                        value: command.lease.approvedActionSummary,
                        systemImage: "doc.text"
                    )
                    DetailRow(
                        label: "approval nonce",
                        value: "\(command.approvalNonceStatus.rawValue) - \(command.approvalNonceLabel)",
                        systemImage: "key"
                    )
                    DetailRow(
                        label: "token material",
                        value: "Bearer and pairing tokens are never displayed in approval UI.",
                        systemImage: "eye.slash"
                    )
                }
                .font(.subheadline)

                Spacer()

                HStack(spacing: 12) {
                    Button(role: .destructive) {
                        bridge.decide(commandID: command.id, decision: .denied)
                        dismiss()
                    } label: {
                        Label("Deny", systemImage: "xmark")
                            .frame(maxWidth: .infinity)
                    }
                    .buttonStyle(.bordered)

                    Button {
                        bridge.decide(commandID: command.id, decision: .approved)
                        dismiss()
                    } label: {
                        Label(approveLabel, systemImage: "checkmark")
                            .frame(maxWidth: .infinity)
                    }
                    .buttonStyle(.borderedProminent)
                    .tint(command.risk == .high ? .red : .accentColor)
                }
            }
            .padding()
            .navigationTitle("Approval")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .topBarTrailing) {
                    Button("Close") {
                        dismiss()
                    }
                }
            }
        }
    }

    private var approveLabel: String {
        command.risk == .high ? "Approve High Risk" : "Approve"
    }
}

struct BrowserClickApprovalSheet: View {
    @Environment(\.dismiss) private var dismiss
    @EnvironmentObject private var bridge: MockMobileCoreBridge
    let approval: BrowserClickApproval

    var body: some View {
        NavigationStack {
            VStack(alignment: .leading, spacing: 18) {
                VStack(alignment: .leading, spacing: 6) {
                    Text("Browser Click Approval")
                        .font(.title3.weight(.semibold))
                    Text(approval.targetDescription)
                        .font(.subheadline)
                        .foregroundStyle(.secondary)
                }

                HStack(spacing: 10) {
                    Image(systemName: "cursorarrow.click")
                        .font(.title3.weight(.semibold))
                    Text("Browser automation")
                        .font(.headline)
                    Spacer()
                }
                .foregroundStyle(.orange)
                .padding(12)
                .frame(maxWidth: .infinity, alignment: .leading)
                .background(Color.orange.opacity(0.10))
                .clipShape(RoundedRectangle(cornerRadius: 8, style: .continuous))
                .overlay(
                    RoundedRectangle(cornerRadius: 8, style: .continuous)
                        .stroke(Color.orange.opacity(0.35), lineWidth: 1)
                )

                VStack(alignment: .leading, spacing: 8) {
                    Text("Click Target")
                        .font(.caption.weight(.semibold))
                        .foregroundStyle(.secondary)
                    Text(approval.targetDescription)
                        .font(.body)
                        .textSelection(.enabled)
                        .padding()
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .background(Color(.secondarySystemBackground))
                        .clipShape(RoundedRectangle(cornerRadius: 8, style: .continuous))
                }

                VStack(alignment: .leading, spacing: 8) {
                    DetailRow(label: "browser session", value: approval.browserSessionID, systemImage: "safari")
                    DetailRow(label: "url", value: approval.pageURL, systemImage: "link")
                    DetailRow(label: "rationale", value: approval.rationale, systemImage: "text.alignleft")
                    DetailRow(
                        label: "click nonce",
                        value: "\(clickNonceStatusLabel) - \(approval.approvalNonceLabel)",
                        systemImage: "cursorarrow.click"
                    )
                }
                .font(.subheadline)

                Spacer()

                HStack(spacing: 12) {
                    Button(role: .destructive) {
                        bridge.decide(browserClickApprovalID: approval.id, decision: .denied)
                        dismiss()
                    } label: {
                        Label("Deny Click", systemImage: "xmark")
                            .frame(maxWidth: .infinity)
                    }
                    .buttonStyle(.bordered)

                    Button {
                        bridge.decide(browserClickApprovalID: approval.id, decision: .approved)
                        dismiss()
                    } label: {
                        Label("Approve Click", systemImage: "checkmark")
                            .frame(maxWidth: .infinity)
                    }
                    .buttonStyle(.borderedProminent)
                }
            }
            .padding()
            .navigationTitle("Browser Approval")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .topBarTrailing) {
                    Button("Close") {
                        dismiss()
                    }
                }
            }
        }
    }

    private var clickNonceStatusLabel: String {
        approval.approvalNonceStatus == .bound
            ? "Bound to browser click"
            : approval.approvalNonceStatus.rawValue
    }
}

private struct RiskBanner: View {
    let risk: CommandRisk

    var body: some View {
        HStack(spacing: 10) {
            Image(systemName: iconName)
                .font(.title3.weight(.semibold))
            Text("\(risk.rawValue) risk")
                .font(.headline)
            Spacer()
        }
        .foregroundStyle(foregroundColor)
        .padding(12)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(backgroundColor)
        .clipShape(RoundedRectangle(cornerRadius: 8, style: .continuous))
        .overlay(
            RoundedRectangle(cornerRadius: 8, style: .continuous)
                .stroke(borderColor, lineWidth: risk == .high ? 2 : 1)
        )
    }

    private var iconName: String {
        switch risk {
        case .low:
            return "checkmark.shield"
        case .medium:
            return "exclamationmark.triangle"
        case .high:
            return "exclamationmark.octagon.fill"
        }
    }

    private var foregroundColor: Color {
        switch risk {
        case .low:
            return .green
        case .medium:
            return .orange
        case .high:
            return .red
        }
    }

    private var backgroundColor: Color {
        foregroundColor.opacity(risk == .high ? 0.18 : 0.10)
    }

    private var borderColor: Color {
        foregroundColor.opacity(risk == .high ? 0.85 : 0.35)
    }
}

private struct DetailRow: View {
    let label: String
    let value: String
    let systemImage: String

    var body: some View {
        HStack(alignment: .top, spacing: 8) {
            Image(systemName: systemImage)
                .foregroundStyle(.secondary)
                .frame(width: 18)

            VStack(alignment: .leading, spacing: 3) {
                Text(label)
                    .font(.caption.weight(.semibold))
                    .foregroundStyle(.secondary)
                Text(value)
                    .foregroundStyle(.primary)
                    .textSelection(.enabled)
            }
        }
    }
}
