import Foundation
import Security

enum CredentialStoreError: Error {
    case invalidData
    case unexpectedStatus(OSStatus)
}

protocol CredentialStore {
    func saveSecret(_ secret: String, account: String) throws
    func loadSecret(account: String) throws -> String?
    func deleteSecret(account: String) throws
}

final class KeychainCredentialStore: CredentialStore {
    private let service: String

    init(service: String = "DeepSeekMobileDemo") {
        self.service = service
    }

    func saveSecret(_ secret: String, account: String) throws {
        guard let data = secret.data(using: .utf8) else {
            throw CredentialStoreError.invalidData
        }

        let query = baseQuery(account: account)
        SecItemDelete(query as CFDictionary)

        var attributes = query
        attributes[kSecValueData as String] = data
        attributes[kSecAttrAccessible as String] = kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly

        let status = SecItemAdd(attributes as CFDictionary, nil)
        guard status == errSecSuccess else {
            throw CredentialStoreError.unexpectedStatus(status)
        }
    }

    func loadSecret(account: String) throws -> String? {
        var query = baseQuery(account: account)
        query[kSecMatchLimit as String] = kSecMatchLimitOne
        query[kSecReturnData as String] = true

        var item: CFTypeRef?
        let status = SecItemCopyMatching(query as CFDictionary, &item)

        if status == errSecItemNotFound {
            return nil
        }

        guard status == errSecSuccess else {
            throw CredentialStoreError.unexpectedStatus(status)
        }

        guard let data = item as? Data,
              let secret = String(data: data, encoding: .utf8) else {
            throw CredentialStoreError.invalidData
        }

        return secret
    }

    func deleteSecret(account: String) throws {
        let status = SecItemDelete(baseQuery(account: account) as CFDictionary)
        guard status == errSecSuccess || status == errSecItemNotFound else {
            throw CredentialStoreError.unexpectedStatus(status)
        }
    }

    private func baseQuery(account: String) -> [String: Any] {
        [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account
        ]
    }
}
