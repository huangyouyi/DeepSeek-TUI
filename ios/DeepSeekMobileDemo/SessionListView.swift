import SwiftUI

struct SessionListView: View {
    @EnvironmentObject private var bridge: MockMobileCoreBridge
    @State private var showingConnectionSetup = false
    @State private var showingBootstrapInput = false
    @State private var showingAuditLog = false

    var body: some View {
        NavigationSplitView {
            List(selection: $bridge.selectedSessionID) {
                Section("Sessions") {
                    ForEach(bridge.sessions) { session in
                        NavigationLink(value: session.id) {
                            SessionRow(session: session)
                        }
                    }
                }
            }
            .navigationTitle("DeepSeek")
            .toolbar {
                ToolbarItemGroup(placement: .topBarTrailing) {
                    Button {
                        bridge.createSession(title: "New session")
                    } label: {
                        Label("New Session", systemImage: "plus")
                    }

                    Menu {
                        Button {
                            showingConnectionSetup = true
                        } label: {
                            Label("Connection", systemImage: "wifi")
                        }

                        Button {
                            showingBootstrapInput = true
                        } label: {
                            Label("Bootstrap Output", systemImage: "doc.text")
                        }

                        Button {
                            showingAuditLog = true
                        } label: {
                            Label("Audit Log", systemImage: "list.bullet.rectangle")
                        }
                    } label: {
                        Label("Tools", systemImage: "ellipsis.circle")
                    }
                }
            }
        } detail: {
            if let session = selectedSession {
                ChatView(session: session)
            } else {
                VStack(spacing: 12) {
                    Image(systemName: "bubble.left.and.bubble.right")
                        .font(.largeTitle)
                        .foregroundStyle(.secondary)
                    Text("No Session Selected")
                        .font(.headline)
                    Text("Create or select a session to begin.")
                        .font(.subheadline)
                        .foregroundStyle(.secondary)
                }
            }
        }
        .sheet(isPresented: $showingConnectionSetup) {
            ConnectionSetupView()
                .environmentObject(bridge)
        }
        .sheet(isPresented: $showingBootstrapInput) {
            BootstrapOutputInputView()
                .environmentObject(bridge)
        }
        .sheet(isPresented: $showingAuditLog) {
            AuditLogView()
                .environmentObject(bridge)
        }
    }

    private var selectedSession: MobileSession? {
        bridge.sessions.first { $0.id == bridge.selectedSessionID }
    }
}

private struct SessionRow: View {
    let session: MobileSession

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack {
                Text(session.title)
                    .font(.headline)
                    .lineLimit(1)

                Spacer()

                if session.unreadCount > 0 {
                    Text("\(session.unreadCount)")
                        .font(.caption2.weight(.bold))
                        .foregroundStyle(.white)
                        .padding(.horizontal, 6)
                        .padding(.vertical, 3)
                        .background(Capsule().fill(Color.accentColor))
                }
            }

            Text(session.subtitle)
                .font(.subheadline)
                .foregroundStyle(.secondary)
                .lineLimit(2)

            HStack {
                StatusBadge(status: session.status)
                Spacer()
                Text(session.updatedAt, style: .relative)
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
        }
        .padding(.vertical, 4)
    }
}

private struct StatusBadge: View {
    let status: SessionStatus

    var body: some View {
        Text(status.rawValue)
            .font(.caption.weight(.semibold))
            .foregroundStyle(color)
    }

    private var color: Color {
        switch status {
        case .ready:
            .green
        case .waitingForApproval:
            .orange
        case .running:
            .blue
        case .disconnected:
            .red
        }
    }
}
