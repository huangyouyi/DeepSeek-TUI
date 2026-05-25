import SwiftUI

struct ChatView: View {
    @EnvironmentObject private var bridge: MockMobileCoreBridge
    let session: MobileSession

    @State private var draft = ""
    @State private var commandForApproval: PendingCommand?
    @State private var browserClickForApproval: BrowserClickApproval?
    @State private var showingConnectionSetup = false

    var body: some View {
        VStack(spacing: 0) {
            header
            Divider()

            ScrollViewReader { proxy in
                ScrollView {
                    LazyVStack(alignment: .leading, spacing: 12) {
                        ForEach(bridge.messages(for: session.id)) { message in
                            MessageBubble(message: message)
                                .id(message.id)
                        }

                        if let command = bridge.pendingCommand(for: session.id) {
                            CommandCardView(
                                command: command,
                                isShellAvailable: capabilityAvailable(.shell)
                            ) {
                                commandForApproval = command
                            }
                            .padding(.top, 4)
                        }

                        if let browserSession = bridge.browserSession(for: session.id) {
                            BrowserSessionCardView(
                                browserSession: browserSession,
                                isBrowserAvailable: capabilityAvailable(.browser)
                            ) { approval in
                                browserClickForApproval = approval
                            }
                            .padding(.top, 4)
                        }

                        if let maintenanceApproval = bridge.maintenanceApproval(for: session.id) {
                            MaintenanceApprovalCardView(
                                approval: maintenanceApproval,
                                isMaintenanceAvailable: capabilityAvailable(.maintenance)
                            ) { decision in
                                bridge.decide(maintenanceApprovalID: maintenanceApproval.id, decision: decision)
                            }
                            .padding(.top, 4)
                        }
                    }
                    .padding()
                }
                .onChange(of: bridge.messages(for: session.id).count) { _ in
                    if let lastID = bridge.messages(for: session.id).last?.id {
                        withAnimation {
                            proxy.scrollTo(lastID, anchor: .bottom)
                        }
                    }
                }
            }

            Divider()
            composer
        }
        .navigationTitle(session.title)
        .navigationBarTitleDisplayMode(.inline)
        .sheet(item: $commandForApproval) { command in
            ApprovalSheet(command: command)
                .environmentObject(bridge)
        }
        .sheet(item: $browserClickForApproval) { approval in
            BrowserClickApprovalSheet(approval: approval)
                .environmentObject(bridge)
        }
        .sheet(isPresented: $showingConnectionSetup) {
            ConnectionSetupView()
                .environmentObject(bridge)
        }
    }

    private var header: some View {
        HStack(spacing: 12) {
            VStack(alignment: .leading, spacing: 2) {
                Text(session.title)
                    .font(.headline)
                HStack(spacing: 6) {
                    Text(session.status.rawValue)
                    Text("-")
                    Text(bridge.connectionSettings.mode.statusLabel)
                    Text("-")
                    Text(bridge.connectionSettings.statusSummary)
                        .lineLimit(1)
                }
                .font(.caption)
                .foregroundStyle(.secondary)

                if showsCapabilities {
                    Text(capabilitiesSummary)
                        .font(.caption2)
                        .foregroundStyle(.secondary)
                        .lineLimit(1)

                    HStack(spacing: 6) {
                        ForEach(bridge.connectionSettings.discoveredRunnerCapabilities.keyStatuses) { status in
                            CapabilityBadge(status: status)
                        }
                    }
                }
            }

            Spacer()

            if bridge.pendingCommand(for: session.id) != nil {
                Image(systemName: "exclamationmark.shield")
                    .foregroundStyle(.orange)
                    .accessibilityLabel("Command waiting for approval")
            }

            if bridge.browserSession(for: session.id)?.pendingClickApproval != nil {
                Image(systemName: "cursorarrow.click")
                    .foregroundStyle(.orange)
                    .accessibilityLabel("Browser click waiting for approval")
            }

            if bridge.maintenanceApproval(for: session.id)?.approvalStatus == .waitingForDryRunApproval {
                Image(systemName: "arrow.triangle.2.circlepath")
                    .foregroundStyle(.red)
                    .accessibilityLabel("Maintenance dry-run waiting for approval")
            }

            Button {
                showingConnectionSetup = true
            } label: {
                Image(systemName: "wifi")
            }
            .buttonStyle(.bordered)
            .accessibilityLabel("Connection settings")
        }
        .padding()
    }

    private var showsCapabilities: Bool {
        bridge.connectionSettings.mode == .runner || bridge.connectionSettings.mode == .remoteMcp
    }

    private var capabilitiesSummary: String {
        let capabilities = bridge.connectionSettings.discoveredRunnerCapabilities
        guard !capabilities.isEmpty else {
            return "Capabilities: not discovered"
        }

        return "Capabilities: \(capabilities.count) tools - \(capabilities.map(\.name).joined(separator: ", "))"
    }

    private func capabilityAvailable(_ key: RunnerCapabilityKey) -> Bool {
        showsCapabilities && bridge.connectionSettings.discoveredRunnerCapabilities.supports(key)
    }

    private var composer: some View {
        HStack(alignment: .bottom, spacing: 10) {
            TextField("Message", text: $draft, axis: .vertical)
                .textFieldStyle(.roundedBorder)
                .lineLimit(1...5)

            Button {
                bridge.sendMessage(draft, in: session.id)
                draft = ""
            } label: {
                Image(systemName: "paperplane.fill")
            }
            .buttonStyle(.borderedProminent)
            .disabled(draft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
        }
        .padding()
    }
}

private struct MaintenanceApprovalCardView: View {
    let approval: MaintenanceApprovalRequest
    let isMaintenanceAvailable: Bool
    let onDecision: (CommandDecision) -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Label(approval.operation.rawValue, systemImage: operationIconName)
                    .font(.headline)

                Spacer()

                Text(approval.risk.rawValue)
                    .font(.caption.weight(.semibold))
                    .foregroundStyle(riskColor)
            }

            VStack(alignment: .leading, spacing: 6) {
                DetailLine(label: "approval", value: approval.approvalStatus.rawValue)
                DetailLine(label: "target", value: approval.targetSummary)
                DetailLine(label: "runner", value: approval.connectionIDLabel)
                DetailLine(label: "request", value: approval.requestedBy)
            }
            .font(.caption)

            if let artifact = approval.artifactVerification {
                ArtifactVerificationView(plan: artifact)
            }

            VStack(alignment: .leading, spacing: 8) {
                Text("Dry-run steps")
                    .font(.caption.weight(.semibold))
                    .foregroundStyle(.secondary)

                ForEach(approval.dryRunSteps) { step in
                    HStack(alignment: .top, spacing: 8) {
                        Image(systemName: step.destructive ? "exclamationmark.triangle.fill" : "checkmark.circle")
                            .foregroundStyle(step.destructive ? .red : .green)
                            .frame(width: 18)

                        VStack(alignment: .leading, spacing: 3) {
                            Text(step.title)
                                .font(.subheadline.weight(.semibold))
                            Text(step.detail)
                                .font(.caption)
                                .foregroundStyle(.secondary)
                        }
                    }
                }
            }
            .padding(10)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(Color.secondary.opacity(0.08))
            .clipShape(RoundedRectangle(cornerRadius: 8, style: .continuous))

            Label(approval.auditSummary, systemImage: "doc.text.magnifyingglass")
                .font(.caption)
                .foregroundStyle(.secondary)

            Label(statusMessage, systemImage: statusIconName)
                .font(.caption)
                .foregroundStyle(statusColor)

            if !isMaintenanceAvailable {
                UnavailableToolNotice(toolName: "Maintenance")
            }

            actionControls
        }
        .padding()
        .background(Color(.secondarySystemBackground))
        .clipShape(RoundedRectangle(cornerRadius: 8, style: .continuous))
    }

    @ViewBuilder
    private var actionControls: some View {
        switch approval.approvalStatus {
        case .planned:
            Label("Dry-run request is planned; no mobile approval is pending yet.", systemImage: "clock")
                .font(.caption)
                .foregroundStyle(.secondary)
                .frame(maxWidth: .infinity, alignment: .leading)
        case .waitingForDryRunApproval:
            HStack(spacing: 12) {
                Button(role: .destructive) {
                    onDecision(.denied)
                } label: {
                    Label("Deny", systemImage: "xmark")
                        .frame(maxWidth: .infinity)
                }
                .buttonStyle(.bordered)

                Button {
                    onDecision(.approved)
                } label: {
                    Label(isMaintenanceAvailable ? "Approve Dry-run" : "Unavailable", systemImage: "checkmark.shield")
                        .frame(maxWidth: .infinity)
                }
                .buttonStyle(.borderedProminent)
                .tint(.red)
            }
            .disabled(!isMaintenanceAvailable)
        case .approved:
            Label("Approved", systemImage: "checkmark.shield.fill")
                .font(.caption.weight(.semibold))
                .foregroundStyle(.green)
                .frame(maxWidth: .infinity, alignment: .leading)
        case .denied:
            Label("Denied", systemImage: "xmark.shield.fill")
                .font(.caption.weight(.semibold))
                .foregroundStyle(.red)
                .frame(maxWidth: .infinity, alignment: .leading)
        }
    }

    private var statusMessage: String {
        guard isMaintenanceAvailable else {
            return "Unavailable: runner did not report maintenance capability; no maintenance request can be sent."
        }

        switch approval.approvalStatus {
        case .planned:
            return "Waiting: dry-run plan is visible, but no runner maintenance request has been sent."
        case .waitingForDryRunApproval:
            return "Waiting: review the dry-run before the runner receives a maintenance request."
        case .approved:
            return "Dry-run sent; nonce redacted; audit updated. No raw token or nonce is shown."
        case .denied:
            return "Denied: no runner maintenance request was sent; audit updated."
        }
    }

    private var statusIconName: String {
        if !isMaintenanceAvailable {
            return "slash.circle"
        }

        switch approval.approvalStatus {
        case .planned:
            return "clock"
        case .waitingForDryRunApproval:
            return "hourglass"
        case .approved:
            return "checkmark.seal"
        case .denied:
            return "xmark.seal"
        }
    }

    private var statusColor: Color {
        if !isMaintenanceAvailable {
            return .secondary
        }

        switch approval.approvalStatus {
        case .planned:
            return .secondary
        case .waitingForDryRunApproval:
            return .orange
        case .approved:
            return .green
        case .denied:
            return .red
        }
    }

    private var operationIconName: String {
        switch approval.operation {
        case .selfUpdate:
            return "arrow.triangle.2.circlepath"
        case .uninstall:
            return "trash"
        }
    }

    private var riskColor: Color {
        switch approval.risk {
        case .low:
            return .green
        case .medium:
            return .orange
        case .high:
            return .red
        }
    }
}

private struct ArtifactVerificationView: View {
    let plan: ArtifactVerificationPlan

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("Artifact verification")
                .font(.caption.weight(.semibold))
                .foregroundStyle(.secondary)

            VStack(alignment: .leading, spacing: 6) {
                DetailLine(label: "artifact", value: plan.artifactName)
                DetailLine(label: "checksum", value: plan.checksumSource)
                DetailLine(label: "signature", value: plan.signatureSource)
                DetailLine(label: "verify", value: plan.verifyStep)
                DetailLine(label: "rollback", value: plan.rollbackGuidance)
            }
            .font(.caption)
        }
        .padding(10)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Color.secondary.opacity(0.08))
        .clipShape(RoundedRectangle(cornerRadius: 8, style: .continuous))
    }
}

private struct BrowserSessionCardView: View {
    let browserSession: MockBrowserSession
    let isBrowserAvailable: Bool
    let onReviewClick: (BrowserClickApproval) -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Label("Browser session", systemImage: "safari")
                    .font(.headline)

                Spacer()

                Text(browserSession.status.rawValue)
                    .font(.caption.weight(.semibold))
                    .foregroundStyle(statusColor)
            }

            VStack(alignment: .leading, spacing: 6) {
                DetailLine(label: "session", value: browserSession.id)
                DetailLine(label: "page", value: browserSession.pageTitle)
                DetailLine(label: "url", value: browserSession.pageURL)
            }
            .font(.caption)

            if let preview = browserSession.extractedTextPreview,
               !preview.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                VStack(alignment: .leading, spacing: 6) {
                    Text("Extracted text")
                        .font(.caption.weight(.semibold))
                        .foregroundStyle(.secondary)
                    Text(preview)
                        .font(.subheadline)
                        .lineLimit(4)
                        .padding(10)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .background(Color.secondary.opacity(0.08))
                        .clipShape(RoundedRectangle(cornerRadius: 8, style: .continuous))
                }
            }

            Label("\(clickNonceStatusLabel) - \(browserSession.clickApprovalNonceLabel)", systemImage: "cursorarrow.click")
                .font(.caption)
                .foregroundStyle(.secondary)

            if !isBrowserAvailable {
                UnavailableToolNotice(toolName: "Browser")
            }

            if let approval = browserSession.pendingClickApproval {
                Button {
                    onReviewClick(approval)
                } label: {
                    Label("Review Browser Click", systemImage: "checkmark.shield")
                        .frame(maxWidth: .infinity)
                }
                .buttonStyle(.borderedProminent)
                .disabled(!isBrowserAvailable)
            }
        }
        .padding()
        .background(Color(.secondarySystemBackground))
        .clipShape(RoundedRectangle(cornerRadius: 8, style: .continuous))
    }

    private var statusColor: Color {
        switch browserSession.status {
        case .opened, .textExtracted:
            .blue
        case .waitingForClickApproval:
            .orange
        case .clickApproved:
            .green
        case .clickDenied:
            .red
        }
    }

    private var clickNonceStatusLabel: String {
        browserSession.clickApprovalNonceStatus == .bound
            ? "Bound to browser click"
            : browserSession.clickApprovalNonceStatus.rawValue
    }
}

private struct CapabilityBadge: View {
    let status: RunnerCapabilityStatus

    var body: some View {
        Image(systemName: status.isAvailable ? "checkmark.circle.fill" : "xmark.circle")
            .foregroundStyle(status.isAvailable ? Color.green : Color.secondary)
            .accessibilityLabel("\(status.key.displayName) \(status.statusLabel)")
    }
}

private struct UnavailableToolNotice: View {
    let toolName: String

    var body: some View {
        Label("\(toolName) unavailable from current runner capabilities", systemImage: "slash.circle")
            .font(.caption)
            .foregroundStyle(.secondary)
    }
}

private struct DetailLine: View {
    let label: String
    let value: String

    var body: some View {
        HStack(alignment: .top, spacing: 6) {
            Text(label)
                .fontWeight(.semibold)
                .foregroundStyle(.secondary)
                .frame(width: 72, alignment: .leading)
            Text(value)
                .foregroundStyle(.primary)
                .lineLimit(3)
        }
    }
}

private struct MessageBubble: View {
    let message: ChatMessage

    var body: some View {
        HStack {
            if message.role == .user {
                Spacer(minLength: 40)
            }

            VStack(alignment: .leading, spacing: 6) {
                Text(message.role.rawValue)
                    .font(.caption.weight(.semibold))
                    .foregroundStyle(.secondary)

                Text(message.body)
                    .font(.body)
                    .foregroundStyle(.primary)
            }
            .padding(12)
            .background(background)
            .clipShape(RoundedRectangle(cornerRadius: 8, style: .continuous))

            if message.role != .user {
                Spacer(minLength: 40)
            }
        }
    }

    private var background: Color {
        switch message.role {
        case .user:
            Color.accentColor.opacity(0.14)
        case .assistant:
            Color.secondary.opacity(message.isThinking ? 0.10 : 0.08)
        case .system:
            Color.orange.opacity(0.12)
        }
    }
}
