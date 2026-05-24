import SwiftUI

struct ConnectionSetupView: View {
    @Environment(\.dismiss) private var dismiss
    @EnvironmentObject private var bridge: MockMobileCoreBridge

    @State private var pairingCodeInput = ""
    @State private var pairingMessage = ""
    @State private var pairingUpgradeEndpoint = ""
    @State private var pairingUpgradeToken = ""
    @State private var pairingUpgradeCapabilitiesJSON = Self.defaultPairingUpgradeCapabilitiesJSON
    @State private var pairingUpgradeStatus = ""
    @State private var pairingUpgradeProfile: RunnerProfile?

    var body: some View {
        NavigationStack {
            Form {
                Section("Mode") {
                    Picker("Connection Mode", selection: $bridge.connectionSettings.mode) {
                        ForEach(ConnectionMode.allCases) { mode in
                            Text(mode.rawValue).tag(mode)
                        }
                    }
                }

                connectionFields
                authReadinessSection
                capabilitySection
                pairingSection
                pairingUpgradeSection

                Section("Token") {
                    TextField("Token Label", text: $bridge.connectionSettings.tokenLabel)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                }

                Section("Bootstrap") {
                    if bridge.connectionSettings.bootstrapOutput.isEmpty {
                        Text("No bootstrap output imported.")
                            .foregroundStyle(.secondary)
                    } else {
                        Text(bridge.connectionSettings.bootstrapOutput)
                            .font(.system(.caption, design: .monospaced))
                            .lineLimit(4)
                    }
                }
            }
            .navigationTitle("Connection")
            .onAppear {
                if pairingUpgradeEndpoint.isEmpty {
                    pairingUpgradeEndpoint = bridge.connectionSettings.endpoint
                }
            }
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") {
                        dismiss()
                    }
                }

                ToolbarItem(placement: .confirmationAction) {
                    Button("Save") {
                        bridge.saveConnectionSettings()
                        dismiss()
                    }
                }
            }
        }
    }

    @ViewBuilder
    private var pairingUpgradeSection: some View {
        if bridge.connectionSettings.mode == .runner || bridge.connectionSettings.mode == .remoteMcp {
            Section("CoreBridge Pairing Upgrade") {
                TextField("Endpoint", text: $pairingUpgradeEndpoint)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()

                SecureField("Pairing Token", text: $pairingUpgradeToken)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()

                VStack(alignment: .leading, spacing: 6) {
                    Text("Capabilities JSON")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                    TextEditor(text: $pairingUpgradeCapabilitiesJSON)
                        .font(.system(.caption, design: .monospaced))
                        .frame(minHeight: 120)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                }

                Button {
                    submitPairingUpgrade()
                } label: {
                    Label("Upgrade Pairing", systemImage: "arrow.up.forward.app")
                }
                .disabled(pairingUpgradeEndpoint.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ||
                    pairingUpgradeToken.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)

                if !pairingUpgradeStatus.isEmpty {
                    Text(pairingUpgradeStatus)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }

                if let profile = pairingUpgradeProfile {
                    LabeledContent("Profile Endpoint") {
                        Text(profile.endpoint)
                            .lineLimit(1)
                    }
                    LabeledContent("Runner Profile") {
                        Text(profile.accountLabel)
                    }
                    LabeledContent("Credential") {
                        Text(profile.tokenLabel)
                    }
                    LabeledContent("Fallback") {
                        Text(profile.isMockFallback ? "Mock fallback" : "CoreBridge")
                            .foregroundStyle(profile.isMockFallback ? Color.orange : Color.green)
                    }

                    if profile.capabilities.isEmpty {
                        Text("No capabilities reported by pairing upgrade.")
                            .foregroundStyle(.secondary)
                    } else {
                        ForEach(profile.capabilities) { capability in
                            VStack(alignment: .leading, spacing: 2) {
                                Text(capability.name)
                                Text(capability.summary)
                                    .font(.caption)
                                    .foregroundStyle(.secondary)
                            }
                        }
                    }
                }
            }
        }
    }

    @ViewBuilder
    private var authReadinessSection: some View {
        if bridge.connectionSettings.mode == .runner || bridge.connectionSettings.mode == .remoteMcp {
            let metadata = bridge.runnerRequestAuthMetadata()

            Section("Runner Request Auth") {
                LabeledContent("Readiness") {
                    Text(metadata.readinessLabel)
                        .foregroundStyle(authReadinessColor(for: metadata.availability))
                }

                LabeledContent("Endpoint") {
                    Text(metadata.endpoint)
                        .lineLimit(1)
                }

                LabeledContent("Token Account") {
                    Text(metadata.tokenAccountLabel ?? "Unavailable")
                        .foregroundStyle(metadata.tokenAccountLabel == nil ? Color.secondary : Color.primary)
                }

                LabeledContent("Token") {
                    Text(metadata.tokenPresent ? "Present" : "Not present")
                        .foregroundStyle(metadata.tokenPresent ? Color.green : Color.secondary)
                }

                LabeledContent("Approval Nonce") {
                    Text(metadata.approvalNonce?.isPresent == true ? "Present" : "Not present")
                        .foregroundStyle(metadata.approvalNonce?.isPresent == true ? Color.green : Color.secondary)
                }

                if let approvalNonce = metadata.approvalNonce {
                    LabeledContent("Nonce Label") {
                        Text("\(approvalNonce.status.rawValue) - \(approvalNonce.label)")
                    }
                }

                Text(metadata.detailLabel)
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
        }
    }

    @ViewBuilder
    private var pairingSection: some View {
        if bridge.connectionSettings.mode == .runner || bridge.connectionSettings.mode == .remoteMcp {
            Section("Pairing") {
                LabeledContent("Status") {
                    Text(bridge.connectionSettings.pairingStatusLabel)
                        .foregroundStyle(bridge.connectionSettings.isPaired ? Color.green : Color.secondary)
                }

                if !bridge.connectionSettings.pendingPairingCode.isEmpty {
                    LabeledContent("Code") {
                        Text(bridge.connectionSettings.pendingPairingCode)
                            .font(.system(.body, design: .monospaced))
                    }
                }

                Button {
                    pairingCodeInput = bridge.requestPairingCode()
                    pairingMessage = "Pairing code ready. Enter it below to redeem the mock token."
                } label: {
                    Label("Request Pairing Code", systemImage: "number.square")
                }

                TextField("Pairing Code", text: $pairingCodeInput)
                    .keyboardType(.numberPad)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()

                Button {
                    if bridge.redeemPairingCode(pairingCodeInput) {
                        pairingCodeInput = ""
                        pairingMessage = "Paired. Capabilities discovered for the credential label."
                    } else {
                        pairingMessage = "Pairing code did not match the active mock code."
                    }
                } label: {
                    Label("Redeem Pairing Code", systemImage: "checkmark.seal")
                }
                .disabled(pairingCodeInput.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)

                if !pairingMessage.isEmpty {
                    Text(pairingMessage)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
            }
        }
    }

    @ViewBuilder
    private var capabilitySection: some View {
        if bridge.connectionSettings.mode == .runner || bridge.connectionSettings.mode == .remoteMcp {
            Section("Capabilities") {
                LabeledContent("Pairing") {
                    Text(bridge.connectionSettings.pairingStatusLabel)
                        .foregroundStyle(bridge.connectionSettings.isPaired ? Color.green : Color.secondary)
                }

                Button {
                    bridge.discoverRunnerCapabilities()
                } label: {
                    Label("Discover", systemImage: "arrow.clockwise")
                }
                .disabled(!bridge.connectionSettings.isPaired)

                if bridge.connectionSettings.discoveredRunnerCapabilities.isEmpty {
                    Text(bridge.connectionSettings.isPaired ? "No paired capabilities discovered." : "Pair or import a runner report before tools are enabled.")
                        .foregroundStyle(.secondary)
                } else {
                    Text("\(bridge.connectionSettings.discoveredRunnerCapabilities.count) paired capabilities")
                        .foregroundStyle(.secondary)

                    ForEach(bridge.connectionSettings.discoveredRunnerCapabilities) { capability in
                        VStack(alignment: .leading, spacing: 2) {
                            Text(capability.name)
                                .font(.body)
                            Text(capability.summary)
                                .font(.caption)
                                .foregroundStyle(.secondary)
                        }
                    }
                }

                VStack(alignment: .leading, spacing: 10) {
                    ForEach(bridge.connectionSettings.discoveredRunnerCapabilities.keyStatuses) { status in
                        CapabilityStatusRow(status: status)
                    }
                }
                .padding(.vertical, 4)
            }
        }
    }

    private func authReadinessColor(for availability: RunnerRequestAuthAvailability) -> Color {
        switch availability {
        case .unavailable:
            return .secondary
        case .unpaired:
            return .orange
        case .ready:
            return .green
        }
    }

    private func submitPairingUpgrade() {
        let profile = bridge.upgradePairing(
            endpoint: pairingUpgradeEndpoint,
            pairingToken: pairingUpgradeToken,
            capabilitiesJSON: pairingUpgradeCapabilitiesJSON
        )
        pairingUpgradeProfile = profile
        pairingUpgradeToken = ""
        pairingMessage = ""
        pairingUpgradeStatus = profile.isMockFallback
            ? "Upgraded with mock fallback. Pairing token cleared and redacted."
            : "Upgraded through CoreBridge. Pairing token cleared and redacted."
    }

    private static let defaultPairingUpgradeCapabilitiesJSON = """
    {
      "capabilities": [
        {"name": "shell", "summary": "Run approved shell commands"},
        {"name": "browser", "summary": "Drive browser automation"},
        {"name": "maintenance", "summary": "Plan self-update and uninstall dry-runs"},
        {"name": "mcp_proxy", "summary": "Proxy approved MCP tools"}
      ]
    }
    """

    @ViewBuilder
    private var connectionFields: some View {
        switch bridge.connectionSettings.mode {
        case .bootstrap:
            Section("Bootstrap Source") {
                TextField("Endpoint", text: $bridge.connectionSettings.endpoint)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
            }
        case .ssh:
            Section("SSH") {
                TextField("Host", text: $bridge.connectionSettings.host)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                TextField("Port", text: $bridge.connectionSettings.port)
                    .keyboardType(.numberPad)
                TextField("Username", text: $bridge.connectionSettings.username)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
            }
        case .runner:
            Section("Runner") {
                TextField("Host", text: $bridge.connectionSettings.host)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                TextField("Endpoint", text: $bridge.connectionSettings.endpoint)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                TextField("Port", text: $bridge.connectionSettings.port)
                    .keyboardType(.numberPad)
            }
        case .remoteMcp:
            Section("Remote MCP") {
                TextField("Endpoint", text: $bridge.connectionSettings.endpoint)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
            }
        }
    }
}

private struct CapabilityStatusRow: View {
    let status: RunnerCapabilityStatus

    var body: some View {
        HStack(alignment: .top, spacing: 10) {
            Image(systemName: status.key.systemImage)
                .foregroundStyle(status.isAvailable ? Color.green : Color.secondary)
                .frame(width: 20)

            VStack(alignment: .leading, spacing: 2) {
                Text(status.key.displayName)
                    .font(.subheadline.weight(.semibold))

                Text(status.summary)
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }

            Spacer()

            Text(status.statusLabel)
                .font(.caption.weight(.semibold))
                .foregroundStyle(status.isAvailable ? Color.green : Color.secondary)
        }
    }
}
