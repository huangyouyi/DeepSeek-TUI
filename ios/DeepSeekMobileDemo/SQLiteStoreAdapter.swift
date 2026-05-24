import Foundation
import SQLite3

private let SQLITE_TRANSIENT = unsafeBitCast(-1, to: sqlite3_destructor_type.self)

enum SQLiteStoreError: Error {
    case openFailed(String)
    case executeFailed(String)
}

protocol MobileStoreAdapter {
    func open() throws
    func recordAuditEvent(_ event: AuditEvent) throws
    func close()
}

final class SQLiteStoreAdapter: MobileStoreAdapter {
    private let databaseURL: URL
    private var database: OpaquePointer?

    init(databaseURL: URL) {
        self.databaseURL = databaseURL
    }

    func open() throws {
        guard database == nil else {
            return
        }

        if sqlite3_open(databaseURL.path, &database) != SQLITE_OK {
            let message = database.map { String(cString: sqlite3_errmsg($0)) } ?? "Unable to open database"
            throw SQLiteStoreError.openFailed(message)
        }

        try execute(
            """
            CREATE TABLE IF NOT EXISTS audit_events (
                id TEXT PRIMARY KEY,
                timestamp REAL NOT NULL,
                kind TEXT NOT NULL,
                title TEXT NOT NULL,
                detail TEXT NOT NULL
            );
            """
        )
    }

    func recordAuditEvent(_ event: AuditEvent) throws {
        try open()

        let sql = """
        INSERT OR REPLACE INTO audit_events (id, timestamp, kind, title, detail)
        VALUES (?, ?, ?, ?, ?);
        """

        var statement: OpaquePointer?
        guard sqlite3_prepare_v2(database, sql, -1, &statement, nil) == SQLITE_OK else {
            throw SQLiteStoreError.executeFailed(lastErrorMessage)
        }
        defer { sqlite3_finalize(statement) }

        sqlite3_bind_text(statement, 1, event.id.uuidString, -1, SQLITE_TRANSIENT)
        sqlite3_bind_double(statement, 2, event.timestamp.timeIntervalSince1970)
        sqlite3_bind_text(statement, 3, event.kind.rawValue, -1, SQLITE_TRANSIENT)
        sqlite3_bind_text(statement, 4, event.title, -1, SQLITE_TRANSIENT)
        sqlite3_bind_text(statement, 5, event.detail, -1, SQLITE_TRANSIENT)

        guard sqlite3_step(statement) == SQLITE_DONE else {
            throw SQLiteStoreError.executeFailed(lastErrorMessage)
        }
    }

    func close() {
        if let database {
            sqlite3_close(database)
            self.database = nil
        }
    }

    deinit {
        close()
    }

    private func execute(_ sql: String) throws {
        guard sqlite3_exec(database, sql, nil, nil, nil) == SQLITE_OK else {
            throw SQLiteStoreError.executeFailed(lastErrorMessage)
        }
    }

    private var lastErrorMessage: String {
        database.map { String(cString: sqlite3_errmsg($0)) } ?? "Database is not open"
    }
}
