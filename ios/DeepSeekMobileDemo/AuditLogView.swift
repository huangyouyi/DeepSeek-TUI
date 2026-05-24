import SwiftUI

struct AuditLogView: View {
    @Environment(\.dismiss) private var dismiss
    @EnvironmentObject private var bridge: MockMobileCoreBridge

    @State private var selectedFilter: AuditTimelineFilter = .all

    var body: some View {
        NavigationStack {
            List {
                Section {
                    Picker("Audit filter", selection: $selectedFilter) {
                        ForEach(AuditTimelineFilter.allCases) { filter in
                            Text(filter.rawValue).tag(filter)
                        }
                    }
                    .pickerStyle(.segmented)
                    .accessibilityLabel("Audit timeline filter")
                }

                Section("Timeline") {
                    ForEach(timelineEntries) { entry in
                        AuditTimelineRow(entry: entry, iconName: iconName(for: entry.kind))
                    }
                }
            }
            .navigationTitle("Audit Timeline")
            .toolbar {
                ToolbarItem(placement: .topBarTrailing) {
                    Button("Done") {
                        dismiss()
                    }
                }
            }
        }
    }

    private var timelineEntries: [AuditTimelineEntry] {
        bridge.auditEvents
            .filter { selectedFilter.includes($0) }
            .map(AuditTimelineEntry.init(event:))
    }

    private func iconName(for kind: AuditEventKind) -> String {
        switch kind {
        case .connection:
            "wifi"
        case .session:
            "bubble.left.and.bubble.right"
        case .command:
            "terminal"
        case .credential:
            "key"
        case .bootstrap:
            "shippingbox"
        case .windows:
            "desktopcomputer"
        case .browser:
            "safari"
        case .maintenance:
            "arrow.triangle.2.circlepath"
        }
    }
}

private struct AuditTimelineRow: View {
    let entry: AuditTimelineEntry
    let iconName: String

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack(alignment: .firstTextBaseline) {
                Label(entry.title, systemImage: iconName)
                    .font(.headline)

                Spacer()

                Text(entry.timestamp, style: .relative)
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }

            HStack(spacing: 8) {
                TimelinePill(label: entry.source, systemImage: "tray.full", color: .blue)
                TimelinePill(label: entry.status, systemImage: "checklist", color: statusColor)
                TimelinePill(label: "\(entry.risk.rawValue) risk", systemImage: "exclamationmark.shield", color: riskColor)
            }

            VStack(alignment: .leading, spacing: 6) {
                Label(entry.userAction, systemImage: "person.crop.circle.badge.checkmark")
                if let leaseLifecycleLabel = entry.leaseLifecycleLabel {
                    Label(leaseLifecycleLabel, systemImage: "checkmark.seal")
                }
                if !entry.commandLeaseMetadata.isEmpty {
                    Label(entry.commandLeaseMetadata, systemImage: "number")
                }
                Label(entry.runnerAuditSummary, systemImage: "doc.text.magnifyingglass")
            }
            .font(.subheadline)
            .foregroundStyle(.secondary)
        }
        .padding(.vertical, 6)
    }

    private var statusColor: Color {
        switch entry.status {
        case "Approved", "Recorded":
            return .green
        case "Denied":
            return .red
        case "Waiting":
            return .orange
        case "Rescue":
            return .purple
        default:
            return .secondary
        }
    }

    private var riskColor: Color {
        switch entry.risk {
        case .low:
            return .green
        case .medium:
            return .orange
        case .high:
            return .red
        }
    }
}

private struct TimelinePill: View {
    let label: String
    let systemImage: String
    let color: Color

    var body: some View {
        Label(label, systemImage: systemImage)
            .font(.caption.weight(.semibold))
            .foregroundStyle(color)
            .labelStyle(.titleAndIcon)
    }
}
