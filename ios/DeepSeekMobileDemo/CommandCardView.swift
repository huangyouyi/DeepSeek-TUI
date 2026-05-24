import SwiftUI

struct CommandCardView: View {
    let command: PendingCommand
    let isShellAvailable: Bool
    let onReview: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Label(command.title, systemImage: "terminal")
                    .font(.headline)

                Spacer()

                Text(command.risk.rawValue)
                    .font(.caption.weight(.semibold))
                    .foregroundStyle(riskColor)
            }

            Text(command.command)
                .font(.system(.body, design: .monospaced))
                .textSelection(.enabled)
                .padding(10)
                .frame(maxWidth: .infinity, alignment: .leading)
                .background(Color.secondary.opacity(0.08))
                .clipShape(RoundedRectangle(cornerRadius: 8, style: .continuous))

            Text(command.workingDirectory)
                .font(.caption)
                .foregroundStyle(.secondary)
                .lineLimit(1)

            Text(command.rationale)
                .font(.subheadline)
                .foregroundStyle(.secondary)

            VStack(alignment: .leading, spacing: 6) {
                Label("Lease \(command.lease.leaseIDLabel)", systemImage: "checkmark.seal")
                Label("Idempotency \(command.lease.idempotencyKeyLabel)", systemImage: "arrow.triangle.2.circlepath")
                Label("Expires \(command.lease.expirySummary)", systemImage: "clock")
                Label(command.lease.approvedActionSummary, systemImage: "doc.text")
            }
            .font(.caption)
            .foregroundStyle(.secondary)

            Label("\(command.approvalNonceStatus.rawValue) - \(command.approvalNonceLabel)", systemImage: "key")
                .font(.caption)
                .foregroundStyle(.secondary)

            Label("Bearer and pairing tokens are not displayed", systemImage: "eye.slash")
                .font(.caption)
                .foregroundStyle(.secondary)

            if !isShellAvailable {
                Label("Shell unavailable from current runner capabilities", systemImage: "slash.circle")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }

            Button {
                onReview()
            } label: {
                Label("Review Command", systemImage: "checkmark.shield")
                    .frame(maxWidth: .infinity)
            }
            .buttonStyle(.borderedProminent)
            .disabled(!isShellAvailable)
        }
        .padding()
        .background(Color(.secondarySystemBackground))
        .clipShape(RoundedRectangle(cornerRadius: 8, style: .continuous))
    }

    private var riskColor: Color {
        switch command.risk {
        case .low:
            .green
        case .medium:
            .orange
        case .high:
            .red
        }
    }
}
